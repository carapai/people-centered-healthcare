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
- **Coverage terms** come from the caller (coverage), each naming its funding source.

## Tests

`tests/capture.rs` (over SurrealDB):

- an event posted once however often it is replayed;
- the same event on two nodes giving the same charges;
- a coverage split and free care at the reference tariff, as postings;
- reversal on cancellation, once.
