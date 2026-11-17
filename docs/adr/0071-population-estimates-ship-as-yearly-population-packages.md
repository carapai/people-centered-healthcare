# Population Estimates ship as yearly Population packages; Age Band Sets live in the national core and program packages

Decided in [How are Age Band Sets and Population Estimates authored, sourced and published?](https://github.com/carapai/people-centered-healthcare/issues/541). Rates and coverage indicators need a population denominator (e.g. ANC1 coverage over expected pregnancies), and Reports disaggregate by MoH's age bands ([ADR 0070](0070-reports-are-configuration-that-lay-out-indicators-and-carry-the-dhis2-mapping.md)). Estimates are data that change every year for thousands of places; age bands are small definitions that rarely change.

**The decision:**
- **Age Band Sets** are configuration: MoH's sets in the national core package, program-specific sets in their program package, reviewed by that package's approvers. Any change to the bands is a **major** version (it changes Report columns and DHIS2 category options); a rename is a patch. Indicators and Reports pin the version they picked.
- **Population Estimates ship as a new package kind, Population**, one package per year (e.g. `ug.population.2026`), approved by the HMIS lead. Review, activation and sync reuse the package machinery: each edge receives the figures for its own places.
- **Estimate types.** A Population package declares its types (total population, expected pregnancies, under-1, under-5, women of reproductive age, …). A type is either given per place or **derived** as a percentage of another type, as MoH does (expected pregnancies = 5% of total population). A rate's denominator names an estimate type.
- **Places.** Figures are given at the lowest level available (a facility's catchment or a sub-county) and summed for higher levels. Where a type has only higher-level figures, a lower-level Report shows *no estimate* rather than guessing a share.
- **Periods.** Estimates are annual. A Report's period takes the annual figure pro rata: a month 1/12, a quarter 1/4, a week 7/365.
- **Source in v1.** The HMIS lead imports a year's figures at central from a CSV template, typically UBOS projections. The Studio checks place codes against the facility registry and the NHFR and shows district totals before approval. District entry of catchment figures comes later.

## Considered options

- **A separate population dataset** imported at central and copied read-only to edges: it would need its own approval, versioning and sync, all of which packages already provide.
- **Estimates inside each program package:** the same figures would be duplicated across programs and fall out of step.
- **Splitting a district figure across its facilities** when no catchment figure exists: a guess presented as data.
