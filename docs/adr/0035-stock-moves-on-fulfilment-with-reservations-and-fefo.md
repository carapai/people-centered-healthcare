# Stock moves only on fulfilment; preparation reserves stock, and FEFO with policy chooses the batch

**What moves stock.** A MedicationRequest never touches stock.
1. Pharmacy preparation creates a **Reservation**: an allocation record with a lifetime (4 h for dispense preparation, 72 h for transfer picks). It reduces *available* but never *on-hand*.
2. Completing the MedicationDispense posts the dispense movement in the same transaction, through inventory's own use case (ADR 0004), and fires the charge trigger.
3. Partial dispenses post partial movements.
4. Patient returns go to quarantine by default.

**Batch selection (FEFO).** The earliest-expiry Stock Unit that is usable, not on hold and eligible under configured funding-source rules, and whose expiry is after the end of the course plus a buffer. Ties go to the smallest quantity. An override needs a reason.

**Substitution:**
- generic equivalence is allowed by policy;
- brand or pack substitution needs a pharmacist;
- therapeutic substitution needs the prescriber.

**Ward and lab consumption:**
- **Ward stores** either track consumption at administration (MedicationAdministration) or treat the issue to the ward as consumption, by configuration.
- **Lab consumables** default to consumption derived from periodic counts.

**Transfers** are always a dispatch to in-transit plus a confirmed receipt. Variances raise Reconciliation Records.

We chose this because §73 requires stock to follow actual fulfilment, and WHO GSDP §12.30 requires FEFO. Reservations prevent two dispensers from promising the same last packs without creating phantom movements.

## Considered Options

- **Decrementing stock at prescription.** Rejected by §73. Prescriptions are often not dispensed, or are dispensed elsewhere.
- **Posting a movement at preparation and reversing it on cancellation.** Rejected: unfilled preparations would distort the ledger and consumption reports.
- **Strict FEFO with no eligibility rules.** Rejected: donor and program stock must be kept for eligible patients.

## Consequences

- Reservations expire and are released automatically, so abandoned preparations cannot cause phantom stock-outs.
- Unleased offline dispensing from an edge's store is allowed by default as candidate operations, and stores can forbid it. Planned outreach uses an Authority Lease over a sub-store.
