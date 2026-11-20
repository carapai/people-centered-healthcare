import { css, cx } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useId, useState } from 'react'
import { useIntl, type IntlShape } from 'react-intl'

import { decisionMessages as m } from '../../../i18n/messages'
import { usePlatform } from '../../../platform'
import { button, control } from '../../reception/styles'
import { StudioError, saveFile, type StudioPackagePage } from '../api'
import { refusalText } from '../wording'
import { checkTable, tryTable, type StudioDecisionTry } from './api'
import {
  JOINS,
  blank,
  combinations,
  compile,
  inputKinds,
  inputsOf,
  inputsRead,
  kindOf,
  literal,
  nodes,
  operatorsFor,
  readRules,
  samples,
  shown,
  testCase,
  testPath,
  updateNode,
  withTable,
  writeRules,
  type Group,
  type Join,
  type Literal,
  type LiteralKind,
  type Node,
  type Operator,
  type Rule,
} from './decisions'
import type { DataRequirement, DecisionTable, Value, WorkflowDefinition } from './model'

const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const alert = css({ color: 'danger.text' })
const good = css({ color: 'success.text' })
const panel = css({
  border: '1px solid',
  borderColor: 'border',
  borderRadius: 'md',
  bg: 'bg.surface',
  p: '3',
  display: 'flex',
  flexDirection: 'column',
  gap: '3',
  minWidth: '0',
})
const columns = css({
  display: 'grid',
  gap: '4',
  alignItems: 'start',
  gridTemplateColumns: { base: '1fr', xl: '18rem minmax(0, 1fr) 20rem' },
})
const row = css({
  display: 'flex',
  alignItems: 'center',
  gap: '2',
  flexWrap: 'wrap',
})
const label = css({
  display: 'flex',
  flexDirection: 'column',
  gap: '1',
  fontSize: 'sm',
})
const input = cx(control, css({ minWidth: '0' }))
const mono = css({
  fontFamily: 'mono',
  fontSize: 'xs',
  bg: 'bg.subtle',
  p: '2',
  borderRadius: 'sm',
  overflowWrap: 'anywhere',
})
const linkButton = css({
  textDecoration: 'underline',
  cursor: 'pointer',
  textAlign: 'left',
  overflowWrap: 'anywhere',
})
const small = css({
  minHeight: 'control',
  minWidth: 'control',
  px: '2',
  borderRadius: 'md',
  border: '1px solid',
  borderColor: 'border',
  bg: 'bg.surface',
  cursor: 'pointer',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
  _disabled: { opacity: '0.5', cursor: 'not-allowed' },
})
const ruleButton = css({
  flex: '1',
  textAlign: 'left',
  p: '2',
  borderRadius: 'sm',
  cursor: 'pointer',
  minWidth: '0',
  overflowWrap: 'anywhere',
  '&[aria-current=true]': { bg: 'accent.subtle', fontWeight: 'semibold' },
})

/** `value` once it has stayed the same for `ms`. */
function useSettled<T>(value: T, ms = 350): T {
  const [settled, setSettled] = useState(value)
  const text = JSON.stringify(value)
  useEffect(() => {
    const timer = setTimeout(() => setSettled(JSON.parse(text) as T), ms)
    return () => clearTimeout(timer)
  }, [text, ms])
  return settled
}

const failure = (intl: IntlShape, error: unknown) =>
  error instanceof StudioError ? (error.refusal === null || error.refusal === undefined ? error.detail || String(error.status) : refusalText(intl, error.refusal)) : String(error)

const pathText = (path: number[]) => (path.length === 0 ? '1' : ['1', ...path.map((n) => String(n + 1))].join('.'))

export interface DecisionTableEditorProps {
  /** The package version, `name@version`. */
  id: string
  definition: WorkflowDefinition
  /** The table's index in the definition. */
  index: number
  editable: boolean
  /** The draft's revision, for saving a test case. */
  revision: number
  /** The draft's files, so a test case gets a path of its own. */
  files: string[]
  onChange: (definition: WorkflowDefinition) => void
  /** A test case was saved: the page as it is now. */
  onSaved: (page: StudioPackagePage) => void
  onClose: () => void
}

/**
 * The decision-table editor (studio.md § Decision tables; #516): the checks above, the rules in
 * order on the left, the open rule's all / any / not tree in the middle, saved as one FHIRPath
 * expression, and *Try a patient* with the saved test cases on the right. The server evaluates
 * the draft table, unsaved edits included, on the engine submission's simulation uses; the
 * definition is saved with the Workflow Studio's own Save.
 */
export function DecisionTableEditor(props: DecisionTableEditorProps) {
  const { id, definition, index, editable, onChange, onClose } = props
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const table = (definition.decisionTables ?? [])[index] as DecisionTable
  const [rules, setRules] = useState<Rule[]>(() => readRules(table))
  const [open, setOpen] = useState<number | null>(rules.length > 0 ? 0 : null)
  const declared = inputsOf(definition)
  const kinds = inputKinds(rules, declared)
  const read = [...new Set(rules.flatMap((r) => inputsRead(r.tree)))].filter((i) => declared.includes(i))
  const [patient, setPatient] = useState<Record<string, Literal>>({})
  const values: Record<string, Literal> = Object.fromEntries(read.map((i) => [i, patient[i] !== undefined && kindOf(patient[i]) === kinds[i] ? patient[i] : blank(kinds[i] ?? 'code')]))
  const rule = open === null ? undefined : rules[open]

  const commit = (next: Rule[], patch: Partial<DecisionTable> = {}) => {
    setRules(next)
    onChange(
      withTable(definition, index, {
        ...table,
        ...patch,
        rows: writeRules(next),
      }),
    )
  }
  const setTable = (patch: Partial<DecisionTable>) => onChange(withTable(definition, index, { ...table, ...patch }))

  const sampled = samples(rules, declared)
  const check = useSettled({ definition, table: table.id, samples: sampled })
  const checks = useQuery({
    queryKey: ['studio', 'decision-check', id, check] as const,
    queryFn: ({ signal }) => checkTable(platform.transport, id, check, signal),
    enabled: rules.length > 0,
    retry: false,
  })
  const asked = rule === undefined ? [] : [...new Set(nodes(rule.tree).map(({ node }) => compile(node)))]
  const trial = useSettled({
    definition,
    table: table.id,
    inputs: values,
    nodes: asked,
  })
  const tried = useQuery({
    queryKey: ['studio', 'decision-try', id, trial] as const,
    queryFn: ({ signal }) => tryTable(platform.transport, id, trial, signal),
    enabled: rules.length > 0,
    retry: false,
  })
  const result = tried.data
  const never = new Set(checks.data?.neverFires ?? [])

  const [name, setName] = useState('')
  const saveTest = useMutation({
    mutationFn: async () => {
      const outcome = result?.outcome
      if (outcome === null || outcome === undefined) throw new Error('no outcome')
      const path = testPath(table.id, name, new Set(props.files))
      const page = await saveFile(platform.transport, id, path, JSON.stringify(testCase(definition, table.id, name.trim(), values, outcome), null, 2), props.revision)
      return { page, path }
    },
    onSuccess: async ({ page }) => {
      props.onSaved(page)
      setName('')
      await queryClient.invalidateQueries({
        queryKey: ['studio', 'decision-check', id],
      })
    },
  })

  const move = (n: number, by: -1 | 1) => {
    const next = [...rules]
    const a = next[n]
    const b = next[n + by]
    if (a === undefined || b === undefined) return
    next[n] = b
    next[n + by] = a
    commit(next)
    if (open === n) setOpen(n + by)
    else if (open === n + by) setOpen(n)
  }
  const outcomes = [...new Set([...rules.map((r) => shown(r.outcome)), shown(table.default)].filter((o) => o !== ''))]
  const addRule = () => {
    commit([
      ...rules,
      {
        name: intl.formatMessage(m.dtNewRule, { n: rules.length + 1 }),
        tree: { kind: 'group', join: 'all', children: [] },
        outcome: outcomes[0] ?? 'outcome',
      },
    ])
    setOpen(rules.length)
  }
  const setRule = (patch: Partial<Rule>) => open !== null && commit(rules.map((r, n) => (n === open ? { ...r, ...patch } : r)))
  const edit = (path: number[], f: (node: Node) => Node | null) => rule !== undefined && setRule({ tree: updateNode(rule.tree, path, f) })
  const marks = new Map((result?.nodes ?? []).map((n) => [n.expression, n.holds]))
  const ids = {
    tableId: useId(),
    dflt: useId(),
    outcomes: useId(),
    testName: useId(),
  }

  return (
    <section aria-labelledby="dt-heading" className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
      <div className={row}>
        <h2 id="dt-heading" className={subheading}>
          {intl.formatMessage(m.dtHeading, { id: table.id })}
        </h2>
        <button type="button" className={small} onClick={onClose}>
          {intl.formatMessage(m.dtBack)}
        </button>
      </div>
      <div className={row}>
        <label className={label} htmlFor={ids.tableId}>
          {intl.formatMessage(m.dtId)}
          <input id={ids.tableId} className={input} value={table.id} disabled={!editable} onChange={(e) => setTable({ id: e.target.value })} />
        </label>
        <label className={label} htmlFor={ids.dflt}>
          {intl.formatMessage(m.dtDefault)}
          <input
            id={ids.dflt}
            className={input}
            list={ids.outcomes}
            value={shown(table.default)}
            disabled={!editable}
            onChange={(e) =>
              setTable({
                default: e.target.value === '' ? null : e.target.value,
              })
            }
          />
        </label>
        <datalist id={ids.outcomes}>
          {outcomes.map((o) => (
            <option key={o} value={o} />
          ))}
        </datalist>
        <p className={muted}>{intl.formatMessage(m.dtDefaultHint)}</p>
      </div>

      <Checks rules={rules} sampled={sampled} checks={checks.data} loading={checks.isFetching} error={checks.error} />

      <Inputs definition={definition} editable={editable} onChange={onChange} />

      <div className={columns}>
        <nav aria-labelledby="dt-rules" className={panel}>
          <h3 id="dt-rules" className={subheading}>
            {intl.formatMessage(m.dtRules)}
          </h3>
          <p className={muted}>{intl.formatMessage(m.dtRulesHint)}</p>
          <ol
            className={css({
              display: 'flex',
              flexDirection: 'column',
              gap: '1',
            })}
          >
            {rules.map((r, n) => (
              <li
                key={n}
                className={css({
                  display: 'flex',
                  gap: '1',
                  alignItems: 'stretch',
                })}
              >
                <button type="button" aria-current={open === n ? 'true' : undefined} className={ruleButton} onClick={() => setOpen(n)}>
                  <span className={css({ display: 'block' })}>
                    {intl.formatMessage(m.dtRuleRow, {
                      n: n + 1,
                      name: r.name,
                    })}
                  </span>
                  <span className={cx(muted, css({ display: 'block' }))}>
                    {intl.formatMessage(m.dtThen, {
                      outcome: shown(r.outcome),
                    })}
                    {result?.row === n ? <span className={good}> · {intl.formatMessage(m.dtFires)}</span> : null}
                    {never.has(n) ? <span className={alert}> · {intl.formatMessage(m.dtNever)}</span> : null}
                  </span>
                </button>
                {editable ? (
                  <span
                    className={css({
                      display: 'flex',
                      flexDirection: 'column',
                      gap: '1',
                    })}
                  >
                    <button type="button" className={small} aria-label={intl.formatMessage(m.dtUp, { name: r.name })} disabled={n === 0} onClick={() => move(n, -1)}>
                      ↑
                    </button>
                    <button
                      type="button"
                      className={small}
                      aria-label={intl.formatMessage(m.dtDown, {
                        name: r.name,
                      })}
                      disabled={n === rules.length - 1}
                      onClick={() => move(n, 1)}
                    >
                      ↓
                    </button>
                  </span>
                ) : null}
              </li>
            ))}
          </ol>
          {editable ? (
            <button type="button" className={small} onClick={addRule}>
              {intl.formatMessage(m.dtAddRule)}
            </button>
          ) : null}
        </nav>

        <section aria-labelledby="dt-rule" className={panel}>
          {rule === undefined || open === null ? (
            <>
              <h3 id="dt-rule" className={subheading}>
                {intl.formatMessage(m.dtRuleNone)}
              </h3>
              <p className={muted}>{intl.formatMessage(m.dtChoose)}</p>
            </>
          ) : (
            <RuleDetail
              key={open}
              n={open}
              rule={rule}
              outcomes={ids.outcomes}
              declared={declared}
              marks={marks}
              editable={editable}
              onRule={setRule}
              onEdit={edit}
              onDelete={() => {
                commit(rules.filter((_, n) => n !== open))
                setOpen(null)
              }}
            />
          )}
        </section>

        <aside aria-labelledby="dt-try" className={panel}>
          <h3 id="dt-try" className={subheading}>
            {intl.formatMessage(m.dtTry)}
          </h3>
          <p className={muted}>{intl.formatMessage(m.dtTryHint)}</p>
          {read.length === 0 ? <p className={muted}>{intl.formatMessage(m.dtTryNothing)}</p> : null}
          {read.map((i) => (
            <SampleField key={i} name={i} kind={kinds[i] ?? 'code'} value={values[i] ?? ''} onChange={(v) => setPatient({ ...patient, [i]: v })} />
          ))}
          <TryResult rules={rules} result={result} loading={tried.isFetching} error={tried.error} onOpen={setOpen} />
          {editable && result !== undefined && result.outcome !== null && result.outcome !== undefined ? (
            <div
              className={css({
                display: 'flex',
                flexDirection: 'column',
                gap: '2',
              })}
            >
              <label className={label} htmlFor={ids.testName}>
                {intl.formatMessage(m.dtTestName)}
                <input id={ids.testName} className={input} value={name} onChange={(e) => setName(e.target.value)} />
              </label>
              <button type="button" className={button} disabled={name.trim() === '' || saveTest.isPending} onClick={() => saveTest.mutate()}>
                {intl.formatMessage(m.dtSaveTest)}
              </button>
            </div>
          ) : null}
          {saveTest.isSuccess ? <p role="status">{intl.formatMessage(m.dtTestSaved, { path: saveTest.data.path })}</p> : null}
          {saveTest.error === null ? null : (
            <p role="alert" className={alert}>
              {intl.formatMessage(m.dtTestFailed, {
                reason: failure(intl, saveTest.error),
              })}
            </p>
          )}
          <h3 className={subheading}>{intl.formatMessage(m.dtTestCases)}</h3>
          {(checks.data?.tests ?? []).length === 0 ? <p className={muted}>{intl.formatMessage(m.dtTestsNone)}</p> : null}
          <ul
            className={css({
              display: 'flex',
              flexDirection: 'column',
              gap: '1',
              fontSize: 'sm',
            })}
          >
            {(checks.data?.tests ?? []).map((t) => (
              <li key={t.path}>
                <button type="button" className={linkButton} onClick={() => setPatient(t.inputs)}>
                  <span className={t.passes ? good : alert}>{intl.formatMessage(t.passes ? m.dtPasses : m.dtFails)}</span>{' '}
                  {intl.formatMessage(m.dtTestCase, {
                    name: t.name,
                    expect: shown(t.expect),
                    outcome: t.outcome === null || t.outcome === undefined ? intl.formatMessage(m.dtNothing) : shown(t.outcome),
                  })}
                </button>
              </li>
            ))}
          </ul>
        </aside>
      </div>
    </section>
  )
}

/** A combination of sample values, in words. */
const example = (gap: Record<string, Literal>) =>
  Object.entries(gap)
    .map(([k, v]) => `${k} ${shown(v)}`)
    .join(', ')

interface ChecksProps {
  rules: Rule[]
  sampled: Record<string, Literal[]>
  checks: Awaited<ReturnType<typeof checkTable>> | undefined
  loading: boolean
  error: unknown
}

/** The checks above the editor: gaps, rules that never fire, failing test cases. */
function Checks({ rules, sampled, checks, loading, error }: ChecksProps) {
  const intl = useIntl()
  const failing = (checks?.tests ?? []).filter((t) => !t.passes).length
  return (
    <section aria-labelledby="dt-checks" className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
      <h3 id="dt-checks" className={subheading}>
        {intl.formatMessage(m.dtChecks)}
      </h3>
      <p className={muted}>{intl.formatMessage(m.dtChecksHint)}</p>
      {rules.length === 0 ? <p className={alert}>{intl.formatMessage(m.dtNoRules)}</p> : null}
      {loading && checks === undefined ? <p className={muted}>{intl.formatMessage(m.dtChecking)}</p> : null}
      {error === null || error === undefined ? null : (
        <p className={alert}>
          {intl.formatMessage(m.dtCheckFailed, {
            reason: failure(intl, error),
          })}
        </p>
      )}
      {checks === undefined || rules.length === 0 ? null : (
        <ul
          className={css({
            listStyle: 'disc',
            pl: '5',
            display: 'flex',
            flexDirection: 'column',
            gap: '1',
          })}
        >
          <li className={checks.covered && checks.gapCount === 0 ? good : alert}>
            {checks.covered
              ? intl.formatMessage(m.dtGaps, {
                  count: checks.gapCount,
                  example: checks.gaps[0] === undefined ? '' : example(checks.gaps[0]),
                })
              : intl.formatMessage(m.dtTooMany, {
                  count: combinations(sampled),
                  most: checks.mostCombinations,
                })}
          </li>
          {checks.covered ? (
            <li className={checks.neverFires.length === 0 ? good : alert}>
              {intl.formatMessage(m.dtNeverFiresCheck, {
                count: checks.neverFires.length,
                names: checks.neverFires.map((n) => `“${rules[n]?.name ?? n + 1}”`).join(', '),
              })}
            </li>
          ) : null}
          <li className={failing === 0 ? good : alert}>
            {intl.formatMessage(m.dtTestsResult, {
              count: checks.tests.length,
              failing,
            })}
          </li>
          {checks.errors.map((e) => (
            <li key={e} className={alert}>
              {intl.formatMessage(m.dtError, { error: e })}
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

/** The data requirements the rules read: each path one input, `%<name>.<path>`. */
function Inputs({ definition, editable, onChange }: { definition: WorkflowDefinition; editable: boolean; onChange: (d: WorkflowDefinition) => void }) {
  const intl = useIntl()
  const requirements = definition.dataRequirements ?? []
  const [name, setName] = useState('')
  const [type, setType] = useState('')
  const [paths, setPaths] = useState<Record<string, string>>({})
  const ids = { name: useId(), type: useId() }
  // Open at first when nothing is declared yet; then as the author leaves it.
  const [opened, setOpened] = useState(requirements.length === 0)
  const set = (next: DataRequirement[]) => onChange({ ...definition, dataRequirements: next })
  const taken = requirements.some((r) => r.name === name.trim())
  return (
    <details open={opened} onToggle={(e) => setOpened(e.currentTarget.open)} className={panel}>
      <summary className={css({ cursor: 'pointer', fontWeight: 'semibold' })}>{intl.formatMessage(m.dtInputs, { count: inputsOf(definition).length })}</summary>
      <p className={muted}>{intl.formatMessage(m.dtInputsHint)}</p>
      <ul className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        {requirements.map((r, n) => (
          <li
            key={r.name}
            className={css({
              display: 'flex',
              flexDirection: 'column',
              gap: '1',
            })}
          >
            <strong>
              {intl.formatMessage(m.dtRequirement, {
                name: r.name,
                resourceType: r.resourceType,
              })}
            </strong>
            <ul
              className={css({
                pl: '3',
                display: 'flex',
                flexDirection: 'column',
                gap: '1',
              })}
            >
              {(r.paths ?? []).map((p) => (
                <li key={p} className={row}>
                  <code className={css({ fontFamily: 'mono', fontSize: 'sm' })}>{p}</code>
                  {editable ? (
                    <button
                      type="button"
                      className={small}
                      onClick={() =>
                        set(
                          requirements.map((x, m2) =>
                            m2 === n
                              ? {
                                  ...x,
                                  paths: (x.paths ?? []).filter((q) => q !== p),
                                }
                              : x,
                          ),
                        )
                      }
                    >
                      {intl.formatMessage(m.dtRemovePath, { path: p })}
                    </button>
                  ) : null}
                </li>
              ))}
            </ul>
            {editable ? (
              <div className={row}>
                <input
                  className={input}
                  aria-label={intl.formatMessage(m.dtPath, { name: r.name })}
                  placeholder={`${r.resourceType}.`}
                  value={paths[r.name] ?? ''}
                  onChange={(e) => setPaths({ ...paths, [r.name]: e.target.value })}
                />
                <button
                  type="button"
                  className={small}
                  disabled={(paths[r.name] ?? '').trim() === ''}
                  onClick={() => {
                    set(
                      requirements.map((x, m2) =>
                        m2 === n
                          ? {
                              ...x,
                              paths: [...new Set([...(x.paths ?? []), (paths[r.name] ?? '').trim()])],
                            }
                          : x,
                      ),
                    )
                    setPaths({ ...paths, [r.name]: '' })
                  }}
                >
                  {intl.formatMessage(m.dtAddPath, { name: r.name })}
                </button>
              </div>
            ) : null}
          </li>
        ))}
      </ul>
      {editable ? (
        <div className={row}>
          <label className={label} htmlFor={ids.name}>
            {intl.formatMessage(m.dtRequirementName)}
            <input id={ids.name} className={input} value={name} onChange={(e) => setName(e.target.value)} />
          </label>
          <label className={label} htmlFor={ids.type}>
            {intl.formatMessage(m.dtResourceType)}
            <input id={ids.type} className={input} value={type} onChange={(e) => setType(e.target.value)} />
          </label>
          <button
            type="button"
            className={small}
            disabled={!/^[A-Za-z_]\w*$/.test(name.trim()) || type.trim() === '' || taken}
            onClick={() => {
              set([...requirements, { name: name.trim(), resourceType: type.trim(), paths: [] }])
              setName('')
              setType('')
            }}
          >
            {intl.formatMessage(m.dtAddRequirement)}
          </button>
        </div>
      ) : null}
    </details>
  )
}

interface RuleDetailProps {
  n: number
  rule: Rule
  outcomes: string
  declared: string[]
  marks: Map<string, boolean>
  editable: boolean
  onRule: (patch: Partial<Rule>) => void
  onEdit: (path: number[], f: (node: Node) => Node | null) => void
  onDelete: () => void
}

function RuleDetail({ n, rule, outcomes, declared, marks, editable, onRule, onEdit, onDelete }: RuleDetailProps) {
  const intl = useIntl()
  const ids = { name: useId(), outcome: useId() }
  return (
    <>
      <h3 id="dt-rule" className={subheading}>
        {intl.formatMessage(m.dtRule, { n: n + 1 })}
      </h3>
      <div className={row}>
        <label className={cx(label, css({ flex: '1', minWidth: '12rem' }))} htmlFor={ids.name}>
          {intl.formatMessage(m.dtRuleName)}
          <input id={ids.name} className={input} value={rule.name} disabled={!editable} onChange={(e) => onRule({ name: e.target.value })} />
        </label>
        <label className={label} htmlFor={ids.outcome}>
          {intl.formatMessage(m.dtOutcome)}
          <input id={ids.outcome} className={input} list={outcomes} value={shown(rule.outcome)} disabled={!editable} onChange={(e) => onRule({ outcome: e.target.value as Value })} />
        </label>
        {editable ? (
          <button type="button" className={small} onClick={onDelete}>
            {intl.formatMessage(m.dtDeleteRule)}
          </button>
        ) : null}
      </div>
      <NodeEditor node={rule.tree} path={[]} declared={declared} marks={marks} editable={editable} onEdit={onEdit} />
      <div>
        <p className={muted}>{intl.formatMessage(m.dtCompiled)}</p>
        <p className={mono}>{compile(rule.tree)}</p>
      </div>
    </>
  )
}

interface NodeEditorProps {
  node: Node
  path: number[]
  declared: string[]
  marks: Map<string, boolean>
  editable: boolean
  onEdit: (path: number[], f: (node: Node) => Node | null) => void
}

function Mark({ held }: { held: boolean | undefined }) {
  const intl = useIntl()
  if (held === undefined) return null
  return held ? (
    <span className={good} role="img" aria-label={intl.formatMessage(m.dtHolds)}>
      ✓
    </span>
  ) : (
    <span className={alert} role="img" aria-label={intl.formatMessage(m.dtHoldsNot)}>
      ✗
    </span>
  )
}

function NodeEditor({ node, path, declared, marks, editable, onEdit }: NodeEditorProps) {
  const intl = useIntl()
  const at = pathText(path)
  const held = marks.get(compile(node))
  if (node.kind === 'fhirpath')
    return (
      <div className={row}>
        <Mark held={held} />
        <input
          className={cx(input, css({ flex: '1', fontFamily: 'mono' }))}
          aria-label={intl.formatMessage(m.dtRaw, { at })}
          value={node.expression}
          disabled={!editable}
          onChange={(e) =>
            onEdit(path, () => ({
              kind: 'fhirpath',
              expression: e.target.value,
            }))
          }
        />
        <span className={muted}>{intl.formatMessage(m.dtRawHint)}</span>
        {editable && path.length > 0 ? (
          <button type="button" className={small} onClick={() => onEdit(path, () => null)}>
            {intl.formatMessage(m.dtRemoveCondition, { at })}
          </button>
        ) : null}
      </div>
    )
  if (node.kind === 'condition') {
    const kind = kindOf(node.value)
    const set = (patch: Partial<typeof node>) => onEdit(path, () => ({ ...node, ...patch }))
    const options = declared.includes(node.input) ? declared : [node.input, ...declared]
    return (
      <div className={row}>
        <Mark held={held} />
        <select className={input} aria-label={intl.formatMessage(m.dtInput, { at })} value={node.input} disabled={!editable} onChange={(e) => set({ input: e.target.value })}>
          {options.map((i) => (
            <option key={i} value={i}>
              {i}
            </option>
          ))}
        </select>
        <select className={input} aria-label={intl.formatMessage(m.dtOperator, { at })} value={node.op} disabled={!editable} onChange={(e) => set({ op: e.target.value as Operator })}>
          {operatorsFor(kind).map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </select>
        <select
          className={input}
          aria-label={intl.formatMessage(m.dtKind, { at })}
          value={kind}
          disabled={!editable}
          onChange={(e) => {
            const next = e.target.value as LiteralKind
            set({
              value: blank(next),
              op: operatorsFor(next).includes(node.op) ? node.op : '=',
            })
          }}
        >
          {(['boolean', 'number', 'code'] as const).map((k) => (
            <option key={k} value={k}>
              {intl.formatMessage(m.dtKindName, { kind: k })}
            </option>
          ))}
        </select>
        {kind === 'boolean' ? (
          <select className={input} aria-label={intl.formatMessage(m.dtValue, { at })} value={String(node.value)} disabled={!editable} onChange={(e) => set({ value: e.target.value === 'true' })}>
            <option value="true">true</option>
            <option value="false">false</option>
          </select>
        ) : (
          <input
            className={cx(input, css({ width: kind === 'number' ? '6rem' : '10rem' }))}
            aria-label={intl.formatMessage(m.dtValue, { at })}
            type={kind === 'number' ? 'number' : 'text'}
            step={kind === 'number' ? 1 : undefined}
            value={shown(node.value)}
            disabled={!editable}
            onChange={(e) => set({ value: literal(kind, e.target.value) })}
          />
        )}
        {editable ? (
          <button type="button" className={small} onClick={() => onEdit(path, () => null)}>
            {intl.formatMessage(m.dtRemoveCondition, { at })}
          </button>
        ) : null}
      </div>
    )
  }
  const group: Group = node
  const add = (child: Node) => onEdit(path, (n) => (n.kind === 'group' ? { ...n, children: [...n.children, child] } : n))
  const first = declared[0] ?? '%input.path'
  return (
    <div
      role="group"
      aria-label={intl.formatMessage(m.dtGroup, { at, join: group.join })}
      className={css({
        display: 'flex',
        flexDirection: 'column',
        gap: '2',
        borderLeft: '3px solid',
        borderColor: group.join === 'not' ? 'danger.text' : group.join === 'any' ? 'accent.default' : 'border',
        pl: '3',
        py: '1',
      })}
    >
      <div className={row}>
        <Mark held={held} />
        <select
          className={cx(input, css({ fontWeight: 'semibold' }))}
          aria-label={intl.formatMessage(m.dtJoin, { at })}
          value={group.join}
          disabled={!editable}
          onChange={(e) => onEdit(path, (n) => (n.kind === 'group' ? { ...n, join: e.target.value as Join } : n))}
        >
          {JOINS.map((j) => (
            <option key={j} value={j}>
              {intl.formatMessage(m.dtJoinName, { join: j })}
            </option>
          ))}
        </select>
        {editable ? (
          <>
            <button type="button" className={small} onClick={() => add({ kind: 'condition', input: first, op: '=', value: true })}>
              {intl.formatMessage(m.dtAddCondition, { at })}
            </button>
            <button type="button" className={small} onClick={() => add({ kind: 'group', join: 'all', children: [] })}>
              {intl.formatMessage(m.dtAddGroup, { at })}
            </button>
            {path.length > 0 ? (
              <button type="button" className={small} onClick={() => onEdit(path, () => null)}>
                {intl.formatMessage(m.dtRemoveGroup, { at })}
              </button>
            ) : null}
          </>
        ) : null}
      </div>
      {group.children.length === 0 ? <p className={muted}>{intl.formatMessage(m.dtEmptyGroup)}</p> : null}
      {group.children.map((child, n) => (
        <NodeEditor key={n} node={child} path={[...path, n]} declared={declared} marks={marks} editable={editable} onEdit={onEdit} />
      ))}
    </div>
  )
}

function SampleField({ name, kind, value, onChange }: { name: string; kind: LiteralKind; value: Literal; onChange: (value: Literal) => void }) {
  const id = useId()
  return (
    <label className={label} htmlFor={id}>
      {name}
      {kind === 'boolean' ? (
        <select id={id} className={input} value={String(value)} onChange={(e) => onChange(e.target.value === 'true')}>
          <option value="true">true</option>
          <option value="false">false</option>
        </select>
      ) : (
        <input
          id={id}
          className={input}
          type={kind === 'number' ? 'number' : 'text'}
          step={kind === 'number' ? 1 : undefined}
          value={shown(value)}
          onChange={(e) => onChange(literal(kind, e.target.value))}
        />
      )}
    </label>
  )
}

function TryResult({ rules, result, loading, error, onOpen }: { rules: Rule[]; result: StudioDecisionTry | undefined; loading: boolean; error: unknown; onOpen: (n: number) => void }) {
  const intl = useIntl()
  if (error !== null && error !== undefined)
    return (
      <p role="alert" className={alert}>
        {intl.formatMessage(m.dtTryFailed, { reason: failure(intl, error) })}
      </p>
    )
  if (result === undefined) return loading ? <p className={muted}>{intl.formatMessage(m.dtTrying)}</p> : null
  const outcome = result.outcome
  if (outcome === null || outcome === undefined)
    return (
      <p role="status" className={alert}>
        {intl.formatMessage(m.dtNoOutcome)}
      </p>
    )
  const fired = result.row === null || result.row === undefined ? undefined : result.row
  return (
    <p role="status" className={good}>
      {fired === undefined ? (
        intl.formatMessage(m.dtOutcomeDefault, { outcome: shown(outcome) })
      ) : (
        <>
          {intl.formatMessage(m.dtOutcomeFrom, { outcome: shown(outcome) })}{' '}
          <button type="button" className={linkButton} onClick={() => onOpen(fired)}>
            “{rules[fired]?.name ?? fired + 1}”
          </button>
        </>
      )}
    </p>
  )
}
