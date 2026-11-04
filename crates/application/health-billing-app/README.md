# health-billing-app

Layer: **application** (billing). Charge capture at the facility authority (billing-and-payments.md § Charge capture, § The ledger; ADRs 0030, 0031; #121).

- **`Billing::capture(event, triggers, coverages, applicability)`** does the following in one transaction:
  - captures the event's charges under the triggers;
  - prices them together through the six stages (`health-pricing`), so a bundle among them applies;
  - keeps each charge once by its deterministic id;
  - appends a balanced posting per charge: the patient's and each payer's `receivable/{funding source}` debited, and `revenue/{service class}` credited.

  The dimensions carry the facility, item, funding source, Account and price versions.
- **Replays never double-charge.** A replayed event finds its charges already kept and posts nothing. The same event captured on another node gives the same charges, so sync converges on one.
- **Free care:** charges whose trigger names a funding source are priced from the reference-tariff scopes, with that source paying in full. The patient share is zero and the consumption is still posted.
- **Bundles:** bundled charges are priced at zero and not posted. The bundle line becomes a charge of its own, with an id derived from the event and the bundle line.
- **`cancel_event`:** cancels the event's charges. Each posted one is reversed once, through the ledger's own reversal, and cancelling again changes nothing.
- **Financial Gates** (#122; ADR 0033): `Billing` implements `health-billing-contract`'s `FinancialGates` over the gates given to `with_gates`.
  - **What is owed:** the patient share of the Account's owed charges, less payments settled against it and waivers.
  - **Facts read:** whether a deferral was recorded at that gate.
  - **Unknown gate:** a gate not configured here is unavailable, and the engine meets that by deferring.
  - **`settle`:** counts a posted payment, or part of one, towards an Account, never beyond the payment's amount.
  - **`defer`:** records who let unpaid care go ahead, and why.
  - **`waive`:** posts `waiver/patient` against `receivable/patient` for no more than is owed. Who may defer and waive is the caller's permission check.
  - **Preauthorizations:** none counts as approved until coverage (#125).
- **Coverage terms** come from the caller (coverage), each naming its funding source.

## Tests

`tests/capture.rs` (over SurrealDB):

- an event posted once however often it is replayed;
- the same event on two nodes giving the same charges;
- a coverage split and free care at the reference tariff, as postings;
- reversal on cancellation, once.
