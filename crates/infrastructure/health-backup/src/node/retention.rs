//! Which backups to keep (ADR 0046): the newest few, then the newest of each recent day, week
//! and month (UTC). An edge or desktop keeps 48 hourly and 14 daily; central keeps 24 hourly,
//! 35 daily, 12 weekly and 12 monthly.

use health_time::Timestamp;

const DAY: i64 = 86_400;

/// How many backups of each kind are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    /// Backups kept regardless of age.
    pub hourly: usize,
    /// Days back, today included, that keep their newest backup.
    pub daily: i64,
    /// Weeks (Monday to Sunday) back, this one included, that keep their newest backup.
    pub weekly: i64,
    /// Months back, this one included, that keep their newest backup.
    pub monthly: i64,
}

impl Retention {
    /// An edge's or a standalone desktop's: 48 hourly, 14 daily.
    pub const NODE: Self = Self {
        hourly: 48,
        daily: 14,
        weekly: 0,
        monthly: 0,
    };

    /// Central's (ADR 0046): 24 hourly, 35 daily, 12 weekly, 12 monthly. The offsite copy is
    /// the operations runbook's.
    pub const CENTRAL: Self = Self {
        hourly: 24,
        daily: 35,
        weekly: 12,
        monthly: 12,
    };

    /// For backups `created`, newest first, whether each is kept at `now`.
    pub fn keep(&self, created: &[Timestamp], now: Timestamp) -> Vec<bool> {
        let (today, this_week, this_month) = (day(now), week(now), month(now));
        let (mut last_day, mut last_week, mut last_month) = (None, None, None);
        created
            .iter()
            .enumerate()
            .map(|(index, at)| {
                let (day, week, month) = (day(*at), week(*at), month(*at));
                let newest_of_day = last_day.replace(day) != Some(day);
                let newest_of_week = last_week.replace(week) != Some(week);
                let newest_of_month = last_month.replace(month) != Some(month);
                index < self.hourly
                    || (newest_of_day && today - day < self.daily)
                    || (newest_of_week && this_week - week < self.weekly)
                    || (newest_of_month && this_month - month < self.monthly)
            })
            .collect()
    }
}

fn day(at: Timestamp) -> i64 {
    at.as_second().div_euclid(DAY)
}

/// Weeks since the Monday 1970-01-05.
fn week(at: Timestamp) -> i64 {
    (day(at) - 4).div_euclid(7)
}

/// Months since January 1970.
fn month(at: Timestamp) -> i64 {
    let date = at.to_zoned(jiff::tz::TimeZone::UTC).date();
    i64::from(date.year()) * 12 + i64::from(date.month()) - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hours_before(now: Timestamp, hours: i64) -> Timestamp {
        Timestamp::from_second(now.as_second() - hours * 3600).expect("a timestamp")
    }

    fn kept(retention: Retention, created: &[Timestamp], now: Timestamp) -> Vec<Timestamp> {
        created
            .iter()
            .zip(retention.keep(created, now))
            .filter_map(|(at, kept)| kept.then_some(*at))
            .collect()
    }

    #[test]
    fn hourly_copies_then_one_a_day_for_two_weeks() {
        let now: Timestamp = "2026-11-01T12:30:00Z".parse().expect("timestamp");
        // Thirty days of hourly backups, newest first.
        let created: Vec<_> = (0..30 * 24).map(|h| hours_before(now, h)).collect();
        let kept = kept(Retention::NODE, &created, now);
        assert!(kept[..48].iter().eq(&created[..48]));
        // Beyond the 48 hours: the newest of each older day, back to 13 days ago.
        let older: Vec<_> = kept[48..].iter().map(|at| at.to_string()).collect();
        assert_eq!(older.len(), 11, "{older:?}");
        assert_eq!(older[0], "2026-10-29T23:30:00Z");
        assert_eq!(older[10], "2026-10-19T23:30:00Z");
    }

    #[test]
    fn a_quiet_node_keeps_everything_it_has() {
        let now: Timestamp = "2026-11-01T12:00:00Z".parse().expect("timestamp");
        let created = [hours_before(now, 1), hours_before(now, 24 * 20)];
        assert_eq!(Retention::NODE.keep(&created, now), [true, true]);
    }

    #[test]
    fn central_keeps_a_day_then_dailies_weeklies_and_monthlies() {
        let now: Timestamp = "2026-11-12T12:30:00Z".parse().expect("timestamp");
        // Two years of hourly backups, newest first.
        let created: Vec<_> = (0..730 * 24).map(|h| hours_before(now, h)).collect();
        let kept = kept(Retention::CENTRAL, &created, now);
        assert!(kept[..24].iter().eq(&created[..24]), "the last day, hourly");
        let days: std::collections::BTreeSet<i64> = kept.iter().map(|at| day(*at)).collect();
        // The 35 most recent days each keep one; then one a week back 12 weeks, one a month
        // back 12 months.
        for back in 0..35 {
            assert!(days.contains(&(day(now) - back)), "day {back}");
        }
        let oldest = kept.last().expect("kept");
        assert!(
            month(now) - month(*oldest) == 11,
            "a year of monthlies: {oldest}"
        );
        assert!(kept.len() < 24 + 35 + 12 + 12, "{}", kept.len());
        // Nothing older than 12 months.
        assert!(created.iter().any(|at| month(now) - month(*at) >= 12));
        assert!(kept.iter().all(|at| month(now) - month(*at) < 12));
    }
}
