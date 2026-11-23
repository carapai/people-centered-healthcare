//! Saved Visualizations and Dashboards at central (#602): kept by id, a Dashboard's shares and
//! items read back as they were saved, and saving again replaces them.

use std::sync::Arc;

use health_analytics::{SavedDashboard, SavedDashboards as _, SavedVisualization, Share};
use health_persistence::{Store as _, Transaction as _};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;

async fn central() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "central".into(),
        database: "dashboards".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

#[tokio::test]
async fn a_dashboard_and_its_visualizations_are_kept_and_replaced() {
    let store = central().await;
    let visualization = SavedVisualization {
        id: "v1".into(),
        owner: "okot".into(),
        title: "ANC 1st visits by facility".into(),
        setup: r#"{"type":"column"}"#.into(),
    };
    let mut dashboard = SavedDashboard {
        id: "d1".into(),
        owner: "okot".into(),
        title: "ANC".into(),
        shares: vec![
            Share::Level {
                level: "district".into(),
            },
            Share::Person {
                account: "a-2".into(),
                username: "e2e-wakiso".into(),
            },
        ],
        items: vec!["v1".into()],
    };
    let mut tx = store.begin().await.expect("begin");
    tx.put_visualization(&visualization).await.expect("put");
    tx.put_dashboard(&dashboard).await.expect("put");
    tx.commit().await.expect("commit");

    let mut tx = store.begin().await.expect("begin");
    assert_eq!(tx.visualizations().await.expect("read"), [visualization]);
    assert_eq!(tx.dashboards().await.expect("read"), [dashboard.clone()]);
    dashboard.shares = vec![Share::Role {
        role: "district".into(),
    }];
    dashboard.title = "ANC, Kampala".into();
    tx.put_dashboard(&dashboard).await.expect("replace");
    assert_eq!(tx.dashboards().await.expect("read"), [dashboard]);
    tx.commit().await.expect("commit");
}
