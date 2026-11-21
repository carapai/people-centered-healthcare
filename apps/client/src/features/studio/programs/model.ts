// The Program Designer's model (program-designer.md § The Program Designer; #500), as pure
// functions. A program is a ProgramPlanDefinition (ADR 0049): a `clinical-protocol`
// PlanDefinition whose `relatedArtifact`s cite its approved protocol sources and name, as
// `composed-of`, the artefacts it is made of. The journey board's stages are the top-level
// states of its care workflow, the first Workflow Definition it names, so stages are added and
// reordered in the Workflow Studio, never here. What the workflow does not say sits under the
// plan's top-level `action` whose `linkId` names a state: its investigations (actions with an
// ActivityDefinition) and its regimen templates (a group with `selectionBehavior`
// `at-most-one` whose rules are taken in order, the first that holds choosing the regimen,
// `health_programs_app::program` on the server). The designer assembles; it does not edit,
// except the order of the regimen rules, which it saves like any other file.

import type { StudioArtefactChange, StudioPackageFile } from '@health/api-client'

import { isWorkflowLibrary, WORKFLOW_CONTENT_TYPE, type Action, type State, type Timer, type Value, type WorkflowDefinition } from '../workflows/model'

/** A PlanDefinition as JSON. */
export type Plan = Record<string, unknown> & { resourceType: 'PlanDefinition' }

type Json = Record<string, unknown>

const EPISODE_TYPE = 'https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/program-episode-type'
const ELIGIBILITY = 'https://carapai.github.io/people-centered-healthcare/fhir/StructureDefinition/program-eligibility'
const PLATFORM_CODES = 'https://carapai.github.io/people-centered-healthcare/fhir/CodeSystem/platform'

const isObject = (value: unknown): value is Json => typeof value === 'object' && value !== null && !Array.isArray(value)
const objects = (value: unknown): Json[] => (Array.isArray(value) ? value.filter(isObject) : [])
const text = (value: unknown): string | null => (typeof value === 'string' && value.trim() !== '' ? value : null)

/** Whether `value` is a program: a PlanDefinition of type `clinical-protocol`. */
export function isProgramPlan(value: unknown): value is Plan {
  if (!isObject(value) || value['resourceType'] !== 'PlanDefinition') return false
  const type = isObject(value['type']) ? value['type'] : {}
  return objects(type['coding']).some((c) => c['code'] === 'clinical-protocol')
}

export type ReadRefusal = 'not-json' | 'not-program'

/** The program a PlanDefinition's text holds, or why it holds none. */
export function readPlan(source: string): { plan: Plan } | { refusal: ReadRefusal } {
  let value: unknown
  try {
    value = JSON.parse(source)
  } catch {
    return { refusal: 'not-json' }
  }
  return isProgramPlan(value) ? { plan: value } : { refusal: 'not-program' }
}

// ---------------------------------------------------------------------------------------------
// Reading the plan

/** One regimen template: a rule of a stage's ordered list. */
export interface Rule {
  title: string
  /** When it applies: FHIRPath over the patient. */
  condition: string | null
  /** The ActivityDefinition it uses, `url|version`. */
  activity: string | null
  /** The protocol sources it cites. */
  cites: string[]
}

/** What the plan says about one stage: its investigations and regimen templates. */
export interface StagePlan {
  stage: string
  /** Investigations and orders: ActivityDefinitions, `url|version`. */
  investigations: string[]
  /** The regimen templates in order; null when the stage chooses no regimen. */
  regimens: Rule[] | null
}

export interface Program {
  url: string
  version: string
  title: string
  episodeType: string
  /** Who may enrol, FHIRPath over the patient; null takes everyone. */
  eligibility: string | null
  /** Its Workflow Definitions (Libraries), in order: the first is the care workflow. */
  workflows: string[]
  questionnaires: string[]
  activities: string[]
  measures: string[]
  /** The approved protocol sources it cites. */
  sources: string[]
  /** What a person reads for a source, where the plan gives it (`display`). */
  sourceNames: Record<string, string>
  stages: StagePlan[]
}

/** What a citing RelatedArtifact names: its resource, its document's URL or its citation. */
function cited(artifact: Json): string | null {
  const document = isObject(artifact['document']) ? artifact['document'] : {}
  return text(artifact['resource']) ?? text(document['url']) ?? text(artifact['citation'])
}

const citing = (artifacts: unknown) =>
  objects(artifacts)
    .filter((a) => a['type'] === 'citation' || a['type'] === 'documentation' || a['type'] === 'justification')
    .flatMap((a) => {
      const named = cited(a)
      return named === null ? [] : [named]
    })

const extension = (plan: Json, url: string) => objects(plan['extension']).find((e) => e['url'] === url)

const kindOfUrl = (canonical: string) => canonical.split('|')[0]?.split('/').at(-2) ?? ''

/** The program `plan` defines, as the board reads it. */
export function programOf(plan: Plan): Program {
  const episode = extension(plan, EPISODE_TYPE)?.['valueCodeableConcept']
  const eligibility = extension(plan, ELIGIBILITY)?.['valueExpression']
  const composed = objects(plan['relatedArtifact'])
    .filter((a) => a['type'] === 'composed-of')
    .flatMap((a) => {
      const resource = text(a['resource'])
      return resource === null ? [] : [resource]
    })
  const named = (type: string) => composed.filter((c) => kindOfUrl(c) === type)
  return {
    url: text(plan['url']) ?? '',
    version: text(plan['version']) ?? '',
    title: text(plan['title']) ?? text(plan['name']) ?? text(plan['id']) ?? '',
    episodeType: isObject(episode) ? (text(episode['text']) ?? text(objects(episode['coding'])[0]?.['display']) ?? text(objects(episode['coding'])[0]?.['code']) ?? '') : '',
    eligibility: isObject(eligibility) ? text(eligibility['expression']) : null,
    workflows: named('Library'),
    questionnaires: named('Questionnaire'),
    activities: named('ActivityDefinition'),
    measures: named('Measure'),
    sources: citing(plan['relatedArtifact']),
    sourceNames: Object.fromEntries(
      objects(plan['relatedArtifact']).flatMap((a) => {
        const source = cited(a)
        const display = text(a['display'])
        return source === null || display === null ? [] : [[source, display]]
      }),
    ),
    stages: objects(plan['action']).flatMap((stage) => {
      const id = text(stage['linkId'])
      if (id === null) return []
      const children = objects(stage['action'])
      const group = children.find((c) => c['selectionBehavior'] === 'at-most-one')
      return [
        {
          stage: id,
          investigations: children.flatMap((c) => {
            const canonical = text(c['definitionCanonical'])
            return c === group || canonical === null || kindOfUrl(canonical) !== 'ActivityDefinition' ? [] : [canonical]
          }),
          regimens: group === undefined ? null : objects(group['action']).map(ruleOf),
        },
      ]
    }),
  }
}

function ruleOf(action: Json): Rule {
  const condition = objects(action['condition']).find((c) => c['kind'] === 'applicability')
  const expression = isObject(condition?.['expression']) ? condition['expression'] : {}
  return {
    title: text(action['title']) ?? '',
    condition: text(expression['expression']),
    activity: text(action['definitionCanonical']),
    cites: citing(action['documentation']),
  }
}

/** `plan` with the regimen rule at `index` of `stage`'s templates moved `by` places (−1 up,
 * +1 down); everything else as read. Unchanged when the move leaves the list. */
export function moveRule(plan: Plan, stage: string, index: number, by: -1 | 1): Plan {
  const to = index + by
  const stages = objects(plan['action'])
  const at = stages.findIndex((s) => s['linkId'] === stage)
  const children = objects(stages[at]?.['action'])
  const groupAt = children.findIndex((c) => c['selectionBehavior'] === 'at-most-one')
  const group = children[groupAt]
  const rules = objects(group?.['action'])
  if (group === undefined || index < 0 || to < 0 || index >= rules.length || to >= rules.length) return plan
  const moved = [...rules]
  ;[moved[index], moved[to]] = [rules[to] as Json, rules[index] as Json]
  const nextChildren = children.map((c, n) => (n === groupAt ? { ...group, action: moved } : c))
  const nextStages = stages.map((s, n) => (n === at ? { ...s, action: nextChildren } : s))
  return { ...plan, action: nextStages }
}

// ---------------------------------------------------------------------------------------------
// Where each linked artefact stands

export type ArtefactKind = 'Questionnaire' | 'Library' | 'ActivityDefinition' | 'Measure'

/** in this draft; published, pinned to a version; or not created yet. */
export type Where = 'draft' | 'published' | 'missing'

export interface Linked {
  kind: ArtefactKind
  /** As the plan or the workflow names it, `url` or `url|version`. */
  canonical: string
  url: string
  /** The version it is pinned to, if the reference pins one. */
  pinned: string | null
  where: Where
  /** The file that holds it in this package; null when the package does not. */
  path: string | null
  /** The version the package holds, or the version a dependency pins. */
  version: string | null
}

/** What the package holds: its files, what changed against its previous published version,
 * whether it has one, and the canonicals its manifest pins as `depends-on`. */
export interface Holdings {
  files: StudioPackageFile[]
  changes: StudioArtefactChange[]
  hasPrevious: boolean
  dependsOn: string[]
}

/** Where `canonical`, an artefact of `kind`, stands. Held and added or changed in this draft
 * (or in a first version): in this draft. Held unchanged since the previous published version,
 * or pinned by a dependency: published. Otherwise: missing. */
export function resolve(canonical: string, kind: ArtefactKind, held: Holdings): Linked {
  const [url = '', pinned = null] = canonical.split('|') as [string?, string?]
  const file = held.files.find((f) => f.url === url && f.resourceType === kind)
  if (file !== undefined) {
    const changed = held.changes.some((c) => c.path === file.path && c.change !== 'removed')
    return { kind, canonical, url, pinned, where: !held.hasPrevious || changed ? 'draft' : 'published', path: file.path, version: file.version ?? null }
  }
  const dependency = held.dependsOn.find((d) => d.split('|')[0] === url)
  if (dependency !== undefined) return { kind, canonical, url, pinned, where: 'published', path: null, version: dependency.split('|')[1] ?? pinned }
  return { kind, canonical, url, pinned, where: 'missing', path: null, version: null }
}

/** The `depends-on` canonicals of a manifest Library's text. */
export function dependsOn(manifest: string | undefined): string[] {
  if (manifest === undefined) return []
  try {
    const value: unknown = JSON.parse(manifest)
    if (!isObject(value)) return []
    return objects(value['relatedArtifact']).flatMap((a) => {
      const resource = text(a['resource'])
      return a['type'] === 'depends-on' && resource !== null ? [resource] : []
    })
  } catch {
    return []
  }
}

/** How many override points a base package declares in its `configuration/override-points.json`. */
export function overridePoints(source: string | undefined): number {
  if (source === undefined) return 0
  try {
    const value: unknown = JSON.parse(source)
    return isObject(value) && Array.isArray(value['points']) ? value['points'].length : 0
  } catch {
    return 0
  }
}

export const OVERRIDE_POINTS = 'configuration/override-points.json'
export const MANIFEST = 'Library-manifest.json'

// ---------------------------------------------------------------------------------------------
// The board

/** Something that happens at a stage, from the care workflow. */
export type Happening =
  | { kind: 'task'; role: string; worklist: string }
  | { kind: 'gate'; gate: string }
  | { kind: 'invoke'; src: string }
  | { kind: 'execute'; action: string }
  | { kind: 'raise'; event: string }
  | { kind: 'assign'; field: string; value: Value }
  | { kind: 'inside'; states: string[] }

/** One way out of a stage. */
export interface Exit {
  targets: string[]
  /** The events it is taken on; empty for a timer's or an eventless one. */
  events: string[]
  /** The timer it waits for, ISO 8601, when its event is a timer's. */
  after: string | null
  guard: string | null
}

export interface RegimenRow {
  rule: Rule
  activity: Linked | null
  /** Whether it cites one of the program's approved protocol sources. */
  approved: boolean
}

export interface Column {
  id: string
  kind: State['kind']
  forms: Linked[]
  happens: Happening[]
  investigations: Linked[]
  regimens: RegimenRow[] | null
  /** The stage's timers: its schedule. */
  schedule: Timer[]
  /** Its outcomes: itself when final, else its final children. */
  outcomes: string[]
  indicators: Linked[]
  leaves: Exit[]
}

const descendants = (state: State): State[] => (state.children ?? []).flatMap((c) => [c, ...descendants(c)])

const actionsOf = (state: State): Action[] => (state.on_entry ?? []).flat()

/** ActivityDefinitions a typed action names in its parameters. */
const activitiesIn = (actions: Action[]) =>
  actions.flatMap((a) => (a.kind === 'execute' ? Object.values(a.params ?? {}).filter((v): v is string => typeof v === 'string' && kindOfUrl(v) === 'ActivityDefinition') : []))

const unique = <T>(items: T[], key: (item: T) => string) => items.filter((item, n) => items.findIndex((other) => key(other) === key(item)) === n)

/** The journey board: one column per top-level state of the care workflow, in its order. */
export function board(program: Program, definition: WorkflowDefinition, held: Holdings): Column[] {
  const states = definition.statechart.states
  const measures = program.measures.map((m) => resolve(m, 'Measure', held))
  const columns = states.map((state): Column => {
    const all = [state, ...descendants(state)]
    const timers = new Map((state.timers ?? []).map((t) => [t.event, t.after]))
    const planned = program.stages.find((s) => s.stage === state.id)
    const children = state.children ?? []
    const happens: Happening[] = [
      ...(children.length === 0 ? [] : [{ kind: 'inside' as const, states: children.map((c) => c.id) }]),
      ...(state.task === null || state.task === undefined ? [] : [{ kind: 'task' as const, role: state.task.role, worklist: state.task.worklist }]),
      ...(state.gate === null || state.gate === undefined ? [] : [{ kind: 'gate' as const, gate: state.gate }]),
      ...(state.invoke ?? []).map((i) => ({ kind: 'invoke' as const, src: i.src })),
      ...actionsOf(state).map((a): Happening => (a.kind === 'execute' ? { kind: 'execute', action: a.action } : a.kind === 'raise' ? { kind: 'raise', event: a.event } : { kind: 'assign', field: a.field, value: a.value })),
    ]
    return {
      id: state.id,
      kind: state.kind,
      forms: unique(
        all.flatMap((s) => (typeof s.task?.form === 'string' ? [resolve(s.task.form, 'Questionnaire', held)] : [])),
        (l) => l.canonical,
      ),
      happens,
      investigations: unique(
        [...(planned?.investigations ?? []), ...all.flatMap((s) => activitiesIn(actionsOf(s)))].map((c) => resolve(c, 'ActivityDefinition', held)),
        (l) => l.url,
      ),
      regimens:
        planned?.regimens === null || planned?.regimens === undefined
          ? null
          : planned.regimens.map((rule) => ({
              rule,
              activity: rule.activity === null ? null : resolve(rule.activity, 'ActivityDefinition', held),
              approved: rule.cites.some((c) => program.sources.includes(c)),
            })),
      schedule: state.timers ?? [],
      outcomes: state.kind === 'final' ? [state.id] : descendants(state).filter((s) => s.kind === 'final').map((s) => s.id),
      indicators: [],
      leaves: (state.transitions ?? []).map((t) => {
        const events = t.events ?? []
        const timer = events.length === 1 ? (timers.get(events[0] ?? '') ?? null) : null
        return { targets: t.targets ?? [], events: timer === null ? events : [], after: timer, guard: t.guard ?? null }
      }),
    }
  })
  // The indicators count the journeys' ends: they sit on the last stage with outcomes.
  const ends = columns.flatMap((c, n) => (c.outcomes.length > 0 ? [n] : []))
  const at = ends.at(-1) ?? columns.length - 1
  return columns.map((c, n) => (n === at ? { ...c, indicators: measures } : c))
}

// ---------------------------------------------------------------------------------------------
// Still missing, and the package panel

export type Gap =
  | { kind: 'missing'; linked: Linked }
  | { kind: 'no-care-workflow' }
  | { kind: 'unknown-stage'; stage: string }
  | { kind: 'uncited'; stage: string; title: string }
  | { kind: 'no-condition'; stage: string; title: string }
  | { kind: 'no-activity'; stage: string; title: string }
  | { kind: 'pinned'; linked: Linked }

/** What the program still lacks: each missing artefact once, regimen templates without their
 * condition, ActivityDefinition or approved source, stages the care workflow does not have,
 * and pins to a version the package does not hold. *Check now* remains the authority. */
export function gaps(program: Program, careWorkflow: Linked | null, columns: Column[] | null, held: Holdings): Gap[] {
  const out: Gap[] = []
  if (careWorkflow === null) out.push({ kind: 'no-care-workflow' })
  const linked = [
    ...(careWorkflow === null ? [] : [careWorkflow]),
    ...program.workflows.slice(1).map((w) => resolve(w, 'Library', held)),
    ...program.questionnaires.map((q) => resolve(q, 'Questionnaire', held)),
    ...(columns ?? []).flatMap((c) => [...c.forms, ...c.investigations, ...(c.regimens ?? []).flatMap((r) => (r.activity === null ? [] : [r.activity])), ...c.indicators]),
    ...program.activities.map((a) => resolve(a, 'ActivityDefinition', held)),
    ...program.measures.map((m) => resolve(m, 'Measure', held)),
  ]
  for (const l of unique(linked, (x) => x.url)) {
    if (l.where === 'missing') out.push({ kind: 'missing', linked: l })
    else if (l.path !== null && l.pinned !== null && l.version !== null && l.pinned !== l.version) out.push({ kind: 'pinned', linked: l })
  }
  const stages = new Set((columns ?? []).map((c) => c.id))
  for (const stage of program.stages) {
    if (columns !== null && !stages.has(stage.stage)) out.push({ kind: 'unknown-stage', stage: stage.stage })
    for (const rule of stage.regimens ?? []) {
      if (rule.condition === null) out.push({ kind: 'no-condition', stage: stage.stage, title: rule.title })
      if (rule.activity === null) out.push({ kind: 'no-activity', stage: stage.stage, title: rule.title })
      if (!rule.cites.some((c) => program.sources.includes(c))) out.push({ kind: 'uncited', stage: stage.stage, title: rule.title })
    }
  }
  return out
}

export const FOLDERS = ['questionnaires/', 'workflows/', 'activitydefinitions/', 'mappings/', 'measures/', 'plans/', 'terminology/', 'configuration/', 'tests/', 'other/'] as const
export type Folder = (typeof FOLDERS)[number]

/** The folder a file or artefact is shown under. A package keeps its artefacts at the top level
 * (`health_packages`), so these folders group them by kind: a Library by what it holds. */
export function folderOf(path: string, resourceType: string | null | undefined, workflows: Set<string>): Folder {
  if (path.startsWith('tests/')) return 'tests/'
  if (path.startsWith('configuration/')) return 'configuration/'
  switch (resourceType) {
    case 'Questionnaire':
      return 'questionnaires/'
    case 'ActivityDefinition':
      return 'activitydefinitions/'
    case 'Measure':
      return 'measures/'
    case 'PlanDefinition':
      return 'plans/'
    case 'CodeSystem':
    case 'ValueSet':
    case 'ConceptMap':
      return 'terminology/'
    case 'Library':
      return workflows.has(path) ? 'workflows/' : path === MANIFEST ? 'other/' : 'mappings/'
    default:
      return 'other/'
  }
}

const folderOfKind = (kind: ArtefactKind): Folder =>
  kind === 'Questionnaire' ? 'questionnaires/' : kind === 'Library' ? 'workflows/' : kind === 'ActivityDefinition' ? 'activitydefinitions/' : 'measures/'

export interface PanelEntry {
  /** The file's path, or for a missing artefact the canonical it is named by. */
  label: string
  path: string | null
  resourceType: string | null
  missing: Linked | null
}

/** The package panel: the files by folder, with each missing artefact under the folder it
 * would go in. Folders with nothing in them are left out. */
export function panel(files: StudioPackageFile[], workflows: Set<string>, missing: Linked[]): { folder: Folder; entries: PanelEntry[] }[] {
  return FOLDERS.map((folder) => ({
    folder,
    entries: [
      ...files.filter((f) => folderOf(f.path, f.resourceType, workflows) === folder).map((f) => ({ label: f.path, path: f.path, resourceType: f.resourceType ?? null, missing: null })),
      ...missing.filter((l) => folderOfKind(l.kind) === folder).map((l) => ({ label: l.canonical, path: null, resourceType: l.kind, missing: l })),
    ],
  })).filter((f) => f.entries.length > 0)
}

// ---------------------------------------------------------------------------------------------
// Create

const pascal = (id: string) =>
  id
    .split(/[^A-Za-z0-9]+/)
    .filter((part) => part !== '')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('')
    .replace(/^[^A-Za-z]+/, '') || 'Artefact'

const encode = (source: string) => btoa(Array.from(new TextEncoder().encode(source), (b) => String.fromCodePoint(b)).join(''))

/** A new, empty artefact for `linked`, at a top-level path the package does not use yet, with
 * its url and the version it is pinned to (else the package's version). Create writes it to the
 * draft and opens it in its editor. */
export function newArtefact(linked: Linked, packageVersion: string, taken: string[]): { path: string; resource: Json } {
  const id = linked.url.split('/').at(-1) || 'new'
  const version = linked.pinned ?? packageVersion
  const base = `${linked.kind}-${id}`
  let path = `${base}.json`
  for (let n = 2; taken.includes(path); n += 1) path = `${base}-${n}.json`
  const common = { id, url: linked.url, version, name: pascal(id), title: id, status: 'active' }
  switch (linked.kind) {
    case 'Questionnaire':
      return { path, resource: { resourceType: 'Questionnaire', ...common, item: [] } }
    case 'Library': {
      const definition: WorkflowDefinition = { url: linked.url, version, statechart: { states: [{ id: 'start', kind: 'final' }], initial: ['start'] } }
      return {
        path,
        resource: {
          resourceType: 'Library',
          ...common,
          type: { coding: [{ system: PLATFORM_CODES, code: 'workflow-definition' }] },
          content: [{ contentType: WORKFLOW_CONTENT_TYPE, data: encode(JSON.stringify(definition, null, 2)) }],
        },
      }
    }
    case 'ActivityDefinition':
      return { path, resource: { resourceType: 'ActivityDefinition', ...common } }
    case 'Measure':
      return { path, resource: { resourceType: 'Measure', ...common } }
  }
}

/** The editor an artefact opens in: the form designer, the Workflow Studio, or else the file
 * editor (the ActivityDefinition and Measure editors are not built yet). */
export function editorOf(kind: string | null, path: string, workflows: Set<string>): 'forms' | 'workflows' | 'terminology' | 'programs' | 'files' {
  if (kind === 'Questionnaire') return 'forms'
  if (kind === 'CodeSystem' || kind === 'ValueSet') return 'terminology'
  if (kind === 'Library' && workflows.has(path)) return 'workflows'
  return 'files'
}

export { isWorkflowLibrary }
