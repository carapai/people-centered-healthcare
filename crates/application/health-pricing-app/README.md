# health-pricing-app

Layer: **application** (pricing). Pricing at central, where Price Books and rules are edited and approved, so prices cannot change at a facility while it is offline (pricing-and-insurance.md; ADR 0031; #120).

- **`Pricing::draft_book` and `draft_rule`:** a new draft is the next version of its book or rule. While the latest version is still a draft, drafting again replaces it. The drafter is its author.
- **`approve_book` and `approve_rule`:** one finance approver who is not the author. The domain refuses overlapping versions of the same book or rule, malformed prices, and rules conflicting with approved ones.
- **`retire_book` and `retire_rule`:** an approved version ends from a time not yet passed, which is the only change an approved version takes.
- **`price` and `check`:** a charge priced through the six stages from every kept version, and the authority's check of the versions a charge records.
- **ChargeItemDefinitions** (`fhir`): approving or retiring a version writes them in the same transaction. Drafts are not projected, and each version has its own resource.
  - **A book entry:** its `code`, the book's `version`, R5 `applicability.effectivePeriod`, a `base` component and a property group per quantity tier.
  - **A rule:** its applicability and one `surcharge`, `discount` or `deduction` component, as a `factor` or an `amount`.
  - **Platform extensions:** the book's scope, and the rule's stage, priority, items and bundle.
- **Transactions and permissions:** each step is one transaction, and who may draft and approve is the caller's permission check.

## Tests

`tests/pricing.rs` (over SurrealDB):

- a book drafted, redrafted, refused to its author, approved, then superseded by a version that starts after the first is retired, with charges before and after priced from the right version;
- a rule that applies only once approved, with the authority's check of the recorded versions.
