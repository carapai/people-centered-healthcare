# District and facility overrides change configuration only, at override points their base package declares

**What an override package may change:** configuration keys that its base package explicitly declares as **override points**. Examples are layouts, section visibility, which Healthcare Services and programs are active, and worklist and queue settings.

**What it may never change:** profiles, terminology meaning, Extraction Mappings, workflow definitions or clinical rules.

**Layering.** Overrides are layered national → district → facility and merged key by key.

**Approval.** Overrides follow the same lifecycle as any package. A presentation-only override needs the facility admin alone. A clinically visible one, such as hiding a form section, also needs a district or central reviewer.

We chose this because facilities genuinely differ in services, staffing and layout. Letting them alter clinical meaning, though, would make records incomparable across facilities and would bypass clinical approval.

## Considered Options

- **Facility-local copies of Questionnaires or ValueSets.** Rejected: the copies fork clinical meaning and break national reporting. A facility that needs different clinical content requests a new national or program version.
- **Overriding any key.** Rejected: base packages could not be upgraded safely under unknown overrides.

## Consequences

- A base package's major version may remove override points. Validation then flags any override that is now orphaned before the new version can be activated in that scope.
