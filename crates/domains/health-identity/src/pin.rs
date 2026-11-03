//! The PIN (offline-auth.md § Credentials): 6 digits, used only to resume or switch to a user who
//! already has a live session on that device, never to start one.

/// The PIN rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinPolicy {
    /// Digits in a PIN.
    pub digits: usize,
    /// Wrong PINs after which the user's session on the device ends, so only a password can
    /// sign them in again.
    pub max_failures: u32,
}

impl Default for PinPolicy {
    /// Six digits; five wrong PINs end the session.
    fn default() -> Self {
        Self {
            digits: 6,
            max_failures: 5,
        }
    }
}

impl PinPolicy {
    /// Whether `pin` is a PIN at all: exactly the policy's number of ASCII digits.
    pub fn valid(&self, pin: &str) -> bool {
        pin.len() == self.digits && pin.bytes().all(|b| b.is_ascii_digit())
    }

    /// Whether `failures` wrong PINs end the session.
    pub fn exhausted(&self, failures: u32) -> bool {
        failures >= self.max_failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pin_is_six_digits_and_five_wrong_ones_end_the_session() {
        let policy = PinPolicy::default();
        assert!(policy.valid("042137"));
        for bad in ["42137", "0421370", "04213a", " 42137", ""] {
            assert!(!policy.valid(bad), "{bad:?}");
        }
        assert!(!policy.exhausted(4));
        assert!(policy.exhausted(5));
    }
}
