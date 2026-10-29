# health-identifiers

Layer: **foundation**. Implements identifiers.md, ADRs 0013 and 0014.

- **Typed ids.** `ResourceId` (UUIDv7, or UUIDv5 when derived), `NodeId`, `DeviceId` and `OpId` (UUIDv7). Modules declare their own with `define_id!`.
  - Ids print lowercase and hyphenated, and parse only that form.
  - The version rule is enforced, and ids serialize as strings.
- **Derived ids.** `derive_id(&DerivationName::extraction(…) | ::rule(…))` computes `UUIDv5(PLATFORM_NAMESPACE, name)`. The namespace is fixed at `83664680-8814-416c-a4bb-2cc772956887`.
- **Human-readable numbers:**
  - `NumberFormat` is the per-type Published Definition (prefix, padding, `luhn-mod-36` check, yearly reset), and `ShortCode` holds facility, node and prefix codes.
  - `SequenceState::next` holds the counter rules: start at 1, never repeat, and refuse a year that went back.
  - The atomic increment is the store's job, in the record's write transaction.

**Tests.** `tests/identifiers.rs` holds golden UUIDv5 and Luhn values computed independently in Python, plus parsing, version, serde, format and counter tests.
