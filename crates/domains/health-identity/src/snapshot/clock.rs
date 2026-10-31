use health_time::Timestamp;

/// The edge's "last known central time" (offline-auth.md § Permission snapshots): it only moves
/// forward, advanced by every snapshot, sync response and token exchange with central.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClockFloor(Option<Timestamp>);

/// The local clock reads earlier than central time already seen: logins are refused and an
/// alert is raised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the local clock reads {local}, before the last known central time {floor}")]
pub struct ClockBroken {
    /// The local reading.
    pub local: Timestamp,
    /// The floor.
    pub floor: Timestamp,
}

impl ClockFloor {
    /// A floor at `at`, as persisted.
    pub fn at(at: Timestamp) -> Self {
        Self(Some(at))
    }

    /// The floor, if any central time has been seen.
    pub fn get(&self) -> Option<Timestamp> {
        self.0
    }

    /// Records a central time; the floor never moves back.
    #[must_use]
    pub fn observe(self, central: Timestamp) -> Self {
        Self(Some(self.0.map_or(central, |floor| floor.max(central))))
    }

    /// The time to check expiry against: the local clock, unless it reads before the floor.
    pub fn check(&self, local: Timestamp) -> Result<Timestamp, ClockBroken> {
        match self.0 {
            Some(floor) if local < floor => Err(ClockBroken { local, floor }),
            _ => Ok(local),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_floor_only_moves_forward_and_catches_a_clock_set_back() {
        let t = |s| Timestamp::from_second(s).unwrap_or(Timestamp::UNIX_EPOCH);
        let floor = ClockFloor::default().observe(t(100)).observe(t(50));
        assert_eq!(floor.get(), Some(t(100)));
        assert_eq!(floor.check(t(120)), Ok(t(120)));
        assert_eq!(
            floor.check(t(99)),
            Err(ClockBroken {
                local: t(99),
                floor: t(100)
            })
        );
        assert_eq!(ClockFloor::default().check(t(1)), Ok(t(1)));
        assert_eq!(ClockFloor::at(t(5)).get(), Some(t(5)));
    }
}
