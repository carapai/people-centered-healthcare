# A workflow guard may name a decision table and the outcome it requires

ADR 0043 lets guards use decision tables, but it does not say how a guard refers to one. We decided that a transition's guard takes one of three forms:
- **always**: the guard is absent;
- **a FHIRPath expression**, as before;
- **a decision-table guard**: the name of a decision table in the same definition, and the outcome it requires, for example `{ "table": "triage-priority", "outcome": "urgent" }`.

**Evaluation.** The engine evaluates a decision-table guard through the same guard port as a FHIRPath guard:
- it runs the table's rows in order over the declared context bundle;
- the first row whose condition holds gives the outcome; if none holds, the table's default does;
- if there is no default and no row holds, the guard is false;
- the guard holds when the outcome equals the required one.

Central and the edge use the same code, so a candidate transition taken offline is re-judged at the authority exactly as a FHIRPath guard is.

**Publication** refuses a decision-table guard that:
- names a table the definition does not have;
- requires an outcome that is neither one of the table's row outcomes nor its default.

The table's own checks (coverage, rows that never fire, its test cases) apply as they do for any table.

**Simulation.** A workflow scenario stubs a decision-table guard by the table's outcome, not by the guard's truth. One stub then answers every guard that names that table, as the real table would.

**The Studio.** The Workflow Studio inspector offers a decision-table guard as a choice next to FHIRPath. The author picks the table and the outcome from the table's outcomes, and can open the table in the decision-table editor from there.

We chose this because it keeps FHIRPath standard and adds no extension function. It also lets publication check the table and outcome by name, and it matches how the Studio already presents tables: named rules with outcomes.

## Considered Options

- **A FHIRPath extension function such as `%decision('triage-priority') = 'urgent'`.** Rejected. Guards would stop being plain FHIRPath. Every evaluator and type checker would need the extension. And a typo in the table name would surface only at run time unless the expression were parsed specially.
- **Inlining the table's rows into the guard as one FHIRPath expression.** Rejected. It loses the named rules, first-match order and default that authors and reviewers read, and a table used by several transitions would be copied into each of them.

## Consequences

- The workflow definition format gains a `guard` variant. Older definitions, whose guards are all FHIRPath or absent, are unchanged.
- The guard port evaluates tables, so the edge must ship the definition's tables with it. They already travel inside the definition.
- Scenario files gain table-outcome stubs next to expression stubs.
