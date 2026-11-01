//! Phone accounts and one-time codes over SurrealDB (memory engine, migration
//! `0012_phone_sign_in`): one account per phone, and concurrent tries counted once.

use std::sync::Arc;

use health_auth::{AuthStore, PhoneOtp, SurrealAuthStore};
use health_identity::{
    AccountId, AccountStatus, OtpPolicy, OtpPurpose, PhoneAccount, PhoneNumber, SessionPolicy,
};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use uuid::Uuid;

async fn store() -> SurrealAuthStore {
    let settings = Settings {
        target: Target::Memory,
        namespace: "auth".into(),
        database: "main".into(),
    };
    let connection = Connection::open(&settings).await.expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealAuthStore::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        SessionPolicy::default().hard_cap,
    )
}

fn phone() -> PhoneNumber {
    PhoneNumber::parse("0772123456", "256").expect("phone")
}

fn at() -> Timestamp {
    Timestamp::from_second(1_800_000_000).expect("time")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_first_sign_ins_create_one_account() {
    let store = store().await;
    let mut created = tokio::task::JoinSet::new();
    for _ in 0..4 {
        let store = store.clone();
        created.spawn(async move {
            store
                .create_phone_account(PhoneAccount {
                    id: AccountId(Uuid::now_v7()),
                    phone: phone(),
                    status: AccountStatus::Active,
                    verified_at: at(),
                })
                .await
                .expect("account")
        });
    }
    let accounts = created.join_all().await;
    assert!(
        accounts.iter().all(|a| a.id == accounts[0].id),
        "{accounts:?}"
    );
    let stored = store
        .phone_account(&phone())
        .await
        .expect("read")
        .expect("account");
    assert_eq!(stored, accounts[0]);
    assert_eq!(
        store.phone_account_by_id(stored.id).await.expect("read"),
        Some(stored)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_tries_of_one_challenge_count_once() {
    let store = store().await;
    let otp = PhoneOtp {
        challenge: OtpPolicy::default().issue(
            phone(),
            OtpPurpose::SignUp,
            "salt$hash".into(),
            at(),
        ),
        sent: vec![at()],
    };
    store.put_phone_otp(&otp).await.expect("put");
    assert_eq!(
        store.phone_otp(&phone()).await.expect("read"),
        Some(otp.clone())
    );

    let mut tried = otp.clone();
    tried.challenge.attempts = 1;
    let mut tries = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let (store, tried) = (store.clone(), tried.clone());
        tries.spawn(async move { store.attempt_phone_otp(0, &tried).await.expect("try") });
    }
    let counted = tries.join_all().await.into_iter().filter(|&t| t).count();
    assert_eq!(counted, 1);
    let stored = store.phone_otp(&phone()).await.expect("read").expect("otp");
    assert_eq!(stored.challenge.attempts, 1);

    // A new code replaces the challenge, so a try against the old one is refused.
    let newer = PhoneOtp {
        challenge: OtpPolicy::default().issue(
            phone(),
            OtpPurpose::SignUp,
            "salt$other".into(),
            Timestamp::from_second(1_800_000_060).expect("time"),
        ),
        sent: vec![at()],
    };
    store.put_phone_otp(&newer).await.expect("put");
    let mut stale = stored.clone();
    stale.challenge.attempts = 2;
    assert!(!store.attempt_phone_otp(1, &stale).await.expect("try"));
}
