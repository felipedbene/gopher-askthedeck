//! Cosmic context — a pure, std-only ephemeris computed from the server clock.
//!
//! askthedeck computes the sky with the `astronomy-engine` npm package
//! (full VSOP/ELP precision). We don't ship a JS engine in a gopher dcgi, so
//! this is a faithful *port of the result*: low-precision geocentric solar and
//! lunar longitudes (Schlyter / Meeus), with the Moon's principal perturbation
//! terms included so the longitude is good to a few arc-minutes — far inside the
//! 30°-wide zodiac buckets and 45°-wide phase buckets we actually report.
//!
//! The bucket *labels* (sun sign, moon sign, moon phase, planetary day) are what
//! reach the reader and the LLM, and they match `astronomy-engine` on the pinned
//! fixture dates (see tests). The same definitions as the original:
//!   - sun/moon sign  = floor(longitude / 30)
//!   - moon phase name = floor(((moonLon - sunLon) + 22.5) / 45)   [8 phases]
//!   - planetary day   = weekday, Sun..Saturn (Sunday = Sun)
//!
//! Ethical note: this module computes *only* sky state from a timestamp. The
//! calendar date is deliberately NOT part of [`Cosmic::prompt_block`] — see the
//! comment there and the prompt-guard test. No geolocation, ever.

use std::f64::consts::PI;

const ZODIAC: [&str; 12] = [
    "Aries",
    "Taurus",
    "Gemini",
    "Cancer",
    "Leo",
    "Virgo",
    "Libra",
    "Scorpio",
    "Sagittarius",
    "Capricorn",
    "Aquarius",
    "Pisces",
];

const MOON_PHASES: [&str; 8] = [
    "New Moon",
    "Waxing Crescent",
    "First Quarter",
    "Waxing Gibbous",
    "Full Moon",
    "Waning Gibbous",
    "Last Quarter",
    "Waning Crescent",
];

/// Indexed by weekday with Sunday = 0, matching askthedeck's
/// `PLANETARY_DAYS[now.getDay()]`.
const PLANETARY_DAYS: [&str; 7] = [
    "Sun", "Moon", "Mars", "Mercury", "Jupiter", "Venus", "Saturn",
];

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// A civil UTC instant — the only input to the ephemeris. The IO layer reads the
/// system clock and hands one of these in; the math stays pure and testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CivilTime {
    pub year: i32,
    pub month: u32, // 1..=12
    pub day: u32,   // 1..=31
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl CivilTime {
    /// Convert Unix epoch seconds (UTC) into a civil date/time. Pure — the
    /// `SystemTime` read lives in the IO layer. Uses Howard Hinnant's
    /// `days_from_civil` inverse (`civil_from_days`).
    pub fn from_unix(secs: i64) -> CivilTime {
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        CivilTime {
            year,
            month,
            day,
            hour: (rem / 3600) as u32,
            minute: ((rem % 3600) / 60) as u32,
            second: (rem % 60) as u32,
        }
    }

    /// Weekday with Sunday = 0 .. Saturday = 6 (matches JS `getUTCDay`).
    fn weekday(self) -> usize {
        // days_from_civil(1970-01-01) = 0, a Thursday. (days + 4) % 7 → Sun=0.
        let days = days_from_civil(self.year, self.month, self.day);
        (days.rem_euclid(7) as usize + 4) % 7
    }
}

/// The computed sky: longitudes plus the human/LLM-facing bucket labels.
#[derive(Debug, Clone, PartialEq)]
pub struct Cosmic {
    pub sun_longitude: f64,
    pub sun_sign: &'static str,
    pub moon_longitude: f64,
    pub moon_sign: &'static str,
    pub phase_angle: f64,
    pub moon_phase: &'static str,
    pub planetary_day: &'static str,
    /// Calendar fields, retained for the human-facing cosmic page only — NOT for
    /// the LLM prompt (see [`Cosmic::prompt_block`]).
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Cosmic {
    /// The astrological-weather block fed to the LLM. Deliberately omits the
    /// calendar date and the time of day: the moon phase, moon sign, and zodiac
    /// season already carry the only temporal information a reading needs, and a
    /// precise date is the closest thing to a "locating timestamp" the prompt
    /// guard forbids. Planetary day is the *planet* (Saturn), never the weekday
    /// name (Saturday), so it leaks nothing locating either.
    pub fn prompt_block(&self) -> String {
        format!(
            "**CONTEXTUAL FRAMEWORK:**\n\
             - **Current Moon Phase:** {} in {}\n\
             - **Zodiac Season:** {} Season\n\
             - **Planetary Day:** {}",
            self.moon_phase, self.moon_sign, self.sun_sign, self.planetary_day
        )
    }

    /// A one-line human summary for menus and the cosmic page (date allowed here
    /// — this never reaches the LLM).
    pub fn human_line(&self) -> String {
        format!(
            "{} {}, {} — {} in {}, {} Season, {}'s day",
            MONTHS[(self.month - 1) as usize],
            self.day,
            self.year,
            self.moon_phase,
            self.moon_sign,
            self.sun_sign,
            self.planetary_day,
        )
    }
}

/// Compute the cosmic context for a UTC instant.
pub fn compute(ct: CivilTime) -> Cosmic {
    let d = day_number(ct);
    let sun_longitude = sun_ecliptic_longitude(d);
    let moon_longitude = moon_ecliptic_longitude(d);
    let phase_angle = rev(moon_longitude - sun_longitude);

    Cosmic {
        sun_longitude,
        sun_sign: zodiac_sign(sun_longitude),
        moon_longitude,
        moon_sign: zodiac_sign(moon_longitude),
        phase_angle,
        moon_phase: moon_phase_name(phase_angle),
        planetary_day: PLANETARY_DAYS[ct.weekday()],
        year: ct.year,
        month: ct.month,
        day: ct.day,
    }
}

fn zodiac_sign(longitude: f64) -> &'static str {
    ZODIAC[(rev(longitude) / 30.0).floor() as usize % 12]
}

fn moon_phase_name(phase_angle: f64) -> &'static str {
    let shifted = rev(phase_angle + 22.5);
    MOON_PHASES[(shifted / 45.0).floor() as usize % 8]
}

// ---- ephemeris (Schlyter "How to compute planetary positions") ------------

/// Days since the epoch 2000 Jan 0.0 TT (= JD 2451543.5), plus the UT fraction.
fn day_number(ct: CivilTime) -> f64 {
    let y = ct.year as i64;
    let m = ct.month as i64;
    let day = ct.day as i64;
    // Schlyter's integer day count (truncating division, valid for our era).
    let d = 367 * y - (7 * (y + ((m + 9) / 12))) / 4 + (275 * m) / 9 + day - 730_530;
    d as f64 + (ct.hour as f64 + ct.minute as f64 / 60.0 + ct.second as f64 / 3600.0) / 24.0
}

fn sun_ecliptic_longitude(d: f64) -> f64 {
    let w = 282.9404 + 4.70935e-5 * d; // longitude of perihelion
    let e = 0.016709 - 1.151e-9 * d; // eccentricity
    let m = rev(356.0470 + 0.9856002585 * d); // mean anomaly

    let e_anom = eccentric_anomaly_deg(m, e);
    let xv = e_anom.to_radians().cos() - e;
    let yv = (1.0 - e * e).sqrt() * e_anom.to_radians().sin();
    let v = atan2_deg(yv, xv); // true anomaly
    rev(v + w)
}

fn moon_ecliptic_longitude(d: f64) -> f64 {
    // Moon orbital elements
    let n = rev(125.1228 - 0.0529538083 * d); // ascending node
    let i = 5.1454_f64; // inclination
    let w = rev(318.0634 + 0.1643573223 * d); // arg. of perigee
    let a = 60.2666_f64; // mean distance (Earth radii)
    let e = 0.054900_f64; // eccentricity
    let mm = rev(115.3654 + 13.0649929509 * d); // moon mean anomaly

    let e_anom = eccentric_anomaly_deg(mm, e);
    let x = a * (e_anom.to_radians().cos() - e);
    let y = a * (1.0 - e * e).sqrt() * e_anom.to_radians().sin();
    let r = (x * x + y * y).sqrt();
    let v = atan2_deg(y, x);

    // position in the ecliptic
    let vw = (v + w).to_radians();
    let nr = n.to_radians();
    let ir = i.to_radians();
    let xeclip = r * (nr.cos() * vw.cos() - nr.sin() * vw.sin() * ir.cos());
    let yeclip = r * (nr.sin() * vw.cos() + nr.cos() * vw.sin() * ir.cos());
    let mut lon = atan2_deg(yeclip, xeclip);

    // --- principal perturbations of the Moon's longitude (degrees) ----------
    // Without these the longitude can be off by >1°, which would mis-bucket the
    // moon sign near a cusp. Sun's elements needed for the cross terms:
    let ws = 282.9404 + 4.70935e-5 * d;
    let ms = rev(356.0470 + 0.9856002585 * d); // sun mean anomaly
    let ls = rev(ms + ws); // sun mean longitude
    let lm = rev(n + w + mm); // moon mean longitude
    let dmoon = rev(lm - ls); // mean elongation
    let f = rev(lm - n); // argument of latitude

    let s = |deg: f64| deg.to_radians().sin();
    lon += -1.274 * s(mm - 2.0 * dmoon); // evection
    lon += 0.658 * s(2.0 * dmoon); // variation
    lon += -0.186 * s(ms); // yearly equation
    lon += -0.059 * s(2.0 * mm - 2.0 * dmoon);
    lon += -0.057 * s(mm - 2.0 * dmoon + ms);
    lon += 0.053 * s(mm + 2.0 * dmoon);
    lon += 0.046 * s(2.0 * dmoon - ms);
    lon += 0.041 * s(mm - ms);
    lon += -0.035 * s(dmoon); // parallactic equation
    lon += -0.031 * s(mm + ms);
    lon += -0.015 * s(2.0 * f - 2.0 * dmoon);
    lon += 0.011 * s(mm - 4.0 * dmoon);

    rev(lon)
}

/// Solve Kepler's equation for the eccentric anomaly (degrees). One Newton
/// refinement after the first approximation — ample for these eccentricities.
fn eccentric_anomaly_deg(m_deg: f64, e: f64) -> f64 {
    let m = m_deg.to_radians();
    let e0 = m + e * m.sin() * (1.0 + e * m.cos());
    // single Newton step
    let e1 = e0 - (e0 - e * e0.sin() - m) / (1.0 - e * e0.cos());
    e1.to_degrees()
}

fn atan2_deg(y: f64, x: f64) -> f64 {
    rev(y.atan2(x) * 180.0 / PI)
}

/// Normalize an angle to `[0, 360)`.
fn rev(x: f64) -> f64 {
    x.rem_euclid(360.0)
}

// ---- calendar (Howard Hinnant's algorithms) -------------------------------

/// Days from 1970-01-01 to the given civil date. Valid for any proleptic
/// Gregorian date.
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let m = m as i64;
    let d = d as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`].
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ground truth captured from askthedeck's `astronomy-engine` (the npm
    /// package the original ships) for these UTC instants. We assert the bucket
    /// labels match exactly and the longitudes match within a tolerance that low-
    /// precision theory comfortably achieves away from cusps.
    struct Vec0 {
        date: (i32, u32, u32, u32, u32, u32),
        sun_lon: f64,
        sun_sign: &'static str,
        moon_lon: f64,
        moon_sign: &'static str,
        phase: &'static str,
        day: &'static str,
    }

    const FIXTURES: &[Vec0] = &[
        Vec0 {
            date: (2025, 1, 15, 12, 0, 0),
            sun_lon: 295.5891,
            sun_sign: "Capricorn",
            moon_lon: 134.6889,
            moon_sign: "Leo",
            phase: "Full Moon",
            day: "Mercury",
        },
        Vec0 {
            date: (2025, 4, 15, 12, 0, 0),
            sun_lon: 25.7663,
            sun_sign: "Aries",
            moon_lon: 232.7482,
            moon_sign: "Scorpio",
            phase: "Waning Gibbous",
            day: "Mars",
        },
        Vec0 {
            date: (2025, 6, 21, 12, 0, 0),
            sun_lon: 90.3697,
            sun_sign: "Cancer",
            moon_lon: 36.1254,
            moon_sign: "Taurus",
            phase: "Waning Crescent",
            day: "Saturn",
        },
        Vec0 {
            date: (2025, 7, 10, 12, 0, 0),
            sun_lon: 108.4921,
            sun_sign: "Cancer",
            moon_lon: 284.2458,
            moon_sign: "Capricorn",
            phase: "Full Moon",
            day: "Jupiter",
        },
        Vec0 {
            date: (2025, 12, 25, 18, 0, 0),
            sun_lon: 274.2005,
            sun_sign: "Capricorn",
            moon_lon: 339.0555,
            moon_sign: "Pisces",
            phase: "Waxing Crescent",
            day: "Jupiter",
        },
        Vec0 {
            date: (2026, 6, 27, 12, 0, 0),
            sun_lon: 95.8658,
            sun_sign: "Cancer",
            moon_lon: 248.5773,
            moon_sign: "Sagittarius",
            phase: "Waxing Gibbous",
            day: "Saturn",
        },
    ];

    fn ct(d: (i32, u32, u32, u32, u32, u32)) -> CivilTime {
        CivilTime {
            year: d.0,
            month: d.1,
            day: d.2,
            hour: d.3,
            minute: d.4,
            second: d.5,
        }
    }

    /// Smallest absolute angular difference (handles 359°/1° wraparound).
    fn ang_diff(a: f64, b: f64) -> f64 {
        let d = (a - b).rem_euclid(360.0);
        d.min(360.0 - d)
    }

    #[test]
    fn labels_match_astronomy_engine() {
        for f in FIXTURES {
            let c = compute(ct(f.date));
            assert_eq!(c.sun_sign, f.sun_sign, "sun sign for {:?}", f.date);
            assert_eq!(c.moon_sign, f.moon_sign, "moon sign for {:?}", f.date);
            assert_eq!(c.moon_phase, f.phase, "moon phase for {:?}", f.date);
            assert_eq!(c.planetary_day, f.day, "planetary day for {:?}", f.date);
        }
    }

    #[test]
    fn longitudes_close_to_astronomy_engine() {
        for f in FIXTURES {
            let c = compute(ct(f.date));
            assert!(
                ang_diff(c.sun_longitude, f.sun_lon) < 0.1,
                "sun lon {:?}: got {:.4}, want {:.4}",
                f.date,
                c.sun_longitude,
                f.sun_lon
            );
            // Low-precision lunar theory with principal terms: a few arc-minutes.
            assert!(
                ang_diff(c.moon_longitude, f.moon_lon) < 0.5,
                "moon lon {:?}: got {:.4}, want {:.4}",
                f.date,
                c.moon_longitude,
                f.moon_lon
            );
        }
    }

    #[test]
    fn weekday_matches_known_dates() {
        // 2025-04-15 is a Tuesday → Mars; 2025-06-21 Saturday → Saturn.
        assert_eq!(compute(ct((2025, 4, 15, 12, 0, 0))).planetary_day, "Mars");
        assert_eq!(compute(ct((2025, 6, 21, 12, 0, 0))).planetary_day, "Saturn");
        // 2026-06-27 is a Saturday.
        assert_eq!(ct((2026, 6, 27, 0, 0, 0)).weekday(), 6);
    }

    #[test]
    fn from_unix_round_trips_a_known_instant() {
        // 2026-06-27T12:00:00Z = 1782561600 (verified below by reconstruction).
        let secs = 1_782_561_600;
        let c = CivilTime::from_unix(secs);
        assert_eq!((c.year, c.month, c.day), (2026, 6, 27));
        assert_eq!((c.hour, c.minute, c.second), (12, 0, 0));
    }

    #[test]
    fn prompt_block_has_no_date_or_time() {
        let c = compute(ct((2026, 6, 27, 12, 0, 0)));
        let block = c.prompt_block();
        // The cosmic buckets are present...
        assert!(block.contains("Zodiac Season"));
        assert!(block.contains("Moon Phase"));
        // ...but no calendar date, year, or weekday name (locating signals).
        assert!(!block.contains("2026"));
        assert!(!block.contains("June"));
        assert!(!block.contains("27"));
        assert!(!block.contains("Saturday"));
    }
}
