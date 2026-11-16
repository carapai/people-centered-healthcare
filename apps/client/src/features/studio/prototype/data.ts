// PROTOTYPE (#478), throwaway: sample packages for the Studio's package workspace variants.
// Nothing here talks to the server; the lifecycle follows ADR 0025 so the separation-of-duties
// rules can be seen.

export type Stage = 'draft' | 'in-review' | 'changes-requested' | 'approved' | 'published' | 'active'
export type Kind = 'national core' | 'terminology' | 'program' | 'country configuration' | 'override'

export interface Person {
  id: string
  name: string
  role: string
}

export const PEOPLE: Person[] = [
  { id: 'nakato', name: 'Dr Sarah Nakato', role: 'Program clinical lead (ANC)' },
  { id: 'okello', name: 'Dr James Okello', role: 'MoH standards team' },
  { id: 'achieng', name: 'Grace Achieng', role: 'Terminology lead' },
  { id: 'mugisha', name: 'Peter Mugisha', role: 'Platform admin' },
]

export interface Check {
  kind: 'validation' | 'simulation'
  name: string
  ok: boolean
  detail: string
}

export interface Artefact {
  type: 'Questionnaire' | 'Form Layout' | 'Extraction Mapping' | 'Workflow Definition' | 'CodeSystem' | 'ValueSet' | 'PlanDefinition' | 'Scenario'
  name: string
  change: 'added' | 'changed' | 'removed' | 'unchanged'
  summary: string
}

export interface Event {
  at: string
  who: string
  what: string
  comment?: string
}

export interface Pkg {
  name: string
  title: string
  kind: Kind
  version: string
  previous: string | null
  bump: 'major' | 'minor' | 'patch'
  stage: Stage
  author: string
  approvals: string[]
  reviewer: string | null
  activations: { scope: string; from: string }[]
  artefacts: Artefact[]
  checks: Check[]
  history: Event[]
}

/** Clinical kinds need two approvers, configuration kinds one; never the author (ADR 0025). */
export const approversNeeded = (kind: Kind) => (kind === 'national core' || kind === 'terminology' || kind === 'program' ? 2 : 1)

export const PACKAGES: Pkg[] = [
  {
    name: 'ug.moh.anc',
    title: 'Antenatal care',
    kind: 'program',
    version: '2.1.0',
    previous: '2.0.0',
    bump: 'minor',
    stage: 'draft',
    author: 'nakato',
    approvals: [],
    reviewer: null,
    activations: [],
    artefacts: [
      { type: 'Questionnaire', name: 'anc-visit', change: 'changed', summary: 'Adds "Danger signs" group (6 items); "Gestational age" now required' },
      { type: 'Form Layout', name: 'anc-visit', change: 'changed', summary: 'Danger signs shown first on visits after 28 weeks' },
      { type: 'Extraction Mapping', name: 'anc-visit', change: 'changed', summary: 'Danger signs extract as Observations (LOINC 72162-9 panel)' },
      { type: 'Workflow Definition', name: 'anc-episode', change: 'unchanged', summary: '' },
      { type: 'Scenario', name: 'danger-sign-referral', change: 'added', summary: 'A positive danger sign raises an urgent referral task' },
    ],
    checks: [
      { kind: 'validation', name: 'Profiles and snapshots', ok: true, detail: 'All 4 resources valid against ug-core 1.3.0' },
      { kind: 'validation', name: 'Semver', ok: true, detail: 'Additive change: minor bump 2.0.0 → 2.1.0 is right' },
      { kind: 'validation', name: 'Bindings', ok: false, detail: 'anc-visit item "danger-sign" binds ValueSet anc-danger-signs|1.0.0, which no package publishes' },
      { kind: 'simulation', name: 'danger-sign-referral', ok: true, detail: 'Reached state "referral-raised" in 3 steps' },
      { kind: 'simulation', name: 'routine-visit', ok: true, detail: 'Reached state "next-visit-booked" in 4 steps' },
    ],
    history: [
      { at: '2026-11-10 09:12', who: 'nakato', what: 'Created draft from 2.0.0' },
      { at: '2026-11-14 16:40', who: 'okello', what: 'Commented', comment: 'Please align the danger signs with the 2025 WHO ANC list before review.' },
      { at: '2026-11-15 11:03', who: 'nakato', what: 'Edited anc-visit (Questionnaire)' },
    ],
  },
  {
    name: 'ug.moh.terminology',
    title: 'National terminology',
    kind: 'terminology',
    version: '1.4.0',
    previous: '1.3.2',
    bump: 'minor',
    stage: 'in-review',
    author: 'achieng',
    approvals: [],
    reviewer: 'okello',
    activations: [],
    artefacts: [
      { type: 'CodeSystem', name: 'ug-facility-levels', change: 'unchanged', summary: '' },
      { type: 'ValueSet', name: 'anc-danger-signs', change: 'added', summary: '9 codes, expansion pinned' },
      { type: 'ValueSet', name: 'triage-category', change: 'changed', summary: 'Adds Luganda designations' },
    ],
    checks: [
      { kind: 'validation', name: 'Profiles and snapshots', ok: true, detail: 'Valid' },
      { kind: 'validation', name: 'Semver', ok: true, detail: 'Additive: minor' },
      { kind: 'validation', name: 'Expansions', ok: true, detail: 'Every bound ValueSet ships a pinned expansion' },
    ],
    history: [
      { at: '2026-11-12 08:30', who: 'achieng', what: 'Created draft from 1.3.2' },
      { at: '2026-11-15 15:20', who: 'achieng', what: 'Submitted for review' },
      { at: '2026-11-16 10:05', who: 'okello', what: 'Took the review' },
    ],
  },
  {
    name: 'ug.moh.art',
    title: 'ART (HIV treatment)',
    kind: 'program',
    version: '3.0.0',
    previous: '2.4.1',
    bump: 'major',
    stage: 'approved',
    author: 'okello',
    approvals: ['nakato'],
    reviewer: 'nakato',
    activations: [],
    artefacts: [
      { type: 'PlanDefinition', name: 'art-program', change: 'changed', summary: 'Eligibility no longer requires CD4 (test-and-treat)' },
      { type: 'Workflow Definition', name: 'art-episode', change: 'changed', summary: 'Removes "CD4 pending" state (breaking)' },
      { type: 'Questionnaire', name: 'art-followup', change: 'changed', summary: 'Viral load due date computed' },
    ],
    checks: [
      { kind: 'validation', name: 'Semver', ok: true, detail: 'Removed state: major bump required and given' },
      { kind: 'simulation', name: 'new-client-same-day-start', ok: true, detail: 'Reached "on-art" in 2 steps' },
      { kind: 'simulation', name: 'suppressed-vl-dsd', ok: true, detail: 'Reached "dsd-multi-month" in 5 steps' },
    ],
    history: [
      { at: '2026-11-01 10:00', who: 'okello', what: 'Created draft from 2.4.1' },
      { at: '2026-11-08 12:00', who: 'okello', what: 'Submitted for review' },
      { at: '2026-11-11 09:30', who: 'nakato', what: 'Approved (1 of 2)', comment: 'Matches the 2025 consolidated guidelines.' },
    ],
  },
  {
    name: 'ug.moh.lab',
    title: 'Laboratory tests',
    kind: 'program',
    version: '1.2.0',
    previous: '1.1.0',
    bump: 'minor',
    stage: 'active',
    author: 'mugisha',
    approvals: ['okello', 'nakato'],
    reviewer: 'okello',
    activations: [
      { scope: 'National', from: '2026-10-01' },
      { scope: 'Kampala district', from: '2026-09-15' },
    ],
    artefacts: [{ type: 'Workflow Definition', name: 'lab-test', change: 'changed', summary: 'Adds rejection with reason' }],
    checks: [{ kind: 'validation', name: 'Semver', ok: true, detail: 'minor' }],
    history: [
      { at: '2026-09-02 10:00', who: 'mugisha', what: 'Published 1.2.0' },
      { at: '2026-09-10 09:00', who: 'okello', what: 'Activated for Kampala district from 2026-09-15' },
      { at: '2026-09-28 09:00', who: 'okello', what: 'Activated nationally from 2026-10-01' },
    ],
  },
  {
    name: 'ug.kampala.override',
    title: 'Kampala district overrides',
    kind: 'override',
    version: '0.3.0',
    previous: '0.2.0',
    bump: 'minor',
    stage: 'changes-requested',
    author: 'mugisha',
    approvals: [],
    reviewer: 'okello',
    activations: [],
    artefacts: [{ type: 'Form Layout', name: 'opd-triage', change: 'changed', summary: 'Moves weight before temperature' }],
    checks: [{ kind: 'validation', name: 'Override points', ok: true, detail: 'Only declared override points are set' }],
    history: [
      { at: '2026-11-13 14:00', who: 'mugisha', what: 'Submitted for review' },
      { at: '2026-11-14 09:15', who: 'okello', what: 'Requested changes', comment: 'Weight-first breaks the paediatric dosing prompt order; keep temperature first for under-5s.' },
    ],
  },
]

export const STAGES: { key: Stage; label: string }[] = [
  { key: 'draft', label: 'Draft' },
  { key: 'changes-requested', label: 'Changes requested' },
  { key: 'in-review', label: 'In review' },
  { key: 'approved', label: 'Approved' },
  { key: 'published', label: 'Published' },
  { key: 'active', label: 'Active' },
]

export const person = (id: string | null) => PEOPLE.find((p) => p.id === id)

/** What `me` may do next with `pkg`, and why not when they may not: ADR 0025's rules, visible. */
export function actions(pkg: Pkg, me: string): { label: string; allowed: boolean; why?: string | undefined }[] {
  const failing = pkg.checks.some((c) => !c.ok)
  const mine = pkg.author === me
  const need = approversNeeded(pkg.kind)
  switch (pkg.stage) {
    case 'draft':
    case 'changes-requested':
      return [
        { label: 'Edit', allowed: mine, why: mine ? undefined : `Only the author, ${person(pkg.author)?.name}, edits. Ask them to hand it over.` },
        { label: 'Run checks', allowed: true },
        { label: 'Submit for review', allowed: mine && !failing, why: !mine ? 'Only the author submits.' : failing ? 'Fix the failing checks first.' : undefined },
        { label: 'Hand over', allowed: mine, why: mine ? undefined : 'Only the author hands a draft over.' },
      ]
    case 'in-review':
      return [
        { label: 'Approve', allowed: !mine && !pkg.approvals.includes(me), why: mine ? 'You wrote this draft, so you cannot review or approve it.' : undefined },
        { label: 'Request changes', allowed: !mine, why: mine ? 'You wrote this draft.' : undefined },
      ]
    case 'approved': {
      const more = need - pkg.approvals.length
      return [
        { label: 'Approve', allowed: more > 0 && !mine && !pkg.approvals.includes(me), why: mine ? 'You wrote this draft.' : pkg.approvals.includes(me) ? 'You already approved; a second approver must be someone else.' : more <= 0 ? 'Enough approvals.' : undefined },
        { label: 'Publish', allowed: more <= 0, why: more > 0 ? `${more} more approval${more > 1 ? 's' : ''} needed (${pkg.kind} needs ${need}).` : undefined },
      ]
    }
    case 'published':
    case 'active':
      return [{ label: 'Activate for a scope…', allowed: true }, { label: 'New draft from this version', allowed: true }]
  }
}
