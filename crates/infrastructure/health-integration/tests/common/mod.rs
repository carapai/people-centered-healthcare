//! A mock MTN MoMo for the tests (#124): the requests to pay it holds, by reference, every
//! call made to it, and how it answers the next request.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use health_integration::mobile_money::{MomoError, MomoTransport, Request};
use serde_json::{Value, json};

/// How the mock answers the next request to pay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Next {
    Accept,
    /// Takes it, but the answer is lost on the way back.
    LoseAnswer,
    Refuse,
}

/// MTN as the tests see it: the requests to pay it holds, by reference, and every call.
#[derive(Clone)]
pub(crate) struct Mock {
    pub(crate) held: Arc<Mutex<BTreeMap<String, Value>>>,
    pub(crate) calls: Arc<Mutex<Vec<String>>>,
    pub(crate) next: Arc<Mutex<Next>>,
}

impl Mock {
    pub(crate) fn new() -> Self {
        Self {
            held: Arc::default(),
            calls: Arc::default(),
            next: Arc::new(Mutex::new(Next::Accept)),
        }
    }

    pub(crate) fn next(&self, next: Next) {
        *self.next.lock().expect("next") = next;
    }

    /// The payer answers the request `reference`: MTN's status body for it now.
    pub(crate) fn answer(&self, reference: &str, status: &str, amount: &str) -> Value {
        let mut held = self.held.lock().expect("held");
        let request = held.get_mut(reference).expect("held request");
        request["status"] = json!(status);
        request["amount"] = json!(amount);
        if status == "SUCCESSFUL" {
            request["financialTransactionId"] = json!("123456789");
        } else if status == "FAILED" {
            request["reason"] = json!("APPROVAL_REJECTED");
        }
        request.clone()
    }

    pub(crate) fn count(&self, prefix: &str) -> usize {
        self.calls
            .lock()
            .expect("calls")
            .iter()
            .filter(|c| c.starts_with(prefix))
            .count()
    }
}

impl MomoTransport for Mock {
    async fn send(&self, request: Request) -> Result<(u16, Value), MomoError> {
        self.calls
            .lock()
            .expect("calls")
            .push(format!("{} {}", request.method, request.path));
        let header = |name: &str| {
            request
                .headers
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.clone())
        };
        if request.path == "/collection/token/" {
            assert!(header("Authorization").is_some_and(|a| a.starts_with("Basic ")));
            return Ok((
                200,
                json!({ "access_token": "token-1", "expires_in": 3600 }),
            ));
        }
        assert_eq!(header("Authorization").as_deref(), Some("Bearer token-1"));
        assert_eq!(header("X-Target-Environment").as_deref(), Some("sandbox"));
        if request.method == "POST" {
            let reference = header("X-Reference-Id").expect("reference");
            assert_eq!(
                uuid::Uuid::parse_str(&reference)
                    .expect("uuid")
                    .get_version_num(),
                4
            );
            let next = *self.next.lock().expect("next");
            if next == Next::Refuse {
                return Ok((400, json!({ "code": "PAYER_NOT_FOUND" })));
            }
            let body = request.body.expect("body");
            let mut held = body.clone();
            held["status"] = json!("PENDING");
            self.held.lock().expect("held").insert(reference, held);
            return match next {
                Next::LoseAnswer => Err(MomoError::Unreachable),
                _ => Ok((202, Value::Null)),
            };
        }
        let reference = request.path.rsplit('/').next().expect("reference");
        match self.held.lock().expect("held").get(reference) {
            Some(held) => Ok((200, held.clone())),
            None => Ok((404, json!({ "code": "RESOURCE_NOT_FOUND" }))),
        }
    }
}
