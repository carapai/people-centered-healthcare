//! Patients' phone accounts: E.164 numbers and one-time codes (ADR 0029).

use health_identity::{InvalidPhone, OtpPolicy, OtpPurpose, OtpRefused, PhoneNumber};
use health_time::{SignedDuration, Timestamp};

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_second(1_800_000_000 + seconds).expect("time")
}

#[test]
fn phone_numbers_are_kept_in_e164() {
    for written in [
        "+256 772 123 456",
        "00256772123456",
        "0772-123-456",
        "(0772) 123.456",
    ] {
        assert_eq!(
            PhoneNumber::parse(written, "256").map(|p| p.to_string()),
            Ok("+256772123456".to_owned()),
            "{written}"
        );
    }
    assert_eq!(
        PhoneNumber::parse("0772 12345a", "256"),
        Err(InvalidPhone::NotDigits)
    );
    assert_eq!(
        PhoneNumber::parse("+1234567", "256"),
        Err(InvalidPhone::Length)
    );
    assert_eq!(
        PhoneNumber::parse("+1234567890123456", "256"),
        Err(InvalidPhone::Length)
    );
    assert_eq!(
        PhoneNumber::parse("+0772123456", "256"),
        Err(InvalidPhone::Length)
    );
    // Stored numbers deserialize only in E.164.
    assert!(PhoneNumber::try_from("+256772123456".to_owned()).is_ok());
    assert!(PhoneNumber::try_from("+256 772123456".to_owned()).is_err());
    assert!(PhoneNumber::try_from("0772123456".to_owned()).is_err());
}

#[test]
fn codes_to_one_phone_are_rate_limited() {
    let policy = OtpPolicy::default();
    assert_eq!(policy.may_send(&[], at(0)), Ok(()));
    assert_eq!(
        policy.may_send(&[at(0)], at(30)),
        Err(OtpRefused::TooSoon { until: at(60) })
    );
    assert_eq!(policy.may_send(&[at(0)], at(60)), Ok(()));
    let five: Vec<Timestamp> = (0..5).map(|i| at(i * 600)).collect();
    assert_eq!(
        policy.may_send(&five, at(3000)),
        Err(OtpRefused::TooMany { until: at(3600) })
    );
    assert_eq!(policy.may_send(&five, at(3600)), Ok(()));
}

#[test]
fn a_code_is_accepted_once_before_it_expires_and_within_its_tries() {
    let policy = OtpPolicy::default();
    let phone = PhoneNumber::parse("0772123456", "256").expect("phone");
    let mut challenge = policy.issue(phone.clone(), OtpPurpose::SignIn, "hash".into(), at(0));
    assert_eq!(challenge.expires_at, at(0) + SignedDuration::from_mins(5));
    assert_eq!(
        challenge.attempt(&policy, false, at(10)),
        Err(OtpRefused::Wrong { remaining: 4 })
    );
    assert_eq!(challenge.attempt(&policy, true, at(20)), Ok(()));
    assert_eq!(
        challenge.attempt(&policy, true, at(30)),
        Err(OtpRefused::Used)
    );

    // Expired.
    let mut late = policy.issue(phone.clone(), OtpPurpose::SignUp, "hash".into(), at(0));
    assert_eq!(
        late.attempt(&policy, true, at(300)),
        Err(OtpRefused::Expired)
    );

    // Spent after five wrong codes, even if the sixth is right.
    let mut guessed = policy.issue(phone, OtpPurpose::SignIn, "hash".into(), at(0));
    for remaining in (0..5).rev() {
        assert_eq!(
            guessed.attempt(&policy, false, at(1)),
            Err(OtpRefused::Wrong { remaining })
        );
    }
    assert_eq!(
        guessed.attempt(&policy, true, at(2)),
        Err(OtpRefused::Spent)
    );
    assert_eq!(guessed.attempts, 5);
}
