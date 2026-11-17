# Translations ship as Language Packs, merged in when served

The words people read in forms and code lists can be in a language other than English. The wording for each language lives in a **Language Pack**: its own package that targets another package and is published and activated separately. Each node merges the active packs into the resources it serves. Translators then never hold a clinical draft, never trigger a new clinical version, and several languages move in parallel. Clients still see standard FHIR (R5's `translation` extension and CodeSystem `designation`). Decided in [How are forms and code lists translated?](https://github.com/carapai/people-centered-healthcare/issues/483).

**The decision:**
- **One pack per package and language,** e.g. `ug.moh.anc.lg` for `ug.moh.anc`, of a new package kind, `language`. It targets one major version of its package (`2.x`). It may also cover the ValueSets and CodeSystems that package binds.
- **Entries are keyed by a stable address.** Each entry records the English text it translates.

  | What | Key |
  |---|---|
  | A Questionnaire item's text, prefix and help | `url` + `linkId` |
  | An answer option's display | `url` + `linkId` + code |
  | A Form Layout section title | layout `url` + section id |
  | A code-list display | system + code |

- **Stale entries are not served.** When the English an entry recorded no longer matches its target, that string falls back to English, and the Studio lists the entry for re-translation. An entry whose target no longer exists is ignored and flagged.
- **English is the base and the fallback.** Each untranslated string falls back to English on its own, with no warning to the reader. A pack need not be complete to be activated.
- **Merged when served, never stored.** Central, edges and desktops merge their active packs into a response for the languages it asks for: `Accept-Language`, or `displayLanguage` on `$expand` and `$lookup`. The stored definition is never rewritten. A pack's entry wins over a `translation` extension already in imported content.
- **Lifecycle.**
  - Packs are versioned with semver on their own: a new or changed translation is a minor version, a typo fix a patch, and dropping the target's major version a major one.
  - They go through the usual lifecycle with **one** approval instead of the clinical pair, by someone who did not hold the draft (ADR 0025), and the approver states in the discussion that they read the language.
  - They are activated per scope and sync to edges like any package.
- **Choosing the language.** A health worker's account has a language preference, defaulting to the device's and then English. It covers both the app's own messages and authored content. Patient-facing output (the portal, SMS, printed slips) uses the patient's `Patient.communication.preferred`. A deployment offers a language only when the app has a message catalog for it. Those catalogs stay in the repository, translated through ordinary changes.
- **What is recorded.** A QuestionnaireResponse keeps answers as codes and values. Its `item.text` is the English text of the definition, and `QuestionnaireResponse.language` records the language the form was filled in. Free-text answers are kept as typed and are never machine-translated.
- **The Uganda reference deployment** ships Luganda (`lg`) first, for what patients read: portal forms, consent texts, appointment SMS and patient-facing displays. Health-worker forms stay in English until a program asks. Swahili (`sw`) and Runyankole (`nyn`) follow on demand.

## Considered Options

| Option | Decision |
|---|---|
| **A Language Pack per package and language, merged in when served** | **Chosen**: translation never blocks or re-versions clinical content, languages proceed in parallel, and the wire format is still standard FHIR |
| `translation` extensions inside each artefact | Rejected: every corrected word needs a new clinical version and the Draft Holder's draft, and all languages queue behind one holder |
| Only CodeSystem designations, with forms in English | Rejected: patients read form text, consent and messages, not only code displays |
| Machine translation at display time | Rejected: unreviewed clinical wording in front of patients |

## Consequences

- The FHIR server and terminology service gain a merge step keyed on the request's language. The renderer reads the `translation` extension for the language it was given.
- The Studio has a translation editor that opens from a Language Pack draft (studio.md).
- A new major version of a clinical package needs a new major version of each of its packs before their wording appears again. Until then, English shows.
