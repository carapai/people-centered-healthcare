import { createIntl } from 'react-intl'
import { describe, expect, test } from 'vitest'

import opdVisit from '../../../packages/reference-packages/opd/Library-opd-visit.json'
import { canonicalOf } from '../src/features/studio/create'
import { designMessages as dm } from '../src/i18n/messages'
import {
  editorOf,
  editPlan,
  icu,
  sections,
  filterDesigns,
  inSection,
  newDesign,
  noFilter,
  packageChoices,
  slug,
  statusOf,
  withNewTable,
  type StudioDesign,
  type StudioDesigns,
} from '../src/features/studio/designs'
import { readDefinition } from '../src/features/studio/workflows/model'

// #855: the Studio starts from what is designed. Central lists each design in the version it
// opens in; the client sorts them into sections, says where each stands, and works out what
// Edit and New do.

const design = (kind: StudioDesign['kind'], title: string, pkg: string, packageId: string, path: string, table: string | null = null): StudioDesign => ({
  kind,
  title,
  path,
  table,
  url: null,
  version: null,
  package: pkg,
  packageId,
})

const designs: StudioDesigns = {
  packages: [
    { name: 'ug.moh.anc', title: 'ANC', kind: 'program', published: 'ug.moh.anc@1.0.0', draft: null, canonical: 'https://health.go.ug/fhir', changed: '2026-11-20T09:00:00Z' },
    { name: 'ug.moh.art', title: 'ART', kind: 'program', published: null, draft: { id: 'ug.moh.art@1.0.0', stage: 'draft', holder: 'ann', mine: true }, canonical: null, changed: null },
    { name: 'ug.moh.opd', title: 'OPD', kind: 'program', published: 'ug.moh.opd@1.0.0', draft: { id: 'ug.moh.opd@1.1.0-draft', stage: 'draft', holder: 'bob', mine: false }, canonical: null, changed: null },
    { name: 'ug.moh.tb', title: 'TB', kind: 'program', published: null, draft: { id: 'ug.moh.tb@0.1.0', stage: 'review', holder: 'ann', mine: true }, canonical: null, changed: null },
    { name: 'ug.moh.hmis', title: 'HMIS', kind: 'reporting', published: null, draft: { id: 'ug.moh.hmis@1.0.0', stage: 'review', holder: 'bob', mine: false }, canonical: null, changed: null },
  ],
  designs: [
    design('form', 'ANC visit', 'ug.moh.anc', 'ug.moh.anc@1.0.0', 'Questionnaire-anc-visit.json'),
    design('workflow', 'ART care', 'ug.moh.art', 'ug.moh.art@1.0.0', 'Library-art-care.json'),
    design('decision-table', 'regimen', 'ug.moh.art', 'ug.moh.art@1.0.0', 'Library-art-care.json', 'regimen'),
    design('form', 'OPD triage', 'ug.moh.opd', 'ug.moh.opd@1.1.0-draft', 'Questionnaire-opd-triage.json'),
    design('code-system', 'Triage category', 'ug.moh.opd', 'ug.moh.opd@1.1.0-draft', 'CodeSystem-triage-category.json'),
    design('value-set', 'TB symptoms', 'ug.moh.tb', 'ug.moh.tb@0.1.0', 'ValueSet-tb-symptoms.json'),
    design('report', 'HMIS 105 · 2 Antenatal care', 'ug.moh.hmis', 'ug.moh.hmis@1.0.0', 'configuration/reports/hmis-105-anc.json'),
    design('dashboard', 'ANC overview', 'ug.moh.hmis', 'ug.moh.hmis@1.0.0', 'configuration/dashboards/anc-overview.json'),
  ],
}
const pkg = (name: string) => designs.packages.find((p) => p.name === name)

describe('sections', () => {
  test('each section lists its kinds; concepts hold code systems and value sets', () => {
    expect(inSection(designs.designs, 'forms').map((d) => d.title)).toEqual(['ANC visit', 'OPD triage'])
    expect(inSection(designs.designs, 'concepts').map((d) => d.kind)).toEqual(['code-system', 'value-set'])
    expect(inSection(designs.designs, 'decision-tables').map((d) => d.table)).toEqual(['regimen'])
    expect(inSection(designs.designs, 'programs')).toEqual([])
    expect(inSection(designs.designs, 'reports').map((d) => d.title)).toEqual(['HMIS 105 · 2 Antenatal care'])
    expect(inSection(designs.designs, 'dashboards').map((d) => d.title)).toEqual(['ANC overview'])
  })

  test('a design is published, in your draft, in someone else’s, or submitted', () => {
    expect(statusOf(pkg('ug.moh.anc'))).toBe('published')
    expect(statusOf(pkg('ug.moh.art'))).toBe('yours')
    expect(statusOf(pkg('ug.moh.opd'))).toBe('theirs')
    expect(statusOf(pkg('ug.moh.tb'))).toBe('submitted')
  })

  test('search matches every word; package and status filter', () => {
    const forms = inSection(designs.designs, 'forms')
    expect(filterDesigns(designs, forms, { ...noFilter, text: 'opd TRIAGE' }).map((d) => d.title)).toEqual(['OPD triage'])
    expect(filterDesigns(designs, forms, { ...noFilter, text: 'anc' }).map((d) => d.title)).toEqual(['ANC visit'])
    expect(filterDesigns(designs, forms, { ...noFilter, package: 'ug.moh.opd' }).map((d) => d.title)).toEqual(['OPD triage'])
    expect(filterDesigns(designs, forms, { ...noFilter, status: 'published' }).map((d) => d.title)).toEqual(['ANC visit'])
    expect(filterDesigns(designs, forms, { ...noFilter, status: 'yours' })).toEqual([])
  })

  test('each kind opens in its editor; a decision table opens its workflow at the table', () => {
    expect(editorOf(designs.designs[0]!, 'ug.moh.anc@1.0.0')).toEqual({ to: '/studio/$id/forms/$', params: { id: 'ug.moh.anc@1.0.0', _splat: 'Questionnaire-anc-visit.json' } })
    expect(editorOf(designs.designs[2]!, 'x@1')).toEqual({ to: '/studio/$id/workflows/$', params: { id: 'x@1', _splat: 'Library-art-care.json' }, search: { table: 'regimen' } })
    expect(editorOf(designs.designs[1]!, 'x@1')).toMatchObject({ search: {} })
    expect(editorOf(designs.designs[4]!, 'x@1').to).toBe('/studio/$id/terminology/$')
    // A Report opens in the file editor until the Report designer (#559).
    expect(editorOf(designs.designs[6]!, 'ug.moh.hmis@1.0.0')).toEqual({ to: '/studio/$id/reports/$', params: { id: 'ug.moh.hmis@1.0.0', _splat: 'configuration/reports/hmis-105-anc.json' } })
    expect(sections.find((s) => s.id === 'reports')?.creates).toBe(true)
    // A standard Dashboard opens in the Dashboard designer (#879).
    expect(editorOf(designs.designs[7]!, 'ug.moh.hmis@1.0.0')).toEqual({ to: '/studio/$id/dashboards/$', params: { id: 'ug.moh.hmis@1.0.0', _splat: 'configuration/dashboards/anc-overview.json' } })
    expect(sections.find((s) => s.id === 'dashboards')?.creates).toBe(true)
  })
})

describe('Edit and New', () => {
  test('Edit reuses the draft you hold, starts the next published version, or says who holds it', () => {
    expect(editPlan(pkg('ug.moh.art'))).toEqual({ kind: 'reuse', id: 'ug.moh.art@1.0.0' })
    expect(editPlan(pkg('ug.moh.anc'))).toEqual({ kind: 'start', copyOf: 'ug.moh.anc@1.0.0' })
    expect(editPlan(pkg('ug.moh.opd'))).toEqual({ kind: 'held', holder: 'bob' })
    expect(editPlan(pkg('ug.moh.tb'))).toEqual({ kind: 'submitted', id: 'ug.moh.tb@0.1.0' })
    expect(editPlan(undefined)).toEqual({ kind: 'unknown' })
  })

  test('New offers drafts you hold and published packages, not another’s or a submitted draft', () => {
    expect(packageChoices(designs)).toEqual([
      { name: 'ug.moh.anc', title: 'ANC', plan: { kind: 'start', copyOf: 'ug.moh.anc@1.0.0' } },
      { name: 'ug.moh.art', title: 'ART', plan: { kind: 'reuse', id: 'ug.moh.art@1.0.0' } },
    ])
  })

  test('a new design takes its id from its title, its url from the package and a free path', () => {
    expect(slug('  Antenatal care: 1st visit ')).toBe('antenatal-care-1st-visit')
    expect(slug('!!!')).toBe('new')
    const form = newDesign('form', 'ANC visit', 'https://health.go.ug/fhir/', '1.1.0-draft', ['Questionnaire-anc-visit.json'])
    expect(form.path).toBe('Questionnaire-anc-visit-2.json')
    expect(form.resource).toMatchObject({ resourceType: 'Questionnaire', id: 'anc-visit', url: 'https://health.go.ug/fhir/Questionnaire/anc-visit', version: '1.1.0-draft', title: 'ANC visit', item: [] })
    const workflow = newDesign('workflow', 'ANC visit', 'https://health.go.ug/fhir', '1.0.0', [])
    expect(workflow.path).toBe('Library-anc-visit.json')
    const read = readDefinition(JSON.stringify(workflow.resource))
    expect('definition' in read && read.definition.url).toBe('https://health.go.ug/fhir/Library/anc-visit')
    expect(newDesign('code-system', 'Danger signs', 'https://h/fhir', '1.0.0', []).resource).toMatchObject({ resourceType: 'CodeSystem', id: 'danger-signs', name: 'DangerSigns', content: 'complete', concept: [] })
    expect(newDesign('value-set', '2 signs', 'https://h/fhir', '1.0.0', []).resource).toMatchObject({ resourceType: 'ValueSet', url: 'https://h/fhir/ValueSet/2-signs', name: 'X2Signs', compose: { include: [] } })
  })

  test('a new decision table goes into its workflow under an id of its own', () => {
    const text = JSON.stringify(opdVisit)
    const once = withNewTable(text, 'Triage risk')
    if ('refusal' in once) throw new Error(once.refusal)
    expect(once.table).toBe('triage-risk')
    const twice = withNewTable(once.content, 'Triage risk')
    if ('refusal' in twice) throw new Error(twice.refusal)
    expect(twice.table).toBe('triage-risk-2')
    const read = readDefinition(twice.content)
    expect('definition' in read && read.definition.decisionTables?.map((t) => t.id).slice(-2)).toEqual(['triage-risk', 'triage-risk-2'])
    expect(withNewTable('{"resourceType":"Questionnaire"}', 'x')).toEqual({ refusal: 'not-workflow' })
  })

  test('the canonical base comes from the manifest Library’s url', () => {
    const page = { files: [{ path: 'Library-manifest.json', resourceType: 'Library', url: 'https://health.go.ug/fhir/Library/ug.moh.opd-manifest', version: '1.0.0' }] }
    expect(canonicalOf(page as never)).toBe('https://health.go.ug/fhir')
    expect(canonicalOf({ files: [] } as never)).toBeNull()
  })
})

test('every section, kind and status is worded, with no ICU error', () => {
  const errors: string[] = []
  const intl = createIntl({ locale: 'en', onError: (e) => void errors.push(String(e)) })
  const texts = [
    ...sections.flatMap((s) => [dm.dhSection, dm.dhSectionHint, dm.dhOpen, dm.dhNew, dm.dhEmpty, dm.dhNewHeading].map((m) => intl.formatMessage(m, { section: icu(s.id) }))),
    ...sections.map((s) => intl.formatMessage(dm.dhCount, { section: icu(s.id), count: 2 })),
    ...(['form', 'workflow', 'decision-table', 'code-system', 'value-set', 'program', 'age-band-set', 'indicator', 'report', 'dashboard'] as const).map((k) => intl.formatMessage(dm.dhKind, { kind: icu(k) })),
    ...(['published', 'yours', 'theirs', 'submitted'] as const).map((st) => intl.formatMessage(dm.dhStatus, { status: st, holder: 'bob' })),
  ]
  expect(errors).toEqual([])
  expect(texts).toContain('Decision tables')
  expect(texts).toContain('2 code systems and value sets')
  expect(texts).toContain('Value set')
  expect(texts).toContain('In bob’s draft')
  expect(texts).toContain('2 reports')
  expect(texts).toContain('Report')
  expect(texts).toContain('2 dashboards')
  expect(texts).toContain('Dashboard')
})
