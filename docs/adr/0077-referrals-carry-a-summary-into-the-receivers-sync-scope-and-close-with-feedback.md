# Referrals carry a summary into the receiver's Sync Scope and close with feedback

Decided in [How do referrals work between facilities, including feedback and offline?](https://github.com/carapai/people-centered-healthcare/issues/624), after the research [What do referrals, inpatient care, emergency care and imaging in Uganda require?](https://github.com/carapai/people-centered-healthcare/issues/623) ([clinical-operations.md](https://github.com/carapai/people-centered-healthcare/blob/research/clinical-operations/docs/research/clinical-operations.md), branch `research/clinical-operations`).

That research found four things:
- Uganda's paper referral note (HMIS 032) is a closed loop: the sender fills the top half and the receiver returns the bottom half.
- HMIS counts referrals only as totals.
- Return rates are poor.
- There is no national e-referral standard. IHE 360X is not FHIR, and mCSD is R4.

The platform already keeps a record authoritative at the facility that created it (ADR 0012). Other facilities' records are read on demand through central rather than replicated.

**The decision:**
- **A Referral is between facilities.**
  - A move to another department in the same facility stays an order or queue transfer.
  - A Back-referral down a level is still a Referral.
- **Two owners, two resources.**
  - The sender owns a `ServiceRequest` (category referral) addressed to the receiving facility.
  - The receiver owns a `Task` for fulfilment: `received`, `accepted` or `declined`, `arrived`, `completed`.
  - Neither side edits the other's resource.
- **A Referral Summary travels; the rest is on demand.**
  - The sender signs a **Referral Summary**: a `Composition` with the HMIS 032 sender fields, prefilled from the record.
  - The receiver's Sync Scope gains every referral addressed to it, with its summary, so it is there offline.
  - The full history stays an on-demand shared-record read through central.
- **The referral grants access.**
  - The receiving facility may read the summary and the shared record until the referral is closed plus 90 days.
  - A patient's recorded objection blocks the shared-record read but not the summary.
  - Confidential categories keep their own rules.
- **Arrival.**
  - The receiver matches an arriving patient from its incoming list, by referral number, or by the note's QR code.
  - Matching links the patient and marks the Task `arrived`.
  - A receiver not on the platform gets a printed HMIS 032-style note with a QR code.
- **Referral Feedback closes the loop.**
  - The receiver records arrival and discharge dates, findings, diagnosis, treatment, what to continue, and whether the referral was appropriate.
  - The feedback syncs back to the sender, notifies the referring clinician and completes the referral.
  - A returned paper note can be entered by the sender.
  - Referrals with no feedback after 14 days (a setting) are listed as open.
- **Destination and priority.**
  - The destination is chosen from the facility registry by Healthcare Service, with the next tier suggested first.
  - Priorities are routine, urgent and emergency. Urgent and emergency raise an immediate Notification at the receiver.
  - Transport mode is recorded. Ambulance dispatch is deferred.
- **Counting.**
  - Referrals out come from the sender's requests.
  - Referrals in come from arrivals: matched, self-reported with a note, or from a VHT or community source.
  - Both feed HMIS 105 and HMIS 108 through package Indicators.

## Considered options

- **Replicating the whole record to the receiver:** moves far more data to edges, outlives the referral, and widens access beyond what the referral needs.
- **The shared record only, with no summary:** fails when the receiver is offline. It is also not what the clinician chose to send.
- **One shared referral resource edited by both facilities:** breaks single authority per record (ADR 0012) and causes conflicts.
- **IHE 360X:** HL7 v2 and C-CDA over Direct, not FHIR, and no counterpart in Uganda.
