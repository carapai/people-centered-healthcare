# Research: how profiles get their snapshots, with no Java and no FSH import

Ticket: [#518 How are profiles authored and given snapshots, with no Java and no FSH import?](https://github.com/carapai/people-centered-healthcare/issues/518). Facts checked on 2026-11-17 against the npm registry, crates.io, GitHub, the HL7 R5 specification and the tools' own source. I ran every candidate in a scratch directory outside the repo and compared its output with the HL7 Java validator's.

**Recommendation:** option **(c) at the product level, with (b) as the supported way to produce snapshots.**
- The Studio and central accept a profile only if it already carries a snapshot.
- Snapshots come from:
  - a CI build: SUSHI or plain JSON, then the pinned HL7 Java validator's `snapshot` command. This is what `tools/fhir/build-core-package.sh` already does for the platform core package;
  - or the IG Publisher snapshots that published IGs already ship.
- Central adds a cheap Rust **consistency check**, which is not a generator, at package Validation.
- Do **not** build or adopt a Studio snapshot generator now. No maintained TypeScript or Rust generator agrees with the Java tool closely enough. The closest one gets three validation-relevant things wrong on the official BP profile and our sliced fixture.

## Requirements

- **ADR 0053:**
  - Published profiles ship snapshots, and package publication rejects a profile that has only a differential.
  - Nodes and central never generate snapshots, and no Java runs in the product.
  - The tier 3 validator compiles snapshots at activation.
- **ADR 0016:** the HL7 Java validator is a CI oracle only.
- **ADR 0045:** licences must be MIT, Apache-2.0, BSD, ISC, Unicode, Zlib or MPL-2.0.
- **studio.md:**
  - The Studio runs in the React app, at central only.
  - It imports plain FHIR JSON and R4 conversions. FSH/SUSHI sources and an IG Publisher round trip are not supported.
  - Profile authoring is not one of its editors. "The Studio's own profile snapshot generator" is listed as fog.
- **configuration.md (Validation step):** "every profile carries a snapshot, compiles, and uses only constructs the tier 3 validator supports".
- **The repo already does option (b) for its own package.**
  - `tools/fhir/build-core-package.sh` (#87) runs SUSHI 3.21.0, then `java -jar validator_cli.jar snapshot -version 5.0.0 -tx n/a`, and commits the result.
  - The `java-differential` job in `rust-ci.yml` rebuilds it and fails on any diff.
  - The repo pins validator 7.0.2 (`tools/fhir/fetch-java-validator.sh`).

## Candidates

| | **fhir-snapshot-generator** (Outburn, FSG) | **tlq-fhir-snapshot** | **rh-foundation `snapshot`** | **SUSHI `--snapshot`** (FSH only) | HL7 Java validator (oracle) |
|---|---|---|---|---|---|
| Language | TypeScript, Node only | Rust | Rust | TypeScript, Node | Java |
| Version tested | 2.3.1 | 0.1.10 | 0.3.0 | 3.21.0 (via GoFSH 2.6.1) | 7.0.6 |
| Licence | Apache-2.0. Dependencies `fhir-package-explorer` 1.9.4 and `fhir-package-installer` 1.13.1 are MIT: **allowed** | MIT: allowed | MIT OR Apache-2.0: allowed (the repo's GitHub licence shows NOASSERTION) | Apache-2.0: allowed | Apache-2.0, but Java is excluded from the product |
| Last release | **2026-03-15** (8 months ago). Twelve releases 2026-01 to 2026-03, then none | **2026-01-10**. The source repo `thalamiq/tlq-fhir-core` returns **404** | 2026-10-08 | 2026-10-15 | 2026-11-13. Releases 7.0.3 to 7.0.6 came within 4 weeks |
| Maintenance | One npm maintainer, 1 GitHub star, 0 open issues. Part of Outburn's FUME | Single author. Repo gone | reason-healthcare/rh, 22 stars. Its docs say it follows "the FHIR R4 specification" | HL7/MITRE, 192 stars, active | HL7, the reference |
| R5 | Yes (`hl7.fhir.r5.core@5.0.0`) | Yes | Not really (see results) | Yes | Yes |
| Runs in the browser | **No.** It reads a disk package cache through `fs-extra` and downloads packages over the network | Library. No loader without `registry-loader` (reqwest, `~/.fhir`) | Library | No | No |
| Slicing | Yes. It adds slice children and handles discriminators and reslicing | Rejects valid slice differentials | Rejects valid slices and sorts elements alphabetically | Yes | Reference |
| Extensions | Yes (extension slice with `profile`) | Yes on the simple extension | Yes on the simple extension | Yes | Reference |
| Choice types | Type restriction yes. **Renamed paths (`valueQuantity`) are applied to the whole `value[x]`** | Leaves `Observation.component.valueQuantity` in the snapshot | No type-slice handling | Retypes through FSH | Emits type slices `value[x]:valueQuantity` (R5 id rule) |
| contentReference | Expands it, but **loses `type` and the `obs-3` invariant** in a slice | Partial | None | Yes | Reference |
| Speed | 12–145 ms per profile | 1–18 ms | under 1 s | (whole build) | about 6 s start-up, then fast |

Not candidates: `octofhir-fhirschema` 0.3.42 and `helios-fhir-validator` 0.2.9 convert to FHIR Schema rather than producing snapshots, and ADR 0053 already records their slicing faults. `@atomic-ehr/fhirschema` 0.0.14 likewise converts to FHIR Schema. An npm search for "fhir snapshot" or "structuredefinition snapshot" and a crates.io search for "fhir snapshot", "fhir structuredefinition" or "fhir profile" found nothing else. I did not assess non-TS/Rust generators such as the Firely .NET SDK.

## The experiment

Inputs:
- The R5 core profiles `vitalsigns`, `heartrate`, `bp` and `bodyweight` from `hl7.fhir.r5.core#5.0.0`, with their snapshots stripped.
- Two hand-written R5 fixtures:
  - `lab-position`, an Extension with a fixed URL and `value[x]` restricted to CodeableConcept;
  - `lab-bp`, an Observation with:
    - an extension slice carrying a profile;
    - `category` sliced by two `value` discriminators with fixed `system` and `code`;
    - a `pattern` code;
    - `subject` restricted to Patient;
    - `effective[x]` restricted to two types;
    - `component` sliced by `pattern` on `code`;
    - `value[x]` restricted to Quantity with fixed `system` and `code` children;
    - an explicit `value[x]:valueQuantity` type slice;
    - a constraint on `component:systolic.referenceRange`, which is a `contentReference` to `#Observation.referenceRange`.

The comparison:
- **Fields compared:** the ones the tier 3 validator consumes: element ids and order, `path`, `min`/`max`, `type` (code, profile and targetProfile), `fixed[x]`/`pattern[x]`, `slicing`, `sliceName`, binding strength and valueSet, constraint keys, `contentReference`, `base`, `mustSupport` and `isModifier`.
- **Normalisation:** Java 7.x appends `|5.0.0` to every core canonical. I normalised that away.
- **Reference:** `validator_cli.jar` 7.0.6 `snapshot -version=5.0 -tx=n/a`.

Validation-field differences against Java 7.0.6, after normalisation. "Elements" gives missing/extra element ids:

| Profile | Shipped 2023 snapshot | FSG 2.3.1 | SUSHI 3.21.0 via GoFSH | tlq 0.1.10 | rh-foundation 0.3.0 |
|---|---|---|---|---|---|
| vitalsigns | 3 diffs; 0/12 elements | 4; 13/0 | 4; 12/0 | 21; 21/1, order wrong | 8; 21/1, order wrong |
| heartrate | 3 | 5; 9/7 | 6 | 21; 16/5, order wrong | 13; 25/6, order wrong |
| bp | 5 | **11**; 18/14 | 14 | **failed** | **failed** |
| bodyweight | 3 | 5; 9/7 | 6 | 21; 16/5, order wrong | 13; 25/6, order wrong |
| lab-position | n/a | **0** | 0 | 0 | 0 (order wrong) |
| lab-bp | n/a | **12**; 8/7 | 12; 8/7 | **failed** | **failed** |

### What the differences mean

**The Java tool does not agree with itself across versions.** The R5 core package's own snapshots, built by the 2023 R5 build, differ from 7.0.6 on 3–5 validation fields per profile:
- 7.0.6 adds `sqty-1` to SimpleQuantity elements;
- it turns `value[x]` type slicing `open` → `closed`;
- it emits a type slice for every `component.value[x]` type in vitalsigns;
- it pins canonicals to `|5.0.0`.

So "agrees with Java" always means "with one pinned Java version". A snapshot is an artefact you generate once and publish, not something to regenerate and expect the same bytes.

**FSG** agrees with Java on the core of slicing:
- slice names, discriminators and rules;
- slice children of complex types (`category:VSCat.coding.code` fixed `vital-signs`);
- extension slices with their profile;
- reference `targetProfile`, fixed/pattern values, cardinalities and element order.

It is also exact on the extension fixture. Three of its differences would change a validation result:
1. **A renamed choice path is applied to all of `value[x]`.**
   - R5 `bp` has `Observation.valueQuantity` with `max 0`. Java makes a type slice `value[x]:valueQuantity` with max 0, which forbids only the Quantity type.
   - FSG sets `Observation.value[x]` to `max 0`, which forbids every value.
   - The R5 id rule says "for type choice elements, the id reflects the type slice", for example `Patient.deceased[x]:deceasedBoolean` (elementdefinition.html, id rules).
2. **The same fault moves a binding.**
   - vitalsigns' `component.valueQuantity` carries a required binding to `ucum-vitals-common`.
   - FSG puts it on `component.value[x]`, so it would apply to every type of a component value, not only Quantity.
3. **contentReference inside a slice loses rules.**
   - `component:systolic.referenceRange` comes out with no `type` and without the `obs-3` invariant.
   - Java gives `BackboneElement` with `ele-1` and `obs-3`. Tier 3 would never check `obs-3` there.

FSG also has two operational traps:
- **It fails silently.** If generation throws, it logs a warning and returns the package's original snapshot ("Using the original StructureDefinition from source package"). It throws only if there is none.
- **It depends on how its cache was filled.** With R5 core copied into its cache by hand, it could not resolve `CodeableConcept` and fell back. It worked once its own installer had downloaded the package.

The remaining differences are representation choices:
- `value[x].system` instead of `value[x]:valueQuantity.system` where the type is already narrowed to Quantity;
- relative `#Observation.referenceRange` against Java's absolute form;
- `mustSupport` not copied onto slices, which is out of tier 3 v1 scope;
- `sqty-1`, which also differs between Java versions.

**SUSHI** generates good snapshots only from FSH. To test it on JSON, I converted the JSON differentials to FSH with GoFSH. That round trip is lossy:
- `coding` became `min 1` from a pattern assignment;
- `value[x]` kept all types where Java narrows it;
- SUSHI reported 3 errors on extension metadata.

The Studio excludes FSH (studio.md), and HL7 recommends leaving snapshots to the IG Publisher. The FSH School docs say "This is the approach recommended by HL7 FHIR leadership".

**tlq-fhir-snapshot:**
- It rejects valid differentials, for example "Element 'Observation.component.code.coding' appears before its parent".
- It drops reference `targetProfile`s: vitalsigns' `subject` loses its Patient restriction.
- It writes `base.path` as `code` instead of `Coding.code`.
- It leaves `Observation.component.valueQuantity` in the snapshot.
- Its repo is gone.

**rh-foundation:**
- It rejects valid slices: "differential min (1) is less than base min (2)". It checks a slice's `min` against the sliced element's `min`, but a slice may have a lower `min`.
- It sorts the snapshot alphabetically by path, with duplicate ids.
- It does not expand complex types under slices.

## Options

| | (a) Generate in the Studio (browser TS, or Rust at central) | (b) Generate in CI with SUSHI/JSON + the Java validator, then upload | (c) Accept only uploaded snapshots |
|---|---|---|---|
| Correctness | Needs our own generator, proven by a differential test against Java as ADR 0053 does for the validator. No library gets there today: FSG is the closest, and it is wrong on renamed choices and on contentReference in slices | Matches the oracle by construction for the pinned version | As good as the producer. Published IGs ship IG Publisher snapshots |
| Java in the product | No | No. Java runs only in an author's or country's CI, like ours | No |
| FSH import | No | No. FSH stays in a git repo outside the Studio; the Studio imports the JSON output | No |
| Cost | High: a second hard FHIR algorithm (base merge, type slices, reslicing, contentReference, extension expansion), plus its oracle test and upkeep across R5 point releases | Already built for our core package (`build-core-package.sh`, `java-differential` job). Countries copy the pattern | Lowest: a check at Validation |
| What it would unlock | Editing profiles in the Studio | Same authoring path HL7 IGs use | Importing published IGs as they are |
| Risk | Getting it wrong silently. A wrong snapshot is a wrong validator (ADR 0053) | Java version drift: pin it, regenerate on purpose | Trusting a snapshot that does not match its differential |

## Recommendation

Choose **(c)**, with **(b) as the documented route**, and defer (a).

1. **Product rule:** a profile enters a package only with a snapshot, as ADR 0053 already says. The Studio imports profile JSON as it is. It never edits a profile's elements, because that would leave the snapshot stale. A changed profile is re-imported.
2. **Where snapshots come from:**
   - **Published IGs:** use the snapshots they ship.
   - **Our own packages and country profiles:**
     1. Keep the sources (FSH or JSON differentials) in a git repo.
     2. In CI, run SUSHI, then the **pinned** HL7 validator `snapshot`, which is what `build-core-package.sh` does.
     3. Commit the output.
     4. Import that output into the Studio.
3. **A Rust consistency check at Validation** (in `health-fhir-validation`, next to the ADR 0053 compile step). It checks; it does not generate:
   - every differential element id appears in the snapshot, with the same constrained values: `min`/`max`, types and profiles, `fixed`/`pattern`, slicing, binding and constraint keys;
   - the snapshot's root and `base` paths resolve against the active base definition;
   - `fhirVersion` is 5.0.0 and `baseDefinition` resolves in the package's dependencies.

   This catches a stale or hand-edited snapshot without our having to produce one. ADR 0053's compile and unsupported-construct checks still apply on top.
4. **Pin the oracle and regenerate on purpose.**
   - The repo pins 7.0.2, while 7.0.6 is current. The 2023 build and 7.0.6 already disagree on `sqty-1` and on the `value[x]` slicing rules.
   - Bump the oracle the way `fetch-java-validator.sh` already describes, and regenerate snapshots in the same change.
5. **Revisit (a) only if a Studio profile editor is ever wanted.**
   - If it is, write our own Rust generator at central and accept it only after a differential test against the pinned Java version, over the R5 core profiles and our slicing fixtures. That is the same gate ADR 0053 sets for the validator.
   - FSG's ~1,200-line algorithm (Apache-2.0) is worth reading, but not as a dependency: it is Node-only, it fails silently, its maintenance has stalled since 2026-03, and it has the semantic faults above.

**Trade-offs accepted:**
- Profile authors (the MoH standards team) work in a git repo with a CI job, not in the Studio. Profiles change rarely and only central experts write them, so this is a small cost.
- Re-importing a changed profile is a round trip.
- The Java tool's output depends on its version, so snapshots are pinned artefacts.

## Sources

- npm registry, read 2026-11-17:
  - `fhir-snapshot-generator`: 2.3.1, Apache-2.0, released 2026-03-15, maintainer `daniel-mechanik`;
  - `fhir-package-explorer` 1.9.4 (MIT); `fhir-package-installer` 1.13.1 (MIT);
  - `fsh-sushi` 3.21.0 (Apache-2.0, 2026-10-15); `gofsh` 2.6.1 (Apache-2.0);
  - `@atomic-ehr/fhirschema` 0.0.14.
- GitHub, read 2026-11-17:
  - `Outburn-IL/fhir-snapshot-generator`: Apache-2.0, last push 2026-03-15, 1 star;
  - `thalamiq/tlq-fhir-core`: HTTP 404;
  - `reason-healthcare/rh`: last push 2026-10-08;
  - `FHIR/sushi`: Apache-2.0, last push 2026-11-09;
  - `hapifhir/org.hl7.fhir.core` releases: 7.0.3 (2026-10-21), 7.0.4 (2026-11-02), 7.0.5 (2026-11-05), 7.0.6 (2026-11-13).
- crates.io, read 2026-11-17:
  - `tlq-fhir-snapshot` 0.1.10 (MIT, 2026-01-10, 307 downloads);
  - `rh-foundation` 0.3.0 (MIT OR Apache-2.0, 2026-10-08);
  - `octofhir-fhirschema` 0.3.42; `helios-fhir-validator` 0.2.9.
- Source read:
  - FSG `dist/index.mjs`: the fallback in `getSnapshotByMeta` and contentReference expansion;
  - rh-foundation `src/snapshot/merger.rs` (`sort_elements_by_path`) and `SNAPSHOT.md` ("as specified in the FHIR R4 specification");
  - tlq `src/lib.rs` and `tests/test_fhir_test_cases.rs` (the HL7 fhir-test-cases harness is `#[ignore]` and only warns on failure).
- HL7 FHIR R5:
  - [profiling.html §5.1.0.11](https://hl7.org/fhir/R5/profiling.html#snapshot): a differential "must be applied to the structure definition on which it is based"; operational systems "should always have the snapshot view populated";
  - [elementdefinition.html](https://hl7.org/fhir/R5/elementdefinition.html), id rules: "For type choice elements, the id reflects the type slice. e.g. … `Patient.deceased[x]:deceasedBoolean`"; `contentReference` "bring[s] across all the rules that are in the ElementDefinition for the element".
- [FSH School, Running SUSHI](https://fshschool.org/docs/sushi/running/): "By default, SUSHI only generates the profile differential, allowing the IG Publisher to create the profile snapshot. This is the approach recommended by HL7 FHIR leadership."
- Package: `hl7.fhir.r5.core#5.0.0` from packages.fhir.org.
- Repo:
  - ADR 0016, 0045 and 0053;
  - `docs/architecture/studio.md` and `configuration.md`;
  - `tools/fhir/build-core-package.sh` and `fetch-java-validator.sh`;
  - `.github/workflows/rust-ci.yml` (`java-differential`).
