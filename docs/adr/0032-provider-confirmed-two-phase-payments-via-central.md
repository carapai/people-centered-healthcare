# Non-cash payments are two-phase and confirmed only by provider evidence; provider integrations run only at central

**Cash** is final at the till and posted within a cashier shift.

**Mobile money, card and bank payments** are posted as a **pending** transfer against a payments-in-transit clearing account.
- **Confirmation.** The transfer is posted only on provider evidence: a callback, a status poll or a statement line. Otherwise it is voided or expires.
- **Receipts** show "provisional" until then.

**Idempotency key.** The provider reference (e.g. MTN MoMo's UUIDv4 `X-Reference-Id`) is generated and persisted in the same transaction as the pending entry, before initiation. Recovery always queries by that reference and never re-initiates.

**Provider integrations run only at central.**
- Callbacks need a registered HTTPS domain, and MTN sends each callback only once, with no retries.
- An edge asks central to initiate, and the outcome syncs back.
- Offline, a facility can record a customer-reported payment (the reference from the customer's SMS) as pending-unverified.

**Merchant accounts.** Each facility collects into its own merchant account, or through a licensed aggregator. The platform never holds funds.

**Reconciliation.** A daily provider-statement reconciliation at central feeds an exceptions queue. Till variances at shift close are posted to a variance account and need supervisor approval.

We chose this because mobile-money confirmation is asynchronous and callbacks are unreliable. §68 also forbids treating provisional offline payments as confirmed, and holding funds for facilities would likely need a Bank of Uganda payment-service licence.

## Considered Options

- **Initiating provider payments from each edge.** Rejected: an edge has no registered callback domain, and every facility would need its own API onboarding.
- **Treating a "202 Accepted" or a customer's SMS as payment.** Rejected by §68.
- **Platform-level collection with settlement to facilities.** Rejected for v1: it is a regulated payment service.

## Consequences

- An edge that is offline cannot initiate mobile-money payments.
- The pending → posted / voided transitions sync as ledger operations.
