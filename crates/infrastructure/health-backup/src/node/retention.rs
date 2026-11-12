//! Which backups to keep: the 48 newest, and the newest of each of the last 14 days (UTC).

use health_time::Timestamp;

/// Backups kept regardless of age.
pub(super) const HOURLY: usize = 48;
/// Days back, today included, that keep their newest backup.
pub(super) const DAILY: i64 = 14;

const DAY: i64 = 86_400;

/// For backups `created`, newest first, whether each is kept at `now`.
pub(super) fn keep(created: &[Timestamp], now: Timestamp) -> Vec<bool> {
    let today = day(now);
    let mut last_day = None;
    created
        .iter()
        .enumerate()
        .map(|(index, at)| {
            let day = day(*at);
            let newest_of_its_day = last_day != Some(day);
            last_day = Some(day);
            index < HOURLY || (newest_of_its_day && today - day < DAILY)
        })
        .collect()
}

fn day(at: Timestamp) -> i64 {
    at.as_second().div_euclid(DAY)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hours_before(now: Timestamp, hours: i64) -> Timestamp {
        Timestamp::from_second(now.as_second() - hours * 3600).expect("a timestamp")
    }

    #[test]
    fn hourly_copies_then_one_a_day_for_two_weeks() {
        let now: Timestamp = "2026-11-01T12:30:00Z".parse().expect("timestamp");
        // Thirty days of hourly backups, newest first.
        let created: Vec<_> = (0..30 * 24).map(|h| hours_before(now, h)).collect();
        let kept: Vec<_> = created
            .iter()
            .zip(keep(&created, now))
            .filter_map(|(at, kept)| kept.then_some(*at))
            .collect();
        assert!(kept[..HOURLY].iter().eq(&created[..HOURLY]));
        // Beyond the 48 hours: the newest of each older day, back to 13 days ago.
        let older: Vec<_> = kept[HOURLY..].iter().map(|at| at.to_string()).collect();
        assert_eq!(older.len(), 11, "{older:?}");
        assert_eq!(older[0], "2026-10-29T23:30:00Z");
        assert_eq!(older[10], "2026-10-19T23:30:00Z");
    }

    #[test]
    fn a_quiet_node_keeps_everything_it_has() {
        let now: Timestamp = "2026-11-01T12:00:00Z".parse().expect("timestamp");
        let created = [hours_before(now, 1), hours_before(now, 24 * 20)];
        assert_eq!(keep(&created, now), [true, true]);
    }
}
