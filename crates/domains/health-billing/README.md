# health-billing

Layer: **domain** (billing). Charge capture from configured triggers on clinical events (billing-and-payments.md § Charge capture; identifiers.md; #121).

- **`ChargeTrigger`:** a configuration record with these fields:
  - the event `kind` it fires on;
  - the service points and item codes it fires for (empty for any);
  - what it charges (a fixed item, or each event item's code);
  - the Account basis (encounter, or episode for inpatient stays and long programs);
  - optionally, a free-care funding source.
- **`ClinicalEvent`:** what a clinical Module emits after its commit: id, kind, service time, patient, encounter, episode, service point and items. Clinical code contains no payment logic.
- **`capture(event, triggers)`:** the event's charges, each `planned`. A charge's id is `charge_id`, the UUIDv5 of `{event id}#{item key}|{trigger canonical, no version}` under the platform namespace. A replay gives the same ids on any node, and the trigger's version is not in the id. An event lacking the Account its trigger needs is refused.
- **`Charge` lifecycle:**
  - `priced` makes it `billable`. The shares must add up to the gross amount with none negative, and free care must leave the patient nothing;
  - `billed` puts it on an Invoice or Claim;
  - `cancel` makes a planned charge `not-billable`, and a billable one `entered-in-error` with a `Reversal::Post`. A billed charge is corrected on its Invoice, and cancelling twice changes nothing.
- **`ChargeStore`:** the driven port. `add_charge` is false for a charge already kept, which is how a replay never double-charges.
