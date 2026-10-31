//! Server-sent invalidation hints (client-application.md § Ports): a hub that the use cases
//! publish to, and each subscriber's stream, filtered to what its caller may know. A hint carries
//! no record content: only a resource type and id, or a worklist key, which the client turns into
//! Query invalidations.

use std::convert::Infallible;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use serde_json::{Value, json};
use tokio::sync::broadcast;

/// Who may receive a hint: all its conditions must hold for the subscriber.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Audience {
    /// Only subscribers at this facility.
    pub facility: Option<String>,
    /// Only subscribers holding this permission code.
    pub permission: Option<String>,
}

/// One invalidation hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    /// Who may receive it.
    pub audience: Audience,
    /// The FHIR resource type that changed, if any.
    pub resource_type: Option<String>,
    /// Its id.
    pub id: Option<String>,
    /// The worklist key that changed, if any (e.g. `break-glass-alerts`).
    pub worklist: Option<String>,
}

impl Hint {
    /// A hint that `worklist` changed, for `audience`.
    pub fn worklist(worklist: &str, audience: Audience) -> Self {
        Self {
            audience,
            resource_type: None,
            id: None,
            worklist: Some(worklist.to_owned()),
        }
    }

    fn data(&self) -> Value {
        let mut data = json!({});
        for (key, value) in [
            ("resourceType", &self.resource_type),
            ("id", &self.id),
            ("worklist", &self.worklist),
        ] {
            if let Some(value) = value {
                data[key] = json!(value);
            }
        }
        data
    }
}

/// The node's hint hub. Cloning shares it.
#[derive(Debug, Clone)]
pub struct Events {
    sender: broadcast::Sender<Hint>,
}

impl Default for Events {
    fn default() -> Self {
        Self::new(1024)
    }
}

/// How often a quiet stream sends a comment, so proxies keep it open.
const KEEP_ALIVE: Duration = Duration::from_secs(25);

impl Events {
    /// A hub keeping up to `capacity` hints for slow subscribers; one that falls further behind
    /// is told to reconnect.
    pub fn new(capacity: usize) -> Self {
        Self {
            sender: broadcast::channel(capacity.max(1)).0,
        }
    }

    /// Sends `hint` to every current subscriber it is for.
    pub fn publish(&self, hint: Hint) {
        // No subscriber is not an error.
        let _ = self.sender.send(hint);
    }

    /// A `text/event-stream` response carrying the hints `admits` lets through.
    pub fn stream(&self, admits: impl Fn(&Audience) -> bool + Send + 'static) -> Response {
        let receiver = self.sender.subscribe();
        let state = (receiver, admits, 0u64);
        let stream =
            futures_util::stream::unfold(state, |(mut receiver, admits, mut seq)| async move {
                loop {
                    let next = tokio::time::timeout(KEEP_ALIVE, receiver.recv()).await;
                    let chunk = match next {
                        Err(_) => ": keep-alive\n\n".to_owned(),
                        Ok(Ok(hint)) if admits(&hint.audience) => {
                            seq += 1;
                            format!("id: {seq}\ndata: {}\n\n", hint.data())
                        }
                        Ok(Ok(_)) => continue,
                        // Fell behind: the client reconnects and refetches what it shows.
                        Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
                            "event: reset\ndata: {\"worklist\":\"*\"}\n\n".to_owned()
                        }
                        Ok(Err(broadcast::error::RecvError::Closed)) => return None,
                    };
                    return Some((
                        Ok::<Bytes, Infallible>(Bytes::from(chunk)),
                        (receiver, admits, seq),
                    ));
                }
            });
        let mut response = Response::new(Body::from_stream(stream));
        *response.status_mut() = StatusCode::OK;
        let headers = response.headers_mut();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/event-stream"),
        );
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
    }
}

#[cfg(test)]
mod tests {
    use http_body_util::BodyExt;

    use super::*;

    #[tokio::test]
    async fn subscribers_receive_only_the_hints_for_them() {
        let events = Events::default();
        let response = events.stream(|audience| audience.facility.as_deref() != Some("F2"));
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/event-stream"
        );
        let mut body = response.into_body();
        events.publish(Hint::worklist(
            "break-glass-alerts",
            Audience {
                facility: Some("F2".into()),
                permission: None,
            },
        ));
        events.publish(Hint::worklist(
            "break-glass-alerts",
            Audience {
                facility: Some("F1".into()),
                permission: None,
            },
        ));
        let frame = body.frame().await.expect("frame").expect("ok");
        let text = String::from_utf8(frame.into_data().expect("data").to_vec()).expect("utf8");
        assert_eq!(
            text,
            "id: 1\ndata: {\"worklist\":\"break-glass-alerts\"}\n\n"
        );
    }
}
