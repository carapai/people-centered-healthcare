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
//!   3. A callback or a poll that says SUCCESSFUL confirms it, with that evidence, only when the
//!      amount is the payment's. FAILED voids it, and PENDING waits.
//!   4. Confirming twice, or a callback racing a poll, posts once: a payment that is no longer
//!      pending is reported as already settled.
//!   5. A request the provider never received (not found once the grace has passed) is voided,
//!      since it is never sent again.
//!
//! **Facility merchant accounts only:** each facility collects into its own MTN account (or a
//! licensed aggregator's), configured per facility; the platform never holds funds.
//!
//! [`Payments`]: health_payments_app::Payments

mod collections;
mod mtn;

pub use collections::{CollectionError, Collections, Settled};
pub use mtn::{Initiated, MomoError, MomoTransport, MtnConfig, MtnMomo, ProviderStatus, Request};
