use std::sync::Mutex;

use health_pki::{Authority, Facility, IssuingCa, inspect, new_key, request};
use health_time::Timestamp;

use super::*;
use crate::tls::Certificate;

const NODE: &str = "ug-kla-0001";

fn pairing(pki_dir: PathBuf, origins: &[&str]) -> Pairing {
    Pairing {
        node_id: NODE.into(),
        origins: origins.iter().map(|o| (*o).to_owned()).collect(),
        pki_dir,
    }
}

/// Installs a chain for this node, as `provision install` does; the intermediate's PEM.
fn install(pki_dir: &std::path::Path) -> String {
    let now = Timestamp::now();
    let root = Authority::root("Test Root", now).expect("root");
    let issuing_key = new_key().expect("key");
    let issuing = root
        .issue_issuing_ca(
            &request(&issuing_key, "issuing").expect("csr"),
            &IssuingCa {
                common_name: "Test Issuing CA".into(),
                dns_suffixes: Vec::new(),
            },
            now,
        )
        .expect("issuing");
    let issuing =
        Authority::from_pem(&issuing.pem(), &issuing_key.serialize_pem()).expect("issuing");
    let facility = issuing
        .issue_facility(
            &request(&new_key().expect("key"), NODE).expect("csr"),
            &Facility {
                node_id: NODE.into(),
                dns_names: Vec::new(),
            },
            now,
        )
        .expect("facility");
    std::fs::create_dir_all(pki_dir).expect("pki");
    std::fs::write(pki_dir.join(ROOT), root.certificate_pem()).expect("root");
    std::fs::write(
        pki_dir.join(CHAIN),
        format!("{}{}", facility.pem(), issuing.certificate_pem()),
    )
    .expect("chain");
    facility.pem()
}

#[test]
fn the_pairing_payload_names_the_url_the_node_and_its_intermediates_pin() {
    let dir = tempfile::tempdir().expect("dir");
    let pki_dir = dir.path().join("pki");
    let origins = ["http://127.0.0.1:8080", "https://ug-kla-0001.local"];
    assert_eq!(
        pairing(pki_dir.clone(), &origins)
            .payload()
            .map_err(|e| e.0),
        Err("not-provisioned")
    );

    let intermediate = install(&pki_dir);
    let pin = inspect(&intermediate).expect("inspect").spki_sha256;
    let payload = pairing(pki_dir.clone(), &origins)
        .payload()
        .expect("payload");
    assert_eq!(payload["version"], PROTOCOL);
    assert_eq!(payload["node_id"], NODE);
    assert_eq!(
        payload["url"], "https://ug-kla-0001.local",
        "the https origin"
    );
    assert_eq!(payload["spki"], pin.as_str());
    let encoded_pin = pin
        .replace('/', "%2F")
        .replace('+', "%2B")
        .replace('=', "%3D");
    assert_eq!(
        payload["uri"],
        format!(
            "health-edge://pair?v=1&node=ug-kla-0001&url=https%3A%2F%2Fug-kla-0001.local&spki={encoded_pin}"
        )
    );

    assert_eq!(
        pairing(pki_dir, &["http://127.0.0.1:8080"])
            .payload()
            .map_err(|e| e.0),
        Err("no-https-origin")
    );
}

#[test]
fn the_announcement_carries_the_node_and_the_protocol_version() {
    let addresses: Vec<IpAddr> = vec!["192.168.1.10".parse().expect("ip")];
    let info = service(NODE, &addresses, 443).expect("service");
    assert_eq!(info.get_fullname(), "ug-kla-0001._health-edge._tcp.local.");
    assert_eq!(info.get_hostname(), "ug-kla-0001.local.");
    assert_eq!(info.get_port(), 443);
    assert_eq!(info.get_property_val_str("node"), Some(NODE));
    assert_eq!(info.get_property_val_str("v"), Some(PROTOCOL));
    assert!(info.get_addresses().contains(&addresses[0]));
}

/// A client on this machine hears the announcement.
#[tokio::test]
async fn a_browser_on_the_lan_finds_the_node() {
    // No addresses of its own: it announces every interface's, as a wildcard-bound node does.
    let state = Arc::new(Mutex::new(Certificate::unprovisioned()));
    let announcing = tokio::spawn(announce(
        "ug-kla-0002".into(),
        8443,
        state,
        Duration::from_secs(3600),
    ));
    let browser = ServiceDaemon::new().expect("browser");
    let events = browser.browse(SERVICE).expect("browse");
    let found = tokio::task::spawn_blocking(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
            match events.recv_timeout(left) {
                Ok(mdns_sd::ServiceEvent::ServiceResolved(found))
                    if found.get_property_val_str("node") == Some("ug-kla-0002") =>
                {
                    return Some(found.get_port());
                }
                Ok(_) => {}
                Err(_) => return None,
            }
        }
        None
    })
    .await
    .expect("browse");
    announcing.abort();
    let _ = browser.shutdown();
    assert_eq!(found, Some(8443));
}
