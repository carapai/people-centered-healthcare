# Dependencies

_Generated from the backlog spec when the backlog was created (2026-10-28). GitHub issues are authoritative; update this file when the plan changes._

Every task-level edge is a native GitHub issue dependency (see each issue's "Blocked by"). This page summarises them.

## Phase-level graph

```mermaid
flowchart LR
  P1["1. Monorepo foundations"]
  P2["2. Persistence and runtime adapters"]
  P3["3. Identity, security and Client Registry"]
  P4["4. Facility Edge Node foundation"]
  P5["5. Synchronization foundation"]
  P6["6. Terminology and metadata"]
  P7["7. Universal workflow engine"]
  P8["8. OPD clinical vertical slice"]
  P9["9. Clinical services"]
  P10["10. Patient engagement"]
  P11["11. Clinical Program Studio"]
  P12["12. Finance and insurance"]
  P13["13. Inventory and procurement"]
  P14["14. Additional clinical operations"]
  P15["15. Public health and analytics"]
  P16["16. Advanced desktop offline mode"]
  P17["17. Production hardening"]
  P1 --> P2
  P1 --> P3
  P1 --> P4
  P1 --> P5
  P1 --> P6
  P1 --> P7
  P1 --> P17
  P2 --> P3
  P2 --> P4
  P2 --> P5
  P2 --> P6
  P2 --> P7
  P2 --> P12
  P2 --> P13
  P2 --> P16
  P2 --> P17
  P3 --> P4
  P3 --> P6
  P3 --> P8
  P3 --> P10
  P3 --> P17
  P4 --> P5
  P4 --> P8
  P4 --> P16
  P4 --> P17
  P5 --> P6
  P5 --> P7
  P5 --> P8
  P5 --> P16
  P5 --> P17
  P6 --> P7
  P6 --> P8
  P6 --> P12
  P7 --> P8
  P7 --> P9
  P7 --> P10
  P7 --> P11
  P7 --> P12
  P8 --> P9
  P8 --> P10
  P8 --> P11
  P9 --> P13
  P10 --> P17
  P12 --> P13
  P13 --> P16
  P13 --> P17
  P17 --> P16
```

## Validation spikes that gate tasks

| Spike | Gates |
|---|---|
| [Does embedded SurrealDB RocksDB survive hard power loss on facility hardware?](https://github.com/carapai/people-centered-healthcare/issues/34) | [Create the facility-edge executable as an OS service](https://github.com/carapai/people-centered-healthcare/issues/72) |
| [Which R5 model crate passes the round-trip spike: helios-fhir or fhir-r5?](https://github.com/carapai/people-centered-healthcare/issues/35) | [Adopt the R5 model crate behind health-fhir-models](https://github.com/carapai/people-centered-healthcare/issues/53) |
| [Does the per-node commit counter sustain facility write rates on edge hardware?](https://github.com/carapai/people-centered-healthcare/issues/36) | [Implement the change log, commit counter and outbox in the write transaction](https://github.com/carapai/people-centered-healthcare/issues/79) |
| [Can SurrealDB blocking indexes serve patient matching at 45M-person scale?](https://github.com/carapai/people-centered-healthcare/issues/37) | [Implement the matching engine and Patient/$match](https://github.com/carapai/people-centered-healthcare/issues/69) |
| [Does the frontend scaffold build on Bun, Vite 8, Panda 2, React Compiler and TS 7?](https://github.com/carapai/people-centered-healthcare/issues/38) | [Create the Bun workspace and the React app shell with the platform boundary](https://github.com/carapai/people-centered-healthcare/issues/44) |
| [Does embedded SurrealDB run reliably inside the Tauri process on facility PCs?](https://github.com/carapai/people-centered-healthcare/issues/39) | [Build the Tauri desktop runtime](https://github.com/carapai/people-centered-healthcare/issues/132) |

## Starting point

The only task with no blockers is [Create the Cargo workspace with the six-layer layout](https://github.com/carapai/people-centered-healthcare/issues/42). Everything else follows from it and from the spikes.
