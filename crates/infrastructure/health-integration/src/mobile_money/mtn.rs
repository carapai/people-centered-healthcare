//! MTN MoMo's Collection API: the access token, a request to pay, and its status.

use std::future::Future;
use std::str::FromStr as _;
use std::sync::Mutex;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use health_time::{SignedDuration, Timestamp};
use rust_decimal::Decimal;
use serde_json::{Value, json};

/// One HTTP request to the provider.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// `GET` or `POST`.
    pub method: &'static str,
    /// The path under the API's base, e.g. `/collection/v1_0/requesttopay`.
    pub path: String,
    /// Its headers.
    pub headers: Vec<(&'static str, String)>,
    /// Its JSON body, if any.
    pub body: Option<Value>,
}

/// Why the provider's API gave no answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MomoError {
    /// It could not be reached, or did not answer in time.
    #[error("the provider could not be reached")]
    Unreachable,
    /// It answered something this adapter cannot read.
    #[error("the provider answered {0}")]
    Unexpected(String),
}

/// The provider's HTTP API: a request in, a status and its JSON (`null` when there is none) out.
pub trait MomoTransport: Send + Sync {
    /// Sends `request`.
    fn send(
        &self,
        request: Request,
    ) -> impl Future<Output = Result<(u16, Value), MomoError>> + Send;
}

/// A facility's MTN merchant account, as onboarded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtnConfig {
    /// The Collection product's subscription key (`Ocp-Apim-Subscription-Key`).
    pub subscription_key: String,
    /// The API user (a UUID MTN issues per merchant).
    pub api_user: String,
    /// The API key of that user.
    pub api_key: String,
    /// `X-Target-Environment`: `sandbox`, or the production environment MTN names, e.g.
    /// `mtnuganda`.
    pub environment: String,
    /// The currency requested in, e.g. `UGX` (the sandbox takes `EUR`).
    pub currency: String,
    /// Where MTN sends its callback; the reference is appended as `?reference=`.
    pub callback_url: Option<String>,
}

/// What became of a request to pay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Initiated {
    /// Accepted (202): the payer is being asked.
    Accepted,
    /// Refused outright, with the provider's answer: nothing will happen.
    Refused(String),
    /// No answer: it may or may not have reached the provider.
    Unknown,
}

/// What the provider says of a request to pay, by its reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderStatus {
    /// The payer has not yet approved or declined.
    Pending,
    /// Paid.
    Successful {
        /// The amount paid.
        amount: Decimal,
        /// The provider's transaction id.
        transaction: Option<String>,
    },
    /// Declined, timed out at the provider, or rejected.
    Failed {
        /// The provider's reason.
        reason: String,
    },
    /// The provider has no request with this reference.
    NotFound,
}

impl ProviderStatus {
    /// The status in a status answer or a callback body.
    pub fn from_body(body: &Value) -> Result<Self, MomoError> {
        match body["status"].as_str() {
            Some("PENDING") => Ok(Self::Pending),
            Some("SUCCESSFUL") => {
                let amount = match &body["amount"] {
                    Value::String(text) => Decimal::from_str(text).ok(),
                    Value::Number(number) => Decimal::from_str(&number.to_string()).ok(),
                    _ => None,
                }
                .ok_or_else(|| MomoError::Unexpected("a payment without its amount".into()))?;
                Ok(Self::Successful {
                    amount,
                    transaction: body["financialTransactionId"].as_str().map(str::to_owned),
                })
            }
            Some("FAILED") => Ok(Self::Failed {
                reason: body["reason"]
                    .as_str()
                    .or_else(|| body["reason"]["code"].as_str())
                    .unwrap_or("FAILED")
                    .to_owned(),
            }),
            other => Err(MomoError::Unexpected(format!("a status of {other:?}"))),
        }
    }
}

/// How long before its expiry a token is renewed.
const RENEW_BEFORE: SignedDuration = SignedDuration::from_secs(60);

/// MTN MoMo's Collection API for one merchant account.
pub struct MtnMomo<T> {
    transport: T,
    config: MtnConfig,
    token: Mutex<Option<(String, Timestamp)>>,
}

impl<T: MomoTransport> MtnMomo<T> {
    /// The API for `config`'s account, through `transport`.
    pub fn new(transport: T, config: MtnConfig) -> Self {
        Self {
            transport,
            config,
            token: Mutex::new(None),
        }
    }

    /// The account.
    pub fn config(&self) -> &MtnConfig {
        &self.config
    }

    /// The transport.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// A bearer token, renewed a minute before it expires.
    async fn token(&self, now: Timestamp) -> Result<String, MomoError> {
        if let Some((token, until)) = self
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            && now < until
        {
            return Ok(token);
        }
        let basic = STANDARD.encode(format!("{}:{}", self.config.api_user, self.config.api_key));
        let (status, body) = self
            .transport
            .send(Request {
                method: "POST",
                path: "/collection/token/".into(),
                headers: vec![
                    ("Authorization", format!("Basic {basic}")),
                    (
                        "Ocp-Apim-Subscription-Key",
                        self.config.subscription_key.clone(),
                    ),
                ],
                body: None,
            })
            .await?;
        let token = body["access_token"]
            .as_str()
            .filter(|_| status == 200)
            .ok_or_else(|| MomoError::Unexpected(format!("{status} to a token request")))?
            .to_owned();
        let lifetime = SignedDuration::from_secs(body["expires_in"].as_i64().unwrap_or(3600));
        let until = now
            .checked_add(lifetime)
            .and_then(|t| t.checked_sub(RENEW_BEFORE))
            .unwrap_or(now);
        *self
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((token.clone(), until));
        Ok(token)
    }

    fn headers(&self, token: &str) -> Vec<(&'static str, String)> {
        vec![
            ("Authorization", format!("Bearer {token}")),
            ("X-Target-Environment", self.config.environment.clone()),
            (
                "Ocp-Apim-Subscription-Key",
                self.config.subscription_key.clone(),
            ),
        ]
    }

    /// Asks `msisdn` to pay `amount`, under `reference` (a UUIDv4), for `external_id` (the
    /// payment's id), with `note` shown to the payer.
    pub async fn request_to_pay(
        &self,
        reference: &str,
        amount: Decimal,
        msisdn: &str,
        external_id: &str,
        note: &str,
        now: Timestamp,
    ) -> Initiated {
        let token = match self.token(now).await {
            Ok(token) => token,
            // No token, no request: nothing reached the provider, and polling will find nothing.
            Err(MomoError::Unreachable) => return Initiated::Unknown,
            Err(MomoError::Unexpected(why)) => return Initiated::Refused(why),
        };
        let mut headers = self.headers(&token);
        headers.push(("X-Reference-Id", reference.to_owned()));
        if let Some(url) = &self.config.callback_url {
            headers.push(("X-Callback-Url", format!("{url}?reference={reference}")));
        }
        let body = json!({
            "amount": amount.normalize().to_string(),
            "currency": self.config.currency,
            "externalId": external_id,
            "payer": { "partyIdType": "MSISDN", "partyId": msisdn },
            "payerMessage": note,
            "payeeNote": note,
        });
        match self
            .transport
            .send(Request {
                method: "POST",
                path: "/collection/v1_0/requesttopay".into(),
                headers,
                body: Some(body),
            })
            .await
        {
            Ok((202, _)) => Initiated::Accepted,
            // A server error may or may not have taken it: the status tells.
            Ok((status, _)) if status >= 500 => Initiated::Unknown,
            Ok((status, answer)) => Initiated::Refused(format!("{status}: {answer}")),
            Err(_) => Initiated::Unknown,
        }
    }

    /// What the provider says of the request to pay `reference`.
    pub async fn status(
        &self,
        reference: &str,
        now: Timestamp,
    ) -> Result<ProviderStatus, MomoError> {
        let token = self.token(now).await?;
        let (status, body) = self
            .transport
            .send(Request {
                method: "GET",
                path: format!("/collection/v1_0/requesttopay/{reference}"),
                headers: self.headers(&token),
                body: None,
            })
            .await?;
        match status {
            200 => ProviderStatus::from_body(&body),
            404 => Ok(ProviderStatus::NotFound),
            status if status >= 500 => Err(MomoError::Unreachable),
            status => Err(MomoError::Unexpected(format!(
                "{status} to a status request"
            ))),
        }
    }
}

/// A fresh UUIDv4, as MTN requires for `X-Reference-Id`.
pub(crate) fn reference() -> String {
    let mut bytes = [0u8; 16];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut bytes);
    uuid::Builder::from_random_bytes(bytes)
        .into_uuid()
        .to_string()
}
