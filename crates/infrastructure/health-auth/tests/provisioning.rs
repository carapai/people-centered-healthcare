//! Importing central's provisioning bundle on an edge (ADR 0059), over SurrealDB: the accounts
//! and snapshots are stored with each hash re-encrypted under the edge's pepper key; a bundle
//! for another facility, or a snapshot for an account it does not carry, is refused.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use health_auth::{
    AuthStore, Bundle, BundleAccount, Credentials, ImportError, Provisioning, SnapshotSigner,
    SnapshotVerifier, SurrealAuthStore,
};
use health_identity::snapshot::{PermissionSnapshot, RoleAssignment, RolePermissions};
use health_identity::{AccountId, AccountStatus, Lockout, SessionPolicy};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::{SystemClock, Timestamp};
use p256::SecretKey;
use uuid::Uuid;

const NODE: &str = "ug-kla-0001";
const FACILITY: &str = "ug-kla-mulago";

async fn store() -> SurrealAuthStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "auth".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealAuthStore::new(
        SurrealStore::new(connection, Arc::new(SystemClock)),
        SessionPolicy::default().hard_cap,
    )
}

fn roles() -> RolePermissions {
    RolePermissions(BTreeMap::from([(
        "clinician".into(),
        BTreeSet::from(["record.read".into()]),
    )]))
}

fn snapshot(account: AccountId, facility: &str) -> PermissionSnapshot {
    let issued = Timestamp::now();
    PermissionSnapshot {
        account,
        facility: facility.into(),
        roles: vec![RoleAssignment {
            role: "clinician".into(),
            locations: Vec::new(),
        }],
        programs: Vec::new(),
        issued_at: issued,
        not_after: issued
            .checked_add(health_time::SignedDuration::from_hours(24 * 14))
            .expect("time"),
        snapshot_version: 1,
    }
}

struct Central {
    signer: SnapshotSigner,
    account: AccountId,
}

impl Central {
    fn bundle(&self, facility: &str, snapshot_for: AccountId, edge: &SecretKey) -> String {
        Bundle {
            facility: facility.into(),
            node_id: NODE.into(),
            issued_at: Timestamp::now(),
            accounts: vec![BundleAccount {
                id: self.account,
                username: "nakato".into(),
                status: AccountStatus::Active,
                phc: Credentials::hash("correct horse battery").expect("hash"),
            }],
            snapshots: vec![
                self.signer
                    .sign(&snapshot(snapshot_for, facility), &roles())
                    .expect("sign"),
            ],
        }
        .seal(&self.signer, &edge.public_key())
        .expect("seal")
    }
}

#[tokio::test]
async fn an_edge_imports_its_staff_with_their_hashes_under_its_own_key() {
    let central = Central {
        signer: SnapshotSigner::generate(),
        account: AccountId(Uuid::now_v7()),
    };
    let edge_key = SecretKey::random(&mut rand_core::OsRng);
    let store = store().await;
    let credentials = Credentials::new([3; 32].to_vec()).expect("credentials");
    let recipient = {
        let key = edge_key.clone();
        Arc::new(move || Ok(key.clone()))
    };
    let provisioning = Provisioning::new(
        store.clone(),
        credentials.clone(),
        SnapshotVerifier::new(&[central.signer.public()]).expect("verifier"),
        NODE,
        FACILITY,
        recipient,
    );

    // For another facility, or with a snapshot for someone it does not carry: refused whole.
    let other = central.bundle("ug-gul-lacor", central.account, &edge_key);
    assert_eq!(
        provisioning.import(&other).await,
        Err(ImportError::WrongFacility("ug-gul-lacor".into()))
    );
    let stray = central.bundle(FACILITY, AccountId(Uuid::now_v7()), &edge_key);
    assert_eq!(
        provisioning.import(&stray).await,
        Err(ImportError::Snapshot)
    );
    assert_eq!(store.account(central.account).await.expect("read"), None);

    let imported = provisioning
        .import(&central.bundle(FACILITY, central.account, &edge_key))
        .await
        .expect("import");
    assert_eq!((imported.accounts, imported.snapshots), (1, 1));
    let account = store
        .account(central.account)
        .await
        .expect("read")
        .expect("stored");
    assert!(!account.credential.contains("argon2"), "stored encrypted");
    assert!(credentials.verify(account.id, "correct horse battery", &account.credential));
    assert!(
        store
            .snapshot(central.account, FACILITY)
            .await
            .expect("read")
            .is_some()
    );

    // Imported again, an account keeps its failed-login state.
    let mut locked = account.clone();
    locked.lockout = Lockout {
        failures: 3,
        ..Lockout::default()
    };
    store.put_account(&locked).await.expect("put");
    provisioning
        .import(&central.bundle(FACILITY, central.account, &edge_key))
        .await
        .expect("import");
    let again = store
        .account(central.account)
        .await
        .expect("read")
        .expect("stored");
    assert_eq!(again.lockout.failures, 3);
}
