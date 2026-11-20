import { describe, expect, test } from 'vitest'

import {
  addTable,
  compile,
  inputKinds,
  inputsOf,
  parse,
  readRules,
  requirementInputs,
  samples,
  testCase,
  testPath,
  updateNode,
  withTable,
  writeRules,
  type Group,
  type Node,
  type Rule,
} from '../src/features/studio/workflows/decisions'
import type { DecisionTable, WorkflowDefinition } from '../src/features/studio/workflows/model'

// The decision-table editor's model (#516): a rule's all / any / not tree saved as one FHIRPath
// expression in the engine's row, and parsed back when the table opens.

const condition = (input: string, op: '=' | '<', value: boolean | number | string): Node => ({ kind: 'condition', input, op, value })
const group = (join: 'all' | 'any' | 'not', ...children: Node[]): Group => ({
  kind: 'group',
  join,
  children,
})

/** any(all(age < 5, danger signs), all(pregnant, urgent)). */
const sickChildOrUrgentPregnancy = group(
  'any',
  group('all', condition('%patient.age', '<', 5), condition('%triage.danger', '=', true)),
  group('all', condition('%patient.pregnant', '=', true), condition('%triage.category', '=', 'urgent')),
)

/** The text the server's test evaluates (health-publication `decisions/tests.rs`). */
const SICK_CHILD_OR_URGENT_PREGNANCY = "(%patient.age < 5 and %triage.danger = true) or (%patient.pregnant = true and %triage.category = 'urgent')"

const definition: WorkflowDefinition = {
  url: 'https://health.go.ug/fhir/Library/opd-visit',
  version: '1.0.0',
  statechart: { states: [{ id: 'done', kind: 'final' }] },
  dataRequirements: [
    {
      name: 'patient',
      resourceType: 'Patient',
      paths: ['Patient.age', 'Patient.pregnant'],
    },
    {
      name: 'triage',
      resourceType: 'Observation',
      paths: ['Observation.category', 'danger', 'Observation', 'Observation.code.coding.where(system = 1)'],
    },
  ],
}

describe('the decision-table editor', () => {
  test('a rule built as any(all(age < 5, danger signs), all(pregnant, urgent)) saves as one FHIRPath expression and reads back as the same tree', () => {
    expect(compile(sickChildOrUrgentPregnancy)).toBe(SICK_CHILD_OR_URGENT_PREGNANCY)
    expect(parse(SICK_CHILD_OR_URGENT_PREGNANCY)).toEqual(sickChildOrUrgentPregnancy)
    const rules: Rule[] = [
      {
        name: 'Sick child or urgent pregnancy',
        tree: sickChildOrUrgentPregnancy,
        outcome: 'bypass',
      },
    ]
    const rows = writeRules(rules)
    expect(rows).toEqual([
      {
        name: 'Sick child or urgent pregnancy',
        when: [SICK_CHILD_OR_URGENT_PREGNANCY],
        // oxlint-disable-next-line unicorn/no-thenable -- the engine's name for a row's outcome
        then: 'bypass',
      },
    ])
    expect(readRules({ id: 'route', rows })).toEqual(rules)
  })

  test('not, nested groups, codes with quotes and empty groups round-trip', () => {
    const trees: Group[] = [
      group('all', condition('%triage.category', '=', 'urgent'), group('not', condition('%patient.pregnant', '=', true))),
      group('not', condition('%patient.age', '<', 1), condition('%triage.category', '=', "o'brien\\x")),
      group(
        'all',
        condition('%patient.age', '<', -2),
        group('any', condition('%triage.danger', '=', false), group('all', condition('%patient.age', '<', 3), condition('%patient.pregnant', '=', true))),
      ),
      group('not', group('all', condition('%patient.age', '<', 1), condition('%triage.danger', '=', true))),
      group('all'),
    ]
    for (const tree of trees) expect(parse(compile(tree)), compile(tree)).toEqual(tree)
    expect(compile(trees[0] as Group)).toBe("%triage.category = 'urgent' and (%patient.pregnant = true).not()")
    expect(compile(trees[4] as Group)).toBe('true')
    // A rule of one condition is an all group of it.
    expect(parse("%triage.category = 'routine'")).toEqual(group('all', condition('%triage.category', '=', 'routine')))
  })

  test('an expression the editor did not write is kept as FHIRPath text', () => {
    for (const text of ["%encounter.priority.coding.where(code = 'urgent').exists()", '%a.b < ', "%a.b < 'x'", 'today() > %patient.birthDate'])
      expect(parse(text)).toEqual(group('all', { kind: 'fhirpath', expression: text }))
    const kept = group('all', condition('%a.b', '=', true), {
      kind: 'fhirpath',
      expression: 'x or y',
    })
    expect(compile(kept)).toBe('%a.b = true and (x or y)')
  })

  test('older rows of several conditions read as one all group, and nameless rows get a name', () => {
    const table: DecisionTable = {
      id: 'route',
      rows: [
        {
          when: ["%triage.category = 'routine'", '%patient.age < 1'],
          // oxlint-disable-next-line unicorn/no-thenable -- the engine's name for a row's outcome
          then: 'bypass',
        },
      ],
      default: 'normal',
    }
    expect(readRules(table)).toEqual([
      {
        name: 'Rule 1',
        tree: group('all', condition('%triage.category', '=', 'routine'), condition('%patient.age', '<', 1)),
        outcome: 'bypass',
      },
    ])
  })

  test('inputs are the declared paths as FHIRPath reads them', () => {
    expect(
      requirementInputs({
        name: 'triage',
        resourceType: 'Observation',
        paths: ['Observation.category', 'danger', 'Observation', 'x.y()'],
      }),
    ).toEqual(['%triage.category', '%triage.danger'])
    expect(inputsOf(definition)).toEqual(['%patient.age', '%patient.pregnant', '%triage.category', '%triage.danger'])
  })

  test('coverage samples both flags, each number with its neighbours and each code', () => {
    const rules: Rule[] = [
      { name: 'a', tree: sickChildOrUrgentPregnancy, outcome: 'bypass' },
      {
        name: 'b',
        tree: group('all', condition('%triage.category', '=', 'routine'), condition('%patient.age', '<', 1), condition('%visit.kind', '=', 'new')),
        outcome: 'normal',
      },
    ]
    const inputs = inputsOf(definition)
    expect(samples(rules, inputs)).toEqual({
      '%patient.age': [0, 1, 2, 4, 5, 6],
      '%triage.danger': [true, false],
      '%patient.pregnant': [true, false],
      '%triage.category': ['urgent', 'routine'],
    })
    expect(inputKinds(rules, inputs)).toEqual({
      '%patient.age': 'number',
      '%triage.danger': 'boolean',
      '%patient.pregnant': 'boolean',
      '%triage.category': 'code',
      '%visit.kind': 'code',
    })
  })

  test('trees are edited by path, and tables are added and replaced', () => {
    const tree = group('all', condition('%a.b', '=', true), group('any'))
    const added = updateNode(tree, [1], (n) => (n.kind === 'group' ? { ...n, children: [...n.children, condition('%a.c', '<', 3)] } : n))
    expect(compile(added)).toBe('%a.b = true and %a.c < 3')
    expect(updateNode(added, [0], () => null)).toEqual(group('all', group('any', condition('%a.c', '<', 3))))
    const first = addTable(definition)
    expect(first.index).toBe(0)
    const second = addTable(first.definition)
    expect(second.definition.decisionTables?.map((t) => t.id)).toEqual(['table', 'table-2'])
    const replaced = withTable(second.definition, 1, {
      id: 'route',
      rows: [],
      default: null,
    })
    expect(replaced.decisionTables?.map((t) => t.id)).toEqual(['table', 'route'])
  })

  test('a saved test case names the definition and the table, under tests/', () => {
    expect(testCase(definition, 'route', 'Sick child', { '%patient.age': 3 }, 'bypass')).toEqual({
      type: 'decision-table-test',
      name: 'Sick child',
      definition: 'https://health.go.ug/fhir/Library/opd-visit|1.0.0',
      table: 'route',
      inputs: { '%patient.age': 3 },
      expect: 'bypass',
    })
    expect(testPath('route', 'Sick child, age 3!', new Set())).toBe('tests/route-sick-child-age-3.json')
    expect(testPath('route', 'x', new Set(['tests/route-x.json']))).toBe('tests/route-x-2.json')
  })
})
