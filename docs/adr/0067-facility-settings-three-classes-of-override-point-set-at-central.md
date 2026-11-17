# Facility settings: three classes of override point, set at central, applied at once unless clinically visible

> Amends [ADR 0026](0026-overrides-only-at-declared-configuration-override-points.md).

ADR 0026 lets a district or facility change only the override points its base package declares. This ADR decides which points exist, who may set them, where, and how a change becomes a published override, decided in [What can a facility admin adjust, and on which screen?](https://github.com/carapai/people-centered-healthcare/issues/484). A facility admin should be able to change a worklist's columns without waiting for anyone. A change that alters what care patients can get here still needs someone above the facility to agree. And only central signs configuration.

**The decision:**
- **No hiding.** An override never hides a question. A layout may reorder sections, collapse them by default and change widgets and hints, as the Form Layout format already says. Turning a program's forms off means turning the program off at that facility. This replaces ADR 0026's example of hiding a form section.
- **Declared points.** A base package declares its override points in `configuration/override-points.json`, each with a type, a default, the values allowed (a list or a range) and one of three **classes**:

  | Class | Examples | Applied |
  |---|---|---|
  | Presentation | Section order and collapsing, widgets and hints, worklist columns and sort, service point display names | By the admin alone |
  | Operational | Healthcare Services and service points that run here, queue settings, clinic days, appointment slot length, Financial Gates on or off, printing | By the admin alone |
  | Clinically visible | Programs active here; ranges a program declares, e.g. tracing after a missed visit in 14–28 days | After a district or central reviewer approves |

  A facility cannot add a key the base package did not declare.
- **One override package per scope,** e.g. `ug.facility.kam013`, with linear versions.
  - **Apply** publishes the presentation and operational changes as the next version and activates it in that scope at once. The record shows the admin as both author and approver, which ADR 0026 allows for these classes, and it is audited.
  - Clinically visible changes in the same batch become a **pending request**. Once a reviewer with `metadata.review` and `metadata.approve` at district or central approves it, it is applied key by key on top of the version current then and published as the next version.
- **Who.** A new permission, `configuration.override`:
  - held at a facility, it changes that facility's points;
  - held at a district, it changes the district layer.

  Nobody changes a layer above their own scope.
- **Where.** The **Facility settings** page runs at central only, like the Studio, and is not part of it. An offline facility waits. The edge receives the published override through configuration sync. No unsigned configuration is ever made at an edge.
- **Base upgrades.** A new base version that removes a point, or narrows the values it allows below what a scope chose, cannot be activated in that scope until the conflict is gone (ADR 0026). The Facility settings page lists the broken overrides with **Remove** or **Change**. Central (`metadata.manage`) may fix them on a facility's behalf, giving a reason, so one facility cannot hold back a national upgrade.

## Considered Options

| Option | Decision |
|---|---|
| **Central only; presentation and operational changes applied at once, clinically visible ones as pending requests** | **Chosen**: no unsigned configuration at the edge, no waiting for routine changes, and review where patients would notice |
| Edit on the edge, apply as unsigned local overrides, and publish later | Rejected: configuration nobody signed, and conflicts with central's versions, for changes that are rare and seldom urgent |
| Every override through the full review | Rejected: a column order would wait for a district reviewer |
| Allow hiding form sections | Rejected: records stop being comparable across facilities, and a layout already cannot hide a question |

## Consequences

- `health-configuration` gains the override-point declaration and its validation, and override packages gain a self-approved path for two classes.
- The Program Designer's facility adjustments and the Workflow Studio's gate configurations become declared override points of their packages.
