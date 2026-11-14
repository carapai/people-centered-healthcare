//! The re-sync limit (ADR 0046; deployment.md § Disaster recovery): after central is restored,
//! every edge re-seeds and re-pushes at once. Pushes and snapshots, the heavy requests, are
//! taken at most [`DEFAULT_CONCURRENT`] at a time across all peers and one at a time per peer.
//! Anything over is answered `429 Too Many Requests` with a `Retry-After` carrying jitter, so the
//! refused peers do not come back in step.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use health_identifiers::NodeId;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Heavy requests taken at once across all peers, unless configured.
pub const DEFAULT_CONCURRENT: usize = 8;

/// The shortest `Retry-After`, in seconds; up to as much again is added as jitter.
const RETRY_AFTER: u64 = 15;

/// How many heavy requests are in flight, and from whom.
#[derive(Debug)]
pub(crate) struct Limit {
    slots: Arc<Semaphore>,
    busy: Arc<Mutex<HashSet<NodeId>>>,
}

/// One admitted request: its slot and its peer's mark, both released when it is dropped. A
/// streamed answer holds it until the stream ends.
#[derive(Debug)]
pub(crate) struct Admitted {
    _slot: OwnedSemaphorePermit,
    peer: NodeId,
    busy: Arc<Mutex<HashSet<NodeId>>>,
}

impl Drop for Admitted {
    fn drop(&mut self) {
        if let Ok(mut busy) = self.busy.lock() {
            busy.remove(&self.peer);
        }
    }
}

impl Limit {
    /// At most `concurrent` heavy requests at once (at least one).
    pub(crate) fn new(concurrent: usize) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(concurrent.max(1))),
            busy: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Admits `peer`'s request, or the `429` to answer it with.
    pub(crate) fn admit(&self, peer: NodeId) -> Result<Admitted, Box<Response>> {
        let mut busy = self.busy.lock().map_err(|_| Box::new(too_many("busy")))?;
        if busy.contains(&peer) {
            return Err(Box::new(too_many(
                "this node already has a push or snapshot in progress",
            )));
        }
        let slot = Arc::clone(&self.slots)
            .try_acquire_owned()
            .map_err(|_| Box::new(too_many("central is taking as many re-syncs as it can")))?;
        busy.insert(peer);
        Ok(Admitted {
            _slot: slot,
            peer,
            busy: Arc::clone(&self.busy),
        })
    }
}

impl Default for Limit {
    fn default() -> Self {
        Self::new(DEFAULT_CONCURRENT)
    }
}

/// `429` with a `Retry-After` of [`RETRY_AFTER`] seconds plus up to as much again.
fn too_many(detail: &str) -> Response {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let wait = RETRY_AFTER + u64::from(nanos) % (RETRY_AFTER + 1);
    let mut response = crate::problem(StatusCode::TOO_MANY_REQUESTS, detail);
    if let Ok(value) = HeaderValue::from_str(&wait.to_string()) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(n: u8) -> NodeId {
        format!("01890a5d-ac96-774b-bcce-b302099a8{n:03}")
            .parse()
            .expect("node id")
    }

    fn retry_after(response: &Response) -> u64 {
        response.headers()[header::RETRY_AFTER]
            .to_str()
            .expect("ascii")
            .parse()
            .expect("seconds")
    }

    #[test]
    fn one_request_per_peer_at_a_time() {
        let limit = Limit::new(4);
        let first = limit.admit(node(1)).expect("admitted");
        let refused = limit
            .admit(node(1))
            .expect_err("a second from the same node");
        assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!((RETRY_AFTER..=2 * RETRY_AFTER).contains(&retry_after(&refused)));
        assert!(limit.admit(node(2)).is_ok(), "another node is admitted");
        drop(first);
        assert!(limit.admit(node(1)).is_ok(), "admitted again once done");
    }

    #[test]
    fn no_more_than_the_limit_across_peers() {
        let limit = Limit::new(2);
        let a = limit.admit(node(1)).expect("first");
        let _b = limit.admit(node(2)).expect("second");
        let refused = limit.admit(node(3)).expect_err("over the limit");
        assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
        // A refusal for the global limit leaves the node free to try again.
        drop(a);
        assert!(limit.admit(node(3)).is_ok());
    }
}
