import type { StudioPackageFile } from '@health/api-client'
import { describe, expect, test } from 'vitest'

import f_ActivityDefinition_order_cd4 from '../e2e/fixtures/art/ActivityDefinition-order-cd4.json'
import f_ActivityDefinition_order_hiv_vl from '../e2e/fixtures/art/ActivityDefinition-order-hiv-vl.json'
import f_ActivityDefinition_rx_abc_3tc_dtg from '../e2e/fixtures/art/ActivityDefinition-rx-abc-3tc-dtg.json'
import f_ActivityDefinition_rx_tld from '../e2e/fixtures/art/ActivityDefinition-rx-tld.json'
import f_Library_art_care from '../e2e/fixtures/art/Library-art-care.json'
import f_Library_manifest from '../e2e/fixtures/art/Library-manifest.json'
import f_Measure_tx_curr from '../e2e/fixtures/art/Measure-tx-curr.json'
import f_PlanDefinition_art_program from '../e2e/fixtures/art/PlanDefinition-art-program.json'
import f_Questionnaire_art_baseline from '../e2e/fixtures/art/Questionnaire-art-baseline.json'
import f_Questionnaire_art_enrolment from '../e2e/fixtures/art/Questionnaire-art-enrolment.json'
import f_Questionnaire_art_followup from '../e2e/fixtures/art/Questionnaire-art-followup.json'
import f_Questionnaire_art_initiation from '../e2e/fixtures/art/Questionnaire-art-initiation.json'
import f_package from '../e2e/fixtures/art/package.json'
import {
  board,
  dependsOn,
  editorOf,
  folderOf,
  gaps,
  moveRule,
  newArtefact,
  overridePoints,
  panel,
  programOf,
  readPlan,
  resolve,
  type Holdings,
  type Plan,
} from '../src/features/studio/programs/model'
import { readDefinition, type WorkflowDefinition } from '../src/features/studio/workflows/model'

// #500: the Program Designer's model over the synthetic ART program package the e2e backend
// drafts (e2e/fixtures/art): its plan, its care workflow and its files.
const fixtures: Record<string, unknown> = {
  'ActivityDefinition-order-cd4.json': f_ActivityDefinition_order_cd4,
  'ActivityDefinition-order-hiv-vl.json': f_ActivityDefinition_order_hiv_vl,
  'ActivityDefinition-rx-abc-3tc-dtg.json': f_ActivityDefinition_rx_abc_3tc_dtg,
  'ActivityDefinition-rx-tld.json': f_ActivityDefinition_rx_tld,
  'Library-art-care.json': f_Library_art_care,
  'Library-manifest.json': f_Library_manifest,
  'Measure-tx-curr.json': f_Measure_tx_curr,
  'PlanDefinition-art-program.json': f_PlanDefinition_art_program,
  'Questionnaire-art-baseline.json': f_Questionnaire_art_baseline,
  'Questionnaire-art-enrolment.json': f_Questionnaire_art_enrolment,
  'Questionnaire-art-followup.json': f_Questionnaire_art_followup,
  'Questionnaire-art-initiation.json': f_Questionnaire_art_initiation,
  'package.json': f_package,
}
const read = (name: string) => JSON.stringify(fixtures[name])
const files: StudioPackageFile[] = Object.entries(fixtures).map(([path, json]) => {
  const value = json as { resourceType?: string; url?: string; version?: string }
  return { path, resourceType: value.resourceType ?? null, url: value.url ?? null, version: value.version ?? null }
})
const plan = (() => {
  const result = readPlan(read('PlanDefinition-art-program.json'))
  if ('refusal' in result) throw new Error(result.refusal)
  return result.plan
})()
const care = (() => {
  const result = readDefinition(read('Library-art-care.json'))
  if ('refusal' in result) throw new Error(result.refusal)
  return result.definition
})()
const first: Holdings = { files, changes: [], hasPrevious: false, dependsOn: [] }
const B = 'https://health.go.ug/fhir'
const titles = (p: Plan) => programOf(p).stages.find((s) => s.stage === 'initiation')?.regimens?.map((r) => r.title)
const missing = (canonical: string, kind: 'Questionnaire' | 'Library' | 'Measure') => resolve(canonical, kind, first)

describe('the ART program package', () => {
  const program = programOf(plan)

  test('is read from its PlanDefinition', () => {
    expect(program.title).toBe('HIV care and antiretroviral therapy (ART)')
    expect(program.episodeType).toBe('HIV care')
    expect(program.eligibility).toContain("'positive'")
    expect(program.workflows).toEqual([`${B}/Library/art-care|1.0.0`])
    expect(program.sources).toEqual(['https://health.go.ug/protocols/consolidated-hiv-guidelines-2024'])
    expect(program.stages.map((s) => s.stage)).toEqual(['baseline', 'initiation', 'follow-up'])
    expect(program.stages[1]?.regimens?.map((r) => r.title)).toEqual(['TDF/3TC/DTG, one tablet daily', 'ABC/3TC with DTG, by weight band', 'TLD with an extra DTG 50 mg twelve hours later'])
    expect(program.stages[0]?.regimens).toBeNull()
  })

  test('renders one column per care-workflow state, in order', () => {
    const columns = board(program, care, first)
    expect(columns.map((c) => c.id)).toEqual(care.statechart.states.map((s) => s.id))
    expect(columns.map((c) => c.id)).toEqual(['enrolment', 'baseline', 'initiation', 'follow-up', 'outcome', 'closed'])
    const [enrolment, baseline, initiation, followUp, outcome, closed] = columns
    expect(enrolment?.forms.map((f) => [f.canonical, f.where])).toEqual([[`${B}/Questionnaire/art-enrolment|1.0.0`, 'draft']])
    expect(enrolment?.happens).toEqual([{ kind: 'task', role: 'clinician', worklist: 'art-enrolment' }])
    expect(enrolment?.leaves).toEqual([{ targets: ['baseline'], events: ['art.enrolled'], after: null, guard: null }])
    expect(baseline?.investigations.map((i) => [i.url, i.where])).toEqual([[`${B}/ActivityDefinition/order-cd4`, 'draft']])
    // The regimen templates sit on initiation; the TB rule's ActivityDefinition is missing.
    expect(initiation?.regimens?.map((r) => [r.rule.title, r.activity?.where, r.approved])).toEqual([
      ['TDF/3TC/DTG, one tablet daily', 'draft', true],
      ['ABC/3TC with DTG, by weight band', 'draft', true],
      ['TLD with an extra DTG 50 mg twelve hours later', 'missing', true],
    ])
    expect(baseline?.regimens).toBeNull()
    // Follow-up: its schedule, and the missed visit's timer as a way out.
    expect(followUp?.schedule).toEqual([{ after: 'P28D', event: 'art.missed-visit' }])
    expect(followUp?.leaves[1]).toEqual({ targets: ['outcome'], events: [], after: 'P28D', guard: null })
    // The exit form is missing; the journey ends at closed, where the indicators are.
    expect(outcome?.forms.map((f) => f.where)).toEqual(['missing'])
    expect(closed?.outcomes).toEqual(['closed'])
    expect(closed?.indicators.map((i) => [i.url, i.where])).toEqual([
      [`${B}/Measure/tx-curr`, 'draft'],
      [`${B}/Measure/art-tracing`, 'missing'],
    ])
    expect(columns.slice(0, 5).every((c) => c.indicators.length === 0)).toBe(true)
  })

  test('lists what is still missing, each once', () => {
    const found = gaps(program, resolve(program.workflows[0] ?? '', 'Library', first), board(program, care, first), first)
    expect(found.map((g) => (g.kind === 'missing' ? `${g.linked.kind} ${g.linked.url}` : g.kind))).toEqual([
      `Questionnaire ${B}/Questionnaire/art-exit`,
      `ActivityDefinition ${B}/ActivityDefinition/rx-tld-extra-dtg`,
      `Measure ${B}/Measure/art-tracing`,
    ])
  })

  test('shows the files by kind, with the missing ones under the folder they would go in', () => {
    const absent = gaps(program, null, board(program, care, first), first).flatMap((g) => (g.kind === 'missing' ? [g.linked] : []))
    const groups = panel(files, new Set(['Library-art-care.json']), absent)
    expect(groups.map((g) => g.folder)).toEqual(['questionnaires/', 'workflows/', 'activitydefinitions/', 'measures/', 'plans/', 'other/'])
    expect(groups.find((g) => g.folder === 'questionnaires/')?.entries.map((e) => [e.label, e.missing === null])).toEqual([
      ['Questionnaire-art-baseline.json', true],
      ['Questionnaire-art-enrolment.json', true],
      ['Questionnaire-art-followup.json', true],
      ['Questionnaire-art-initiation.json', true],
      [`${B}/Questionnaire/art-exit|1.0.0`, false],
    ])
  })
})

describe('where a linked artefact stands', () => {
  const url = `${B}/Questionnaire/art-followup`
  test('held in a first version, or added or changed since the published one: in this draft', () => {
    expect(resolve(`${url}|1.0.0`, 'Questionnaire', first)).toMatchObject({ where: 'draft', path: 'Questionnaire-art-followup.json', pinned: '1.0.0', version: '1.0.0' })
    const changed: Holdings = { ...first, hasPrevious: true, changes: [{ change: 'changed', path: 'Questionnaire-art-followup.json', resourceType: 'Questionnaire', elements: [] }] }
    expect(resolve(url, 'Questionnaire', changed).where).toBe('draft')
  })

  test('held unchanged since the published version, or pinned by a dependency: published', () => {
    expect(resolve(url, 'Questionnaire', { ...first, hasPrevious: true }).where).toBe('published')
    const shared = `${B}/Measure/shared`
    const deps = dependsOn(JSON.stringify({ resourceType: 'Library', relatedArtifact: [{ type: 'depends-on', resource: `${shared}|2.1.0` }, { type: 'composed-of', resource: 'x' }] }))
    expect(deps).toEqual([`${shared}|2.1.0`])
    expect(resolve(shared, 'Measure', { ...first, dependsOn: deps })).toMatchObject({ where: 'published', version: '2.1.0', path: null })
  })

  test('held nowhere: missing; a pin to another version than the one held is a gap', () => {
    expect(resolve(`${B}/Questionnaire/nowhere|1.0.0`, 'Questionnaire', first)).toMatchObject({ where: 'missing', path: null })
    const program = programOf({ ...plan, relatedArtifact: [{ type: 'citation', resource: 'https://health.go.ug/protocols/consolidated-hiv-guidelines-2024' }, { type: 'composed-of', resource: `${url}|0.9.0` }] } as Plan)
    expect(gaps(program, null, null, first).map((g) => g.kind)).toEqual(['no-care-workflow', 'pinned'])
  })
})

describe('regimen templates', () => {
  test('reordered: the rule moves, everything else is kept as read, and a move off the list changes nothing', () => {
    const moved = moveRule(plan, 'initiation', 2, -1)
    expect(titles(moved)).toEqual(['TDF/3TC/DTG, one tablet daily', 'TLD with an extra DTG 50 mg twelve hours later', 'ABC/3TC with DTG, by weight band'])
    expect(titles(moveRule(moved, 'initiation', 1, 1))).toEqual(titles(plan))
    expect({ ...moved, action: undefined }).toEqual({ ...plan, action: undefined })
    expect(programOf(moved).stages[0]).toEqual(programOf(plan).stages[0])
    expect(moveRule(plan, 'initiation', 0, -1)).toBe(plan)
    expect(moveRule(plan, 'initiation', 2, 1)).toBe(plan)
    expect(moveRule(plan, 'baseline', 0, 1)).toBe(plan)
  })

  test('without a condition, an ActivityDefinition or an approved source: gaps', () => {
    const odd = {
      ...plan,
      action: [{ linkId: 'initiation', action: [{ selectionBehavior: 'at-most-one', action: [{ title: 'Anyone', documentation: [{ type: 'citation', citation: 'a blog' }] }] }] }, { linkId: 'nowhere', action: [] }],
    } as Plan
    const program = programOf(odd)
    const columns = board(program, care, first)
    expect(columns.find((c) => c.id === 'initiation')?.regimens?.map((r) => [r.activity, r.approved])).toEqual([[null, false]])
    expect(gaps(program, null, columns, first).filter((g) => g.kind !== 'missing')).toEqual([
      { kind: 'no-care-workflow' },
      { kind: 'no-condition', stage: 'initiation', title: 'Anyone' },
      { kind: 'no-activity', stage: 'initiation', title: 'Anyone' },
      { kind: 'uncited', stage: 'initiation', title: 'Anyone' },
      { kind: 'unknown-stage', stage: 'nowhere' },
    ])
  })
})

describe('Create', () => {
  test('a missing form becomes an empty Questionnaire at its url and pinned version, opened in the form designer', () => {
    const made = newArtefact(missing(`${B}/Questionnaire/art-exit|1.0.0`, 'Questionnaire'), '2.0.0', files.map((f) => f.path))
    expect(made).toEqual({
      path: 'Questionnaire-art-exit.json',
      resource: { resourceType: 'Questionnaire', id: 'art-exit', url: `${B}/Questionnaire/art-exit`, version: '1.0.0', name: 'ArtExit', title: 'art-exit', status: 'active', item: [] },
    })
    expect(editorOf('Questionnaire', made.path, new Set())).toBe('forms')
  })

  test('a missing workflow becomes a Workflow Definition the Workflow Studio opens; a taken path is not reused', () => {
    const made = newArtefact(missing(`${B}/Library/art-care`, 'Library'), '2.0.0', ['Library-art-care.json'])
    expect(made.path).toBe('Library-art-care-2.json')
    const opened = readDefinition(JSON.stringify(made.resource))
    expect('definition' in opened ? (opened.definition as WorkflowDefinition) : opened).toMatchObject({ url: `${B}/Library/art-care`, version: '2.0.0', statechart: { states: [{ id: 'start', kind: 'final' }] } })
  })

  test('a missing Measure is created as a count and opens in the indicator editor', () => {
    const made = newArtefact(missing(`${B}/Measure/art-tracing`, 'Measure'), '1.0.0', [])
    expect(made.resource).toMatchObject({ resourceType: 'Measure', name: 'ArtTracing', scoring: { coding: [{ code: 'cohort' }] }, basis: 'Encounter' })
    expect(editorOf('Measure', made.path, new Set())).toBe('indicators')
  })
})

test('the strip counts override points, and the panel groups files by kind', () => {
  expect(overridePoints(JSON.stringify({ points: [{ key: 'a' }, { key: 'b' }] }))).toBe(2)
  expect(overridePoints(undefined)).toBe(0)
  expect(overridePoints('not json')).toBe(0)
  expect(folderOf('tests/run.json', null, new Set())).toBe('tests/')
  expect(folderOf('configuration/override-points.json', null, new Set())).toBe('configuration/')
  expect(folderOf('Library-art-care.json', 'Library', new Set(['Library-art-care.json']))).toBe('workflows/')
  expect(folderOf('Library-art-mapping.json', 'Library', new Set())).toBe('mappings/')
  expect(folderOf('Library-manifest.json', 'Library', new Set())).toBe('other/')
})
