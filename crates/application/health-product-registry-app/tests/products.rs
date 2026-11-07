//! Keeping the product master over SurrealDB (memory engine): a product is saved and updated, its
//! code unique and unchanging, and an invalid one refused.

use std::sync::Arc;

use health_identifiers::ResourceId;
use health_persistence::{ResourceType, Store, Transaction};
use health_product_registry::{Product, ProductRefused};
use health_product_registry_app::{ProductError, Products, product};
use health_surrealdb::{Connection, MIGRATIONS, Settings, SurrealStore, Target, migrate};
use health_time::SystemClock;

async fn store() -> SurrealStore {
    let connection = Connection::open(&Settings {
        target: Target::Memory,
        namespace: "products".into(),
        database: "main".into(),
    })
    .await
    .expect("open");
    migrate(&connection, MIGRATIONS).await.expect("migrate");
    SurrealStore::new(connection, Arc::new(SystemClock))
}

fn al(code: &str) -> Product {
    Product {
        code: code.into(),
        name: "Artemether/lumefantrine 20/120 mg".into(),
        gtin: None,
        base_unit: "tablet".into(),
        pack_sizes: vec![24],
        medicines: vec![],
        synonyms: vec![],
        active: true,
    }
}

async fn read(store: &SurrealStore, id: ResourceId) -> Option<Product> {
    let mut tx = store.begin().await.expect("begin");
    let medication: ResourceType = "Medication".parse().expect("type");
    let head = tx.read(&medication, id).await.expect("read")?;
    product(&serde_json::from_str(head.json.as_str()).expect("json"))
}

#[tokio::test]
async fn a_product_is_kept_with_a_unique_unchanging_code() {
    let store = store().await;
    let products = Products::new(store.clone());
    let id = ResourceId::new();
    products.save(id, &al("al-20-120")).await.expect("created");
    let mut renamed = al("al-20-120");
    renamed.synonyms = vec!["Coartem".into()];
    products.save(id, &renamed).await.expect("updated");
    assert_eq!(read(&store, id).await, Some(renamed));

    assert_eq!(
        products.save(ResourceId::new(), &al("al-20-120")).await,
        Err(ProductError::CodeTaken("al-20-120".into())),
        "another product with the same code"
    );
    assert_eq!(
        products.save(id, &al("al-24")).await,
        Err(ProductError::CodeChanged("al-20-120".into()))
    );
    let mut bad = al("al-6");
    bad.pack_sizes = vec![0];
    assert_eq!(
        products.save(ResourceId::new(), &bad).await,
        Err(ProductError::Refused(ProductRefused::PackSize))
    );
}
