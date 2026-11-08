//! The daily: one chart a day, the same for everyone, drawn from the ranked
//! pool by the date. Deterministic, so a client can verify it and the server
//! never has to store it: the date is the seed.

use downbeat_core::chart::Level;
use downbeat_core::mods::Mods;
use downbeat_core::patterns::{fill_level, street_hard_level, street_level};

use crate::charts::Charts;

#[derive(Clone, Debug)]
pub struct Daily {
    pub date: String,
    pub level: Level,
    pub mods: Mods,
    pub fingerprint: u64,
    pub title: String,
}

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Days since 1970-01-01 for a `YYYY-MM-DD` string (proleptic Gregorian).
pub fn day_number(date: &str) -> Option<i64> {
    let mut it = date.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe - 719468)
}

/// Today's date in UTC as `YYYY-MM-DD`.
pub fn today_utc() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    civil_from_days(secs.div_euclid(86400))
}

pub fn civil_from_days(z: i64) -> String {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// The daily for a date. Pool: the shipped campaign levels (as charted) and
/// the three builtin action levels at a tempo from 120 to 180 in steps of 2;
/// a third of days carry HR, a sixth DT. Every pick is in the ranked set.
pub fn daily_for(date: &str, charts: &Charts) -> Option<Daily> {
    day_number(date)?;
    let seed = fnv(date);
    let mut shipped: Vec<&Level> = charts.levels.values().collect();
    shipped.sort_by(|a, b| a.name.cmp(&b.name));
    let pool = 3 + shipped.len() as u64;
    let pick = seed % pool;
    let bpm = 120.0 + ((seed >> 8) % 31) as f64 * 2.0;
    let roll = (seed >> 16) % 6;
    let mods = match roll {
        0 | 1 => Mods { hr: true, ..Mods::NONE },
        2 => Mods { dt: true, ..Mods::NONE },
        _ => Mods::NONE,
    };
    let (level, title) = match pick {
        0 => (street_level(bpm), format!("STREET {bpm:.0}")),
        1 => (street_hard_level(bpm), format!("STREET HARD {bpm:.0}")),
        2 => (fill_level(bpm), format!("FILL {bpm:.0}")),
        k => {
            let l = shipped[(k - 3) as usize].clone();
            let t = if l.song.is_empty() { l.name.to_uppercase() } else { l.song.to_uppercase() };
            (l, t)
        }
    };
    let fingerprint = crate::charts::fingerprint_with_mods(&level, mods)?;
    let title = if mods.label() == "NM" { title } else { format!("{title} {}", mods.label()) };
    Some(Daily { date: date.to_string(), level, mods, fingerprint, title })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip() {
        for d in ["1970-01-01", "2000-02-29", "2026-11-01", "2099-12-31"] {
            assert_eq!(civil_from_days(day_number(d).unwrap()), d);
        }
        assert_eq!(day_number("2026-11-01"), Some(20758));
        assert!(day_number("2026-13-01").is_none());
    }

    #[test]
    fn the_daily_is_deterministic_and_ranked() {
        let charts = Charts::build("/nonexistent");
        let a = daily_for("2026-11-01", &charts).unwrap();
        let b = daily_for("2026-11-01", &charts).unwrap();
        assert_eq!(a.fingerprint, b.fingerprint);
        assert!(charts.get(a.fingerprint).is_some(), "daily chart is in the ranked set");
        let c = daily_for("2026-11-02", &charts).unwrap();
        assert_ne!(a.fingerprint, c.fingerprint);
    }
}
