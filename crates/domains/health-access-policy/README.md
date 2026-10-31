# health-access-policy

Layer: **domain** (Identity & Access). The policy decision point's rules (offline-auth.md § Authorization model, § Break-glass; security.md §95–97). They are pure and the same at central and on edges.

- **`Subject`:** the account, the facility, the `Permissions` resolved from its roles, the program assignments, a declared purpose of use, and an active `BreakGlass` grant, if any.
- **`Permissions`:** codes of the form `{Type|*}.{cruds}`, such as `Observation.rs` or `*.r`.
  - A code with any other letters (`payment.record`) is a use-case permission, not a resource grant.
  - `has` answers literal codes, such as `break-glass`.
- **`Pdp::decide(subject, access, now)`** checks, in order:
  1. the permission;
  2. facility isolation: a facility-scope record belongs to the subject's facility. With `NodeScope::Central` (`Pdp::with_scope`, ADR 0056), a read of another facility's record about a patient is allowed (central is the shared record), but searches across patients and writes are not;
  3. sensitivity labels: each configured `LabelRule` label on the record needs its program;
  4. a purpose of use for any sensitive access.

  Break-glass for the record's patient overrides only steps 2 and 3, never permissions. A permit that relied on break-glass carries `Obligation::BreakGlassUsed`, for audit.
- **`Pdp::search_filter(subject, type, patient, now)`** gives the labels a search must exclude: those whose program the subject lacks, or every sensitive label if no purpose is declared. Break-glass lifts the exclusions only for a search confined to the grant's patient.
- **Facility scope (ADR 0057):** `FACILITY_SCOPE_TYPES`, `is_facility_scope` and `FACILITY_TAG_SYSTEM`. `SearchFilter::facility` is the facility a search of a facility-scope type is confined to. It is `None` for global types, central's patient-confined searches, and break-glass.
- **`purpose_of_use(header)`** parses the `X-Purpose-Of-Use` header (ADR 0056). It is a v3 PurposeOfUse code from `PURPOSES_OF_USE`, `TREAT` when absent, and `UnknownPurpose` otherwise.
- **`BreakGlassPolicy::grant`** needs the `break-glass` permission, the `BTG` reason and a justification. The grant covers one patient for 1 hour, and there is no extension or second grant while one is active. It returns the `BreakGlassEvent` to audit, alert on and review.

## Tests

`tests/matrix.rs` is the authorization matrix (testing.md § Security), one table of subject × record × interaction covering:
- roles, and facility isolation, at an edge and at central;
- labels and programs, including several labels at once and labels outside the rules;
- purpose of use.

Further tests cover break-glass (one patient, one hour, the reported event, no widening of permissions) and the search filters.
