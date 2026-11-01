//! Payments and cashier shifts over HTTP (#254; billing-and-payments.md § Payments; ADR 0032),
//! behind `subject_layer`, for the signed-in facility. Cashiers need [`CASH_PERMISSION`];
//! approving a till variance, confirming by a statement and voiding need
//! [`SUPERVISE_PERMISSION`]; refunds need [`REFUND_PERMISSION`].
//!
//! - `POST /shifts` `{till, float}`, `GET /shifts/{id}` (with its payments),
//!   `GET /shifts/open?till=`, `POST /shifts/{id}/close` `{counted}`, `POST /shifts/{id}/approve`;
//! - `POST /cash` `{shift, amount, payer, deposit?}`: final, with its receipt;
//! - `POST /pending` `{shift?, method, amount, payer, reference, deposit?}`: provisional until
//!   provider evidence;
//! - `GET /payments/{id}`, `POST /payments/{id}/void` `{reason}`,
//!   `POST /payments/{id}/refund` `{amount}`;
//! - `POST /statements` `{lines: [{reference, amount}]}`: confirms the pending payments a
//!   provider statement matches, and returns the exceptions;
//! - `GET /balances`: the till, in-transit and merchant accounts.

use std::str::FromStr;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use health_access_policy::Subject;
use health_financial_ledger::{ChartTemplate, Currency, IN_TRANSIT, TILL_VARIANCE};
use health_identifiers::NumberFormat;
use health_payments::{
    Method, NewPayment, Payment, Purpose, ReceiptStatus, Shift, ShiftStatus, StatementLine, Status,
};
use health_payments_app::{Accounts, Payments, PaymentsError, Settings};
use health_surrealdb::SurrealStore;
use health_time::{SignedDuration, Timestamp};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::FhirServer;

/// The permission to take payments and run a shift.
pub const CASH_PERMISSION: &str = "payments.cash";
/// The permission to approve variances, confirm by statement and void.
pub const SUPERVISE_PERMISSION: &str = "payments.supervise";
/// The permission to refund.
pub const REFUND_PERMISSION: &str = "payments.refund";

/// How this node takes payments: the currency, the tills, and a merchant account per non-cash
/// method.
#[derive(Debug, Clone)]
pub struct PaymentConfig {
    /// The functional currency.
    pub currency: Currency,
    /// The tills (each with a `cash/{till}` account).
    pub tills: Vec<String>,
    /// This node's short code, in receipt numbers.
    pub node: String,
    /// How long a pending payment waits for evidence.
    pub pending_for: SignedDuration,
}

impl PaymentConfig {
    fn settings(&self, facility: &str) -> Result<Settings, Box<Response>> {
        let merchant = |name: &str| format!("merchant-{name}");
        let methods = [
            (Method::MtnMomo, "mtn"),
            (Method::AirtelMoney, "airtel"),
            (Method::Card, "card"),
            (Method::BankTransfer, "bank"),
            // Reported from the customer's SMS: held apart until the statement shows where it is.
            (Method::CustomerReported, "reported"),
        ];
        let mut tills = self.tills.clone();
        tills.extend(methods.iter().map(|(_, name)| merchant(name)));
        Ok(Settings {
            facility: facility.parse().map_err(|_| {
                refused(
                    StatusCode::FORBIDDEN,
                    "the session's facility has no short code",
                )
            })?,
            node: self.node.parse().map_err(|_| {
                refused(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "the node code is not valid",
                )
            })?,
            currency: self.currency,
            chart: ChartTemplate {
                funding_sources: vec!["patient".into()],
                tills,
                ..ChartTemplate::default()
            },
            accounts: Accounts {
                receivable: "receivable/patient".into(),
                merchants: methods
                    .iter()
                    .map(|(method, name)| (*method, format!("cash/{}", merchant(name))))
                    .collect(),
            },
            receipts: NumberFormat {
                prefix: "RCT".parse().ok(),
                padding: 6,
                ..NumberFormat::default()
            },
        })
    }
}

fn refused(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn failed(error: &PaymentsError) -> Response {
    match error {
        PaymentsError::NotFound(_) => refused(StatusCode::NOT_FOUND, &error.to_string()),
        PaymentsError::DuplicateReference(_) | PaymentsError::ShiftOpen(_) => {
            refused(StatusCode::CONFLICT, &error.to_string())
        }
        PaymentsError::Store(_) => refused(StatusCode::SERVICE_UNAVAILABLE, "try again"),
        _ => refused(StatusCode::UNPROCESSABLE_ENTITY, &error.to_string()),
    }
}

type Service = Payments<SurrealStore>;

/// The member of staff with `permission`, and the use cases for their facility.
fn staff(
    server: &FhirServer,
    subject: Option<Extension<Subject>>,
    permission: &str,
) -> Result<(Subject, Service), Box<Response>> {
    let Some(config) = server.payments.clone() else {
        return Err(Box::new(refused(
            StatusCode::NOT_FOUND,
            "payments are not enabled",
        )));
    };
    let Some(Extension(subject)) = subject else {
        return Err(Box::new(refused(StatusCode::UNAUTHORIZED, "sign in")));
    };
    if !subject.permissions.has(permission) {
        return Err(Box::new(refused(
            StatusCode::FORBIDDEN,
            &format!("this needs the {permission} permission"),
        )));
    }
    let settings = config.settings(&subject.facility)?;
    Ok((subject, Payments::new(server.store.clone(), settings)))
}

macro_rules! staff_or_return {
    ($server:expr, $subject:expr, $permission:expr) => {
        match staff(&$server, $subject, $permission) {
            Ok(found) => found,
            Err(response) => return *response,
        }
    };
}

fn status_code(status: Status) -> &'static str {
    match status {
        Status::Pending => "pending",
        Status::PendingUnverified => "pending-unverified",
        Status::Posted => "posted",
        Status::Voided => "voided",
        Status::Expired => "expired",
        Status::Reversed => "reversed",
    }
}

fn method_code(method: Method) -> &'static str {
    match method {
        Method::Cash => "cash",
        Method::MtnMomo => "mtn-momo",
        Method::AirtelMoney => "airtel-money",
        Method::Card => "card",
        Method::BankTransfer => "bank-transfer",
        Method::CustomerReported => "customer-reported",
    }
}

fn method_of(code: &str) -> Option<Method> {
    [
        Method::MtnMomo,
        Method::AirtelMoney,
        Method::Card,
        Method::BankTransfer,
        Method::CustomerReported,
    ]
    .into_iter()
    .find(|m| method_code(*m) == code)
}

fn payment_json(p: &Payment) -> Value {
    json!({
        "id": p.id,
        "method": method_code(p.method),
        "deposit": p.purpose == Purpose::Deposit,
        "amount": p.amount.to_string(),
        "payer": p.payer,
        "reference": p.reference,
        "shift": p.shift,
        "status": status_code(p.status),
        "receipt": {
            "number": p.receipt,
            "status": match p.receipt() {
                ReceiptStatus::Provisional => "provisional",
                ReceiptStatus::Confirmed => "confirmed",
                ReceiptStatus::Void => "void",
            },
        },
        "refunded": p.refunded.to_string(),
        "recorded_at": p.recorded_at.to_string(),
    })
}

fn shift_json(s: &Shift) -> Value {
    json!({
        "id": s.id,
        "till": s.till,
        "cashier": s.cashier,
        "status": match s.status {
            ShiftStatus::Open => "open",
            ShiftStatus::AwaitingApproval => "awaiting-approval",
            ShiftStatus::Settled => "settled",
        },
        "float": s.float.to_string(),
        "expected": s.expected().to_string(),
        "counted": s.counted.map(|c| c.to_string()),
        "variance": s.variance().map(|v| v.to_string()),
        "approved_by": s.approved_by,
    })
}

fn amount(text: &str) -> Result<Decimal, Box<Response>> {
    Decimal::from_str(text.trim()).map_err(|_| {
        Box::new(refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "an amount is a number",
        ))
    })
}

#[derive(Debug, Deserialize)]
struct OpenBody {
    till: String,
    #[serde(default)]
    float: Option<String>,
}

async fn open_shift(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<OpenBody>,
) -> Response {
    let (subject, service) = staff_or_return!(server, subject, CASH_PERMISSION);
    let float = match body.float.as_deref().map(amount).transpose() {
        Ok(float) => float.unwrap_or_default(),
        Err(response) => return *response,
    };
    let id = format!("shift-{}", uuid::Uuid::now_v7());
    match service
        .open_shift(
            &id,
            body.till.trim(),
            &subject.account,
            float,
            Timestamp::now(),
        )
        .await
    {
        Ok(shift) => (StatusCode::CREATED, Json(shift_json(&shift))).into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Debug, Deserialize)]
struct TillQuery {
    till: String,
}

async fn open_of(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Query(query): Query<TillQuery>,
) -> Response {
    let (_, service) = staff_or_return!(server, subject, CASH_PERMISSION);
    match service.open_shift_of(query.till.trim()).await {
        Ok(Some(shift)) => Json(shift_json(&shift)).into_response(),
        Ok(None) => refused(StatusCode::NOT_FOUND, "no open shift at this till"),
        Err(error) => failed(&error),
    }
}

async fn shift(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
) -> Response {
    let (_, service) = staff_or_return!(server, subject, CASH_PERMISSION);
    match service.shift(&id).await {
        Ok((shift, payments)) => Json(json!({
            "shift": shift_json(&shift),
            "payments": payments.iter().map(payment_json).collect::<Vec<_>>(),
        }))
        .into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Debug, Deserialize)]
struct CloseBody {
    counted: String,
}

async fn close(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<CloseBody>,
) -> Response {
    let (_, service) = staff_or_return!(server, subject, CASH_PERMISSION);
    let counted = match amount(&body.counted) {
        Ok(counted) => counted,
        Err(response) => return *response,
    };
    match service.close_shift(&id, counted).await {
        Ok(shift) => Json(shift_json(&shift)).into_response(),
        Err(error) => failed(&error),
    }
}

async fn approve(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
) -> Response {
    let (subject, service) = staff_or_return!(server, subject, SUPERVISE_PERMISSION);
    match service
        .approve_shift(&id, &subject.account, Timestamp::now())
        .await
    {
        Ok(shift) => Json(shift_json(&shift)).into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Debug, Deserialize)]
struct CashBody {
    shift: String,
    amount: String,
    payer: String,
    #[serde(default)]
    deposit: bool,
}

fn purpose(deposit: bool) -> Purpose {
    if deposit {
        Purpose::Deposit
    } else {
        Purpose::Settlement
    }
}

async fn cash(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<CashBody>,
) -> Response {
    let (_, service) = staff_or_return!(server, subject, CASH_PERMISSION);
    let value = match amount(&body.amount) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let id = format!("pay-{}", uuid::Uuid::now_v7());
    let new = NewPayment {
        id: &id,
        method: Method::Cash,
        purpose: purpose(body.deposit),
        amount: value,
        payer: body.payer.trim(),
    };
    match service.take_cash(&new, &body.shift, Timestamp::now()).await {
        Ok(payment) => (StatusCode::CREATED, Json(payment_json(&payment))).into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Debug, Deserialize)]
struct PendingBody {
    shift: Option<String>,
    method: String,
    amount: String,
    payer: String,
    reference: String,
    #[serde(default)]
    deposit: bool,
}

async fn pending(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<PendingBody>,
) -> Response {
    let (_, service) = staff_or_return!(server, subject, CASH_PERMISSION);
    let Some(method) = method_of(&body.method) else {
        return refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "an unknown payment method",
        );
    };
    let value = match amount(&body.amount) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let id = format!("pay-{}", uuid::Uuid::now_v7());
    let new = NewPayment {
        id: &id,
        method,
        purpose: purpose(body.deposit),
        amount: value,
        payer: body.payer.trim(),
    };
    let now = Timestamp::now();
    let expires = now
        + server
            .payments
            .as_ref()
            .map_or(SignedDuration::from_hours(24), |c| c.pending_for);
    match service
        .record_pending(&new, &body.reference, body.shift.as_deref(), now, expires)
        .await
    {
        Ok(payment) => (StatusCode::CREATED, Json(payment_json(&payment))).into_response(),
        Err(error) => failed(&error),
    }
}

async fn payment(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
) -> Response {
    let (_, service) = staff_or_return!(server, subject, CASH_PERMISSION);
    match service.payment(&id).await {
        Ok(payment) => Json(payment_json(&payment)).into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Debug, Deserialize)]
struct VoidBody {
    reason: String,
}

async fn void(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<VoidBody>,
) -> Response {
    let (_, service) = staff_or_return!(server, subject, SUPERVISE_PERMISSION);
    match service
        .void(&id, body.reason.trim(), Timestamp::now())
        .await
    {
        Ok(payment) => Json(payment_json(&payment)).into_response(),
        Err(error) => failed(&error),
    }
}

#[derive(Debug, Deserialize)]
struct RefundBody {
    amount: String,
}

async fn refund(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Path(id): Path<String>,
    Json(body): Json<RefundBody>,
) -> Response {
    // The refund permission is checked by the payment's rules, so its refusal is the same.
    let permitted = subject
        .as_ref()
        .is_some_and(|Extension(s)| s.permissions.has(REFUND_PERMISSION));
    let (_, service) = staff_or_return!(server, subject, CASH_PERMISSION);
    let value = match amount(&body.amount) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match service
        .refund(&id, value, permitted, Timestamp::now())
        .await
    {
        Ok(payment) => Json(payment_json(&payment)).into_response(),
        Err(PaymentsError::Payment(health_payments::PaymentRefused::NotPermitted)) => refused(
            StatusCode::FORBIDDEN,
            "refunds need the payments.refund permission",
        ),
        Err(error) => failed(&error),
    }
}

#[derive(Debug, Deserialize)]
struct LineBody {
    reference: String,
    amount: String,
}

#[derive(Debug, Deserialize)]
struct StatementBody {
    lines: Vec<LineBody>,
}

async fn statement(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
    Json(body): Json<StatementBody>,
) -> Response {
    let (_, service) = staff_or_return!(server, subject, SUPERVISE_PERMISSION);
    let mut lines = Vec::with_capacity(body.lines.len());
    for line in body.lines {
        match amount(&line.amount) {
            Ok(value) => lines.push(StatementLine {
                reference: line.reference.trim().to_owned(),
                amount: value,
            }),
            Err(response) => return *response,
        }
    }
    match service.reconcile_statement(&lines, Timestamp::now()).await {
        Ok(result) => Json(json!({
            "confirmed": result.confirm,
            "mismatched": result.mismatched.iter().map(|(id, recorded, statement)| json!({ "payment": id, "recorded": recorded.to_string(), "statement": statement.to_string() })).collect::<Vec<_>>(),
            "unknown": result.unknown,
        }))
        .into_response(),
        Err(error) => failed(&error),
    }
}

async fn balances(
    State(server): State<FhirServer>,
    subject: Option<Extension<Subject>>,
) -> Response {
    let (_, service) = staff_or_return!(server, subject, SUPERVISE_PERMISSION);
    let tills: Vec<String> = server
        .payments
        .as_ref()
        .map(|c| c.tills.iter().map(|t| format!("cash/{t}")).collect())
        .unwrap_or_default();
    let mut codes: Vec<&str> = tills.iter().map(String::as_str).collect();
    codes.extend([
        IN_TRANSIT,
        TILL_VARIANCE,
        "cash/merchant-mtn",
        "cash/merchant-airtel",
    ]);
    match service.balances(&codes).await {
        Ok(list) => Json(Value::Object(
            list.into_iter()
                .map(|(code, value)| (code, json!(value.to_string())))
                .collect(),
        ))
        .into_response(),
        Err(error) => failed(&error),
    }
}

impl FhirServer {
    /// This server, taking payments with `config` (#254).
    #[must_use]
    pub fn with_payments(mut self, config: PaymentConfig) -> Self {
        self.payments = Some(std::sync::Arc::new(config));
        self
    }

    /// The payment routes (see the module docs); mount them behind `subject_layer`. Without
    /// [`with_payments`](Self::with_payments) they answer 404.
    pub fn payments_router(&self) -> Router {
        Router::new()
            .route("/shifts", post(open_shift))
            .route("/shifts/open", get(open_of))
            .route("/shifts/{id}", get(shift))
            .route("/shifts/{id}/close", post(close))
            .route("/shifts/{id}/approve", post(approve))
            .route("/cash", post(cash))
            .route("/pending", post(pending))
            .route("/payments/{id}", get(payment))
            .route("/payments/{id}/void", post(void))
            .route("/payments/{id}/refund", post(refund))
            .route("/statements", post(statement))
            .route("/balances", get(balances))
            .with_state(self.clone())
    }
}
