import type { StudioReportRow, StudioReportSection } from '@health/api-client'
import { describe, expect, test } from 'vitest'

import monitoring from '../../../packages/reference-packages/hmis/configuration/reports/anc-facility-monitoring.json'
import hmis105 from '../../../packages/reference-packages/hmis/configuration/reports/hmis-105-anc.json'
import { newDesign } from '../src/features/studio/designs'
import { countedAt, layoutTable, percentChange, sentCells } from '../src/features/studio/reports/layout'
import {
  addAgeBands,
  addAttribute,
  addConcept,
  addLineList,
  addRow,
  addTable,
  addVisitType,
  newReport,
  readReport,
  toggleLevel,
  updateSection,
  withList,
  withOptions,
  withRowPer,
  withSettings,
  writeReport,
  type Report,
  type TableSection,
} from '../src/features/studio/reports/report'

// The Report designer (#559): a Report built step by step as the designer builds it is the same
// definition as the reference package's, read and written back unchanged, and laid out from the
// preview central computes.

const BANDS = 'https://health.go.ug/fhir/Library/age-bands-anc|1.0.0'
const M = 'https://health.go.ug/fhir/Measure/'
const ANC = 'https://health.go.ug/fhir/PlanDefinition/anc'
const GA = { system: 'http://loinc.org', code: '18185-9' }

/** The designer's steps on the last section added. */
function last<K extends 'table' | 'lineList'>(report: Report, kind: K, change: Parameters<typeof updateSection<K>>[3]): Report {
  return updateSection(report, report.sections.length - 1, kind, change)
}

describe('building the sample Reports in the designer', () => {
  test('the HMIS 105 ANC Report is the same definition as the fixture', () => {
    let report = newReport('hmis-105-anc', 'HMIS 105 · 2 Antenatal care')
    for (const level of ['district', 'region', 'national'] as const) report = toggleLevel(report, level, true)
    report = addTable(report, '2.1 ANC attendance')
    report = last(report, 'table', (s) => {
      let t: TableSection = { ...addAgeBands(s, BANDS), id: 'attendance' }
      t = addRow(t, `${M}anc-first-visits|1.0.0`, 'ANC 1st visit for women', 'first-visits')
      t = addRow(t, `${M}anc-visits|1.0.0`, 'ANC visits (all)', 'visits')
      return withOptions(t, { rowTotals: true, change: true, sendToDhis2: true })
    })
    report = addTable(report, '2.2 Timing of the 1st visit')
    report = last(report, 'table', (s) => {
      let t: TableSection = { ...s, id: 'timing' }
      t = addRow(t, `${M}anc-first-visit-before-12-weeks|1.0.0`, 'ANC 1st visit before 12 weeks (%)', 'before-12-weeks')
      t = addRow(t, `${M}anc-first-visit-next-visit-booked|1.0.0`, 'ANC 1st visits with the next visit booked (%)', 'next-visit-booked')
      return withOptions(t, { change: true })
    })
    expect(writeReport(report)).toEqual(hmis105)
    // In the schema's key order, as the file is written.
    expect(JSON.stringify(writeReport(report))).toBe(JSON.stringify(hmis105))
  })

  test('the facility ANC monitoring Report, with its line lists, is the same definition as the fixture', () => {
    let report = newReport('anc-facility-monitoring', 'ANC facility monitoring')
    report = addTable(report, 'Gestational age at the 1st visit')
    report = last(report, 'table', (s) => withOptions(addRow({ ...s, id: 'gestational-age' }, `${M}anc-gestational-age-at-first-visit|1.0.0`, 'Median gestational age at the 1st visit (weeks)', 'median-ga'), { change: true }))
    report = addLineList(report, 'ANC clients this month')
    report = last(report, 'lineList', (s) => {
      let l = { ...withList(s, { program: ANC, enrollments: 'active' }), id: 'clients' }
      l = addAttribute(l, 'person.name', 'Name')
      l = addAttribute(l, 'person.age', 'Age (years)')
      l = addAttribute(l, 'person.phone', 'Phone')
      l = addAttribute(l, 'enrollment.start', 'Enrolled on', 'enrolled')
      l = addAttribute(l, 'enrollment.lastVisit', 'Last visit', 'last-visit')
      return addConcept(l, GA, 'Gestational age (weeks)', 'gestational-age')
    })
    report = addLineList(report, '1st visits counted')
    report = last(report, 'lineList', (s) => {
      let l = withRowPer({ ...s, id: 'first-visits' }, 'visit')
      l = withList(addVisitType(l, { system: 'https://health.go.ug/fhir/CodeSystem/anc-visit-type', code: 'first' }), { program: ANC, countedBy: `${M}anc-first-visits|1.0.0` })
      l = addAttribute(l, 'person.name', 'Name')
      l = addAttribute(l, 'visit.date', 'Visit date')
      return addConcept(l, GA, 'Gestational age (weeks)', 'gestational-age')
    })
    expect(JSON.stringify(writeReport(report))).toBe(JSON.stringify(monitoring))
  })

  test('each fixture reads and writes back unchanged', () => {
    for (const fixture of [hmis105, monitoring]) {
      const read = readReport(JSON.stringify(fixture))
      if ('refusal' in read) throw new Error(read.refusal)
      expect(JSON.stringify(writeReport(read.report))).toBe(JSON.stringify(fixture))
    }
    expect(readReport('{"id":"x"}')).toEqual({ refusal: 'not written to the report/1 schema' })
  })

  test('switching a line list to visits drops the enrollment choice and visit-only columns go with enrollments', () => {
    let report = addLineList(newReport('r', 'R'), 'Clients')
    report = last(report, 'lineList', (s) => addConcept(addAttribute(withRowPer(s, 'visit'), 'visit.date', 'Date'), GA, 'GA'))
    const visit = report.sections[0]
    expect(visit).toMatchObject({ rowPer: 'visit', visitTypes: [] })
    expect(visit?.kind === 'lineList' && visit.columns[1]?.pick).toBeUndefined()
    report = last(report, 'lineList', (s) => withRowPer(s, 'enrollment'))
    expect(report.sections[0]).toMatchObject({ rowPer: 'enrollment', enrollments: 'active', columns: [{ label: 'GA', pick: 'latest' }] })
  })

  test('New report writes an empty Report at configuration/reports/<id>.json', () => {
    const made = newDesign('report', 'HMIS 105 ANC', 'https://health.go.ug/fhir', '1.1.0-draft', ['configuration/reports/hmis-105-anc.json'])
    expect(made.path).toBe('configuration/reports/hmis-105-anc-2.json')
    expect(made.resource).toEqual({ $schema: hmis105.$schema, id: 'hmis-105-anc-2', title: 'HMIS 105 ANC', periodType: 'monthly', levels: ['facility'], sections: [] })
    expect(withSettings(newReport('a', 'A'), { periodType: 'quarterly' }).periodType).toBe('quarterly')
  })
})

// A preview as central gives it: ANC 1st visits by two age bands, ANC visits not broken down.
const band = (code: string, label: string, count: number) => ({ code, label, count, value: { toString: () => String(count) } })
const figures = (count: number, bands: ReturnType<typeof band>[] | null) => ({
  count,
  value: { toString: () => String(count) },
  disaggregations: bands === null ? [] : [{ name: BANDS, title: 'ANC age bands', bands }],
})
const previewRows: StudioReportRow[] = [
  {
    row: 'first-visits',
    indicator: { title: 'ANC 1st visits', calculation: 'count', package: 'ug.moh.anc@1.0.0', published: false },
    current: figures(4, [band('15-19', '15–19 years', 1), band('20-24', '20–24 years', 3), band('not-recorded', 'Not recorded', 0)]),
    previous: figures(2, [band('15-19', '15–19 years', 2)]),
    counted: [
      { name: 'Akello', bands: [['15-19']] },
      { name: 'Nansubuga', bands: [['20-24']] },
    ],
  },
  { row: 'visits', indicator: { title: 'ANC visits', calculation: 'median', package: 'ug.moh.anc@1.0.0', published: true }, current: figures(9, null), previous: null, counted: [] },
] as unknown as StudioReportRow[]
const preview = {
  disaggregations: [
    {
      name: BANDS,
      title: 'ANC age bands',
      bands: [
        { code: '15-19', label: '15–19 years' },
        { code: '20-24', label: '20–24 years' },
        { code: 'not-recorded', label: 'Not recorded' },
      ],
    },
  ],
  rows: previewRows,
} as StudioReportSection
const attendance = (hmis105.sections[0] as unknown as TableSection)

describe('laying a table section out from the preview', () => {
  test('column bands, a Total column and % change; the Total is the indicator’s whole', () => {
    const layout = layoutTable(attendance, preview, 'facility')
    expect(layout.columns.map((c) => (c.total ? 'Total' : c.bands.map((b) => b.label).join()))).toEqual(['15–19 years', '20–24 years', 'Not recorded', 'Total'])
    const first = layout.rows[0]!
    expect(first.cells.map((c) => c.figure?.value)).toEqual(['1', '3', '0', '4'])
    expect(first.change).toEqual({ change: '100.0' })
    // ANC visits lists no age bands: its band cells say so, and its Total is its whole.
    const visits = layout.rows[1]!
    expect(visits.cells.slice(0, 3).map((c) => [c.notShown, c.missing])).toEqual([
      ['not-broken-down', BANDS],
      ['not-broken-down', BANDS],
      ['not-broken-down', BANDS],
    ])
    expect(visits.cells[3]?.figure?.value).toBe('9')
  })

  test('a median shows n/a above facility level', () => {
    const layout = layoutTable(attendance, preview, 'district')
    expect(layout.rows[1]?.cells.every((c) => c.notShown === 'facility-only')).toBe(true)
    expect(layout.rows[0]?.cells[3]?.figure?.value).toBe('4')
  })

  test('bands on rows give a row per band and, with column totals, a Total row', () => {
    const onRows: TableSection = { ...attendance, disaggregations: [{ ageBandSet: BANDS, axis: 'rows' }], rowTotals: false, columnTotals: true }
    const layout = layoutTable(onRows, preview, 'facility')
    expect(layout.rows.filter((r) => r.row.id === 'first-visits').map((r) => [r.total, r.bands.map((b) => b.code).join(), r.cells[0]?.figure?.value])).toEqual([
      [false, '15-19', '1'],
      [false, '20-24', '3'],
      [false, 'not-recorded', '0'],
      [true, '', '4'],
    ])
    const crossed: TableSection = { ...attendance, disaggregations: [{ ageBandSet: BANDS, axis: 'rows' }, { code: 'hiv', expression: 'x', axis: 'columns' }] }
    const hiv = { ...preview, disaggregations: [...preview.disaggregations, { name: 'hiv', title: 'hiv', bands: [{ code: 'positive', label: 'positive' }] }] }
    expect(layoutTable(crossed, hiv, 'facility').rows[0]?.cells[0]?.notShown).toBe('not-broken-down')
    const both = { ...hiv, rows: [{ ...previewRows[0]!, current: { ...previewRows[0]!.current!, disaggregations: [...previewRows[0]!.current!.disaggregations, { name: 'hiv', title: 'hiv', bands: [] }] } }] } as StudioReportSection
    expect(layoutTable(crossed, both, 'facility').rows[0]?.cells[0]?.notShown).toBe('crossed')
  })

  test('a cell crossing two Disaggregations shows its crossed figure, and the Total row still adds up', () => {
    // Age bands on rows by HIV status on columns, with column and row totals.
    const crossed: TableSection = { ...attendance, disaggregations: [{ ageBandSet: BANDS, axis: 'rows' }, { code: 'hiv', expression: 'x', axis: 'columns' }], columnTotals: true, rowTotals: true, change: false }
    const at = (age: string, hiv: string) => [
      { name: BANDS, code: age },
      { name: 'hiv', code: hiv },
    ]
    const cell = (age: string, hiv: string, count: number) => ({ at: at(age, hiv), count, value: { toString: () => String(count) } })
    const hivBands = [band('positive', 'Positive', 1), band('negative', 'Negative', 3), band('not-recorded', 'Not recorded', 0)]
    const current = {
      ...figures(4, [band('15-19', '15–19 years', 1), band('20-24', '20–24 years', 3), band('not-recorded', 'Not recorded', 0)]),
      // Every age band by every HIV status, as central counts them.
      crossed: ['15-19', '20-24', 'not-recorded'].flatMap((age) => ['positive', 'negative', 'not-recorded'].map((hiv) => cell(age, hiv, age === '15-19' && hiv === 'positive' ? 1 : age === '20-24' && hiv === 'negative' ? 3 : 0))),
    }
    current.disaggregations = [...current.disaggregations, { name: 'hiv', title: 'hiv', bands: hivBands }]
    const section = {
      disaggregations: [...preview.disaggregations, { name: 'hiv', title: 'hiv', bands: hivBands.map(({ code, label }) => ({ code, label })) }],
      rows: [{ ...previewRows[0]!, current, previous: null }],
    } as unknown as StudioReportSection
    const layout = layoutTable(crossed, section, 'facility')
    const lines = layout.rows.filter((r) => r.row.id === 'first-visits')
    const values = lines.map((r) => r.cells.map((c) => c.figure?.value ?? c.notShown))
    expect(values).toEqual([
      // positive, negative, not recorded, Total
      ['1', '0', '0', '1'],
      ['0', '3', '0', '3'],
      ['0', '0', '0', '0'],
      ['1', '3', '0', '4'],
    ])
    // Down each column the crossed cells add up to its Total.
    const body = values.slice(0, -1)
    expect([0, 1, 2].map((n) => body.reduce((sum, line) => sum + Number(line[n]), 0))).toEqual([1, 3, 0])
    // Who a crossed cell counted: those with a counted record in both bands.
    const counted = { ...section.rows[0]!, counted: [{ name: 'Akello', bands: [['15-19', 'positive']] }, { name: 'Nansubuga', bands: [['20-24', 'negative']] }] } as StudioReportRow
    expect(countedAt(counted, lines[1]!.cells[1]!.at)).toEqual(['Nansubuga'])
  })

  test('a cell names who it counted, and the DHIS2 sheet lists every cell of a sent section', () => {
    expect(countedAt(previewRows[0], [{ name: BANDS, code: '20-24', label: '20–24 years' }])).toEqual(['Nansubuga'])
    expect(countedAt(previewRows[0], [])).toEqual(['Akello', 'Nansubuga'])
    expect(percentChange({ value: '3', count: 3, denominator: null }, { value: '0', count: 0, denominator: null })).toBeNull()
    const sheet = sentCells([{ section: attendance, index: 0, preview }])
    expect(sheet).toHaveLength(8)
    expect(sheet.filter((c) => c.total || c.notRecorded)).toHaveLength(4)
  })
})
