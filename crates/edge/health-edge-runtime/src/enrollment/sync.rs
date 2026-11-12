//! The edge as its desktops' sync server (synchronization.md: the edge serves the desktops under
//! it with the logic central serves it; #133). Desktops are not provisioned nodes: each is an
//! enrolled LAN device (ADR 0062), so `/sync/v1` sits behind the device gate and a request's peer
//! is the device the gate verified. A DPoP proof does not sign the body; the body's integrity
//! rests on the LAN's TLS, as every other gated route's does.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::Router;
use axum::http::{Extensions, HeaderMap, Method, Uri};
use health_auth::{DeviceGate, EnrolledDevice, SurrealAuthStore};
use health_identifiers::NodeId;
use health_identity::Device;
use health_surrealdb::{SurrealStore, SyncScope};
use health_sync_protocol::{Compatibility, PROTOCOL};
use health_sync_server::{AuthenticatedPeer, PeerAuth, PeerRefused, SyncServer};

/// The edge's desktops: each request's peer is the enrolled device the gate put in its
/// extensions, scoped to the edge's facility.
pub(crate) struct EnrolledDesktops {
    /// The facility this edge serves.
    pub(crate) facility: String,
}

impl EnrolledDesktops {
    fn peer(&self, device: Option<&Device>) -> Result<AuthenticatedPeer, PeerRefused> {
        let device = device
            .ok_or_else(|| PeerRefused::Unauthorized("an enrolled device is required".into()))?;
        let id: NodeId = device.id.to_string().parse().map_err(|_| {
            PeerRefused::Unavailable("an enrolled device's id is not a node id".into())
        })?;
        Ok(AuthenticatedPeer {
            id,
            scope: SyncScope::Facility(self.facility.clone()),
        })
    }
}

impl PeerAuth for EnrolledDesktops {
    fn authenticate<'a>(
        &'a self,
        _method: &'a Method,
        _uri: &'a Uri,
        _headers: &'a HeaderMap,
        extensions: &'a Extensions,
        _body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<AuthenticatedPeer, PeerRefused>> + Send + 'a>> {
        Box::pin(std::future::ready(
            self.peer(extensions.get::<EnrolledDevice>().map(|e| &e.device)),
        ))
    }
}

/// `/sync/v1` over `store` for the desktops `gate` admits, as members of `facility`; or why
/// there is none.
pub(crate) fn routes(
    store: SurrealStore,
    gate: &DeviceGate<SurrealAuthStore>,
    facility: &str,
) -> Result<Router, String> {
    let compatibility = Compatibility {
        protocol: PROTOCOL,
        software: env!("CARGO_PKG_VERSION")
            .parse()
            .map_err(|_| "the package version is not major.minor.patch")?,
        approved: std::collections::BTreeSet::new(),
    };
    let auth = Arc::new(EnrolledDesktops {
        facility: facility.to_owned(),
    });
    Ok(gate.layer(SyncServer::with_auth(store, auth, compatibility).router()))
}

#[cfg(test)]
mod tests {
    use health_identity::{DeviceId, DeviceStatus};
    use health_time::Timestamp;
    use uuid::Uuid;

    use super::*;

    fn desktops() -> EnrolledDesktops {
        EnrolledDesktops {
            facility: "KAM013".into(),
        }
    }

    #[test]
    fn a_request_without_an_enrolled_device_is_refused() {
        assert!(matches!(
            desktops().peer(None),
            Err(PeerRefused::Unauthorized(_))
        ));
    }

    #[test]
    fn the_enrolled_device_is_the_peer_in_the_edges_facility() {
        let id = Uuid::now_v7();
        let device = Device {
            id: DeviceId(id),
            thumbprint: "thumb".into(),
            name: "Desk 1".into(),
            location: "OPD".into(),
            label: "desktop".into(),
            enrolled_by: "admin".into(),
            enrolled_at: Timestamp::UNIX_EPOCH,
            status: DeviceStatus::Active,
            revoked: None,
            version: 1,
        };
        let peer = desktops().peer(Some(&device)).expect("peer");
        assert_eq!(peer.id.to_string(), id.to_string());
        assert_eq!(peer.scope, SyncScope::Facility("KAM013".into()));
    }
}
