//! Mobile money at central (billing-and-payments.md § Payments, ADR 0032; #124). Provider
//! integrations run only at central: an edge asks central to initiate, and the outcome syncs back.
//!
//! - **[`MtnMomo`]**: MTN MoMo's Collection API, through a [`MomoTransport`] (bound to an HTTP
//!   client when a facility's merchant account is onboarded, a mock in the tests). A request to
//!   pay carries a fresh UUIDv4 `X-Reference-Id`; its status is read back by that reference, and
//!   a callback carries the same body.
//! - **[`Collections`]**: the flow over `health-payments-app`'s [`Payments`]:
//!   1. The pending payment is stored with its reference **before** the request goes out, so a
//!      crash or a lost answer leaves a payment to recover, never a charge with no record.
//!   2. A refused request voids it. One whose answer was lost stays pending: it is polled, and
//!      never sent again.
//!   3. A callback is only a hint: it reaches a public route, so its body is never trusted. The
//!      status is read back from MTN by the reference, as a poll does. SUCCESSFUL confirms the
//!      payment, with that evidence, only when the amount is the payment's; another amount opens
//!      an exception. FAILED voids it, and PENDING waits.
//!   4. Confirming twice, or a callback racing a poll, posts once: a payment that is no longer
//!      pending is reported as already settled.
//!   5. A request the provider never received (not found once the grace has passed) is voided,
//!      since it is never sent again.
//!
//! 6. A provider statement is reconciled with the pending payments
//!    ([`Collections::reconcile_statement`]): those it shows are confirmed, and a different
//!    amount or a line no payment has opens an exception in the queue for a person.
//!
//! **Facility merchant accounts only:** each facility collects into its own MTN account (or a
//! licensed aggregator's), configured per facility; the platform never holds funds.
//!
//! [`Payments`]: health_payments_app::Payments

mod central;
mod collections;
mod https;
mod mtn;

pub use central::{MomoAccounts, MomoService};
pub use collections::{CollectionError, Collections, Settled};
pub use https::HttpsTransport;
pub use mtn::{Initiated, MomoError, MomoTransport, MtnConfig, MtnMomo, ProviderStatus, Request};
