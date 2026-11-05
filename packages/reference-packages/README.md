# Reference packages

Published Definition packages the platform ships as references (configuration.md; ADR 0024). Each directory is a package, in the layout `health_packages` packs. `health-publication`'s `tests/reference_packages.rs` checks that each one passes validation and its `tests/` scenarios.

| Package | What | Issue |
|---|---|---|
| `lab/` (`ug.moh.lab`) | The lab Workflow Definition (`Library/lab-test`) and its workflow scenarios | #109 |

**Sources:** a Workflow Definition is inline, base64 content in its Library. Its readable source is in `sources/`, and the test checks the Library holds exactly that source. Edit the source, then regenerate the Library's `content[0].data` from it.

## `lab/`: the laboratory test

A test is a workflow instance about its ServiceRequest. Laboratory delivers each step to it (`health-fhir-server`'s `lab`):

| State | Worklist | Leaves on |
|---|---|---|
| `collect` | `lab-collect` | `laboratory.collected`, or `laboratory.closed` (cancelled) |
| `result` | `lab-result` | `laboratory.resulted`, or `laboratory.closed` (specimen rejected) |
| `verify` | `lab-verify` | `laboratory.verified`, or `laboratory.verified-critical` |
| `review` | `clinician-results` | `laboratory.reviewed` |
| `review-critical` | `clinician-results`, priority `stat` | `laboratory.reviewed` |

The clinician's worklist gets a test only once its results are verified, and a critical result arrives at `stat` priority.
