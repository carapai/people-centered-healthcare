import { css, cx } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link, useNavigate } from '@tanstack/react-router'
import { useIntl } from 'react-intl'

import { programMessages as m, studioMessages as sm } from '../../../i18n/messages'
import { usePlatform } from '../../../platform'
import { button } from '../../reception/styles'
import { StudioError, fileQuery, packageQuery, saveFile, studioKey, type StudioPackagePage } from '../api'
import { useWorkflowFiles } from '../workflowFiles'
import { refusalText } from '../wording'
import { readDefinition } from '../workflows/model'
import {
  MANIFEST,
  OVERRIDE_POINTS,
  board,
  dependsOn,
  editorOf,
  gaps,
  moveRule,
  newArtefact,
  overridePoints,
  panel,
  programOf,
  readPlan,
  resolve,
  type Column,
  type Exit,
  type Gap,
  type Happening,
  type Holdings,
  type Linked,
  type Plan,
  type Program,
} from './model'

const page = css({ display: 'flex', flexDirection: 'column', gap: '4' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const small = css({ fontSize: 'sm', fontWeight: 'semibold', color: 'fg.muted' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })
const alert = css({ color: 'danger.text' })
const mono = css({ fontFamily: 'mono', fontSize: 'xs', overflowWrap: 'anywhere' })
const box = css({ border: '1px solid', borderColor: 'border', borderRadius: 'md', bg: 'bg.surface', p: '3', minWidth: '0' })
const list = css({ display: 'flex', flexDirection: 'column', gap: '1' })
const bullets = css({ display: 'flex', flexDirection: 'column', gap: '1', listStyle: 'disc', pl: '5' })
const ghost = css({
  minHeight: 'control',
  minWidth: 'control',
  px: '2',
  borderRadius: 'md',
  border: '1px solid',
  borderColor: 'border',
  bg: 'bg.surface',
  cursor: 'pointer',
  fontSize: 'sm',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
  _disabled: { opacity: '0.5', cursor: 'not-allowed' },
})
const tone = { draft: css({ color: 'accent.text' }), published: css({ color: 'success.text' }), missing: css({ color: 'danger.text' }) }

/**
 * The Program Designer (program-designer.md § The Program Designer; #500): a program
 * package's ProgramPlanDefinition assembled, not edited. A strip across the top holds what
 * applies to the whole program; the journey board has one column per state of its care
 * workflow, each with its form, what happens there, its investigations, its regimen templates,
 * its schedule, its outcomes and indicators and when a patient leaves; the package panel beside
 * it lists the files by kind with the missing ones marked, and what is still missing. Every
 * linked artefact says where it stands and opens in its editor, or, when missing, *Create* adds
 * it to the draft and opens it. Reordering regimen rules saves the plan through the file
 * editor's path, with the draft's revision as `If-Match` (#494).
 */
export function ProgramDesigner({ id, path }: { id: string; path: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const draft = useQuery(packageQuery(platform.transport, id))
  const file = useQuery({ ...fileQuery(platform.transport, id, path), retry: false })
  return (
    <section className={page}>
      <div className={css({ display: 'flex', gap: '4', flexWrap: 'wrap' })}>
        <Link to="/studio/$id" params={{ id }}>
          {intl.formatMessage(sm.sdEditorBack)}
        </Link>
        <Link to="/studio/$id/files/$" params={{ id, _splat: path }}>
          {intl.formatMessage(sm.fdAsJson)}
        </Link>
      </div>
      {draft.data === undefined || file.data === undefined ? (
        draft.isError || file.isError ? (
          <p role="alert" className={alert}>
            {intl.formatMessage(sm.sdFailed)}
          </p>
        ) : (
          <p role="status">{intl.formatMessage(m.pdLoading)}</p>
        )
      ) : (
        <Loaded id={id} path={path} draft={draft.data} content={file.data.content} onReload={() => void Promise.all([draft.refetch(), file.refetch()])} />
      )}
    </section>
  )
}

interface LoadedProps {
  id: string
  path: string
  draft: StudioPackagePage
  content: string
  onReload: () => void
}

function Loaded(props: LoadedProps) {
  const intl = useIntl()
  const read = readPlan(props.content)
  if ('refusal' in read)
    return (
      <p role="alert" className={alert}>
        {intl.formatMessage(m.pdRefused, { refusal: read.refusal === 'not-json' ? 'not_json' : 'other' })}
      </p>
    )
  return <Designer {...props} plan={read.plan} />
}

/** The text of one of the draft's files, if it has it. */
function useText(id: string, draft: StudioPackagePage, path: string | null): { text: string | undefined; settled: boolean } {
  const platform = usePlatform()
  const held = path !== null && draft.files.some((f) => f.path === path)
  const read = useQuery({ ...fileQuery(platform.transport, id, path ?? ''), enabled: held, retry: false })
  return { text: read.data?.content, settled: !held || read.data !== undefined || read.isError }
}

/** What a Create or Open needs: the package, whether the person may edit it and why not, and
 * the Workflow Definitions it holds. */
interface Context {
  id: string
  draft: StudioPackagePage
  workflows: Set<string>
  create: (linked: Linked) => void
  creating: boolean
}

function Designer({ id, path, draft, plan, onReload }: LoadedProps & { plan: Plan }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const workflows = useWorkflowFiles(id, draft)
  const program = programOf(plan)
  const manifest = useText(id, draft, MANIFEST)
  const overrides = useText(id, draft, OVERRIDE_POINTS)
  const held: Holdings = { files: draft.files, changes: draft.changes, hasPrevious: draft.package.previous !== null && draft.package.previous !== undefined, dependsOn: dependsOn(manifest.text) }
  const careCanonical = program.workflows[0]
  const care = careCanonical === undefined ? null : resolve(careCanonical, 'Library', held)
  const careFile = useText(id, draft, care?.path ?? null)
  const careRead = careFile.text === undefined ? null : readDefinition(careFile.text)
  const definition = careRead !== null && 'definition' in careRead ? careRead.definition : null
  const columns = definition === null ? null : board(program, definition, held)
  const found = gaps(program, care, columns, held)
  const why = (error: unknown) => (error instanceof StudioError ? (error.refusal === null || error.refusal === undefined ? error.detail : refusalText(intl, error.refusal)) : '')

  const settle = async (next: StudioPackagePage) => {
    queryClient.setQueryData(packageQuery(platform.transport, id).queryKey, next)
    await queryClient.invalidateQueries({ queryKey: studioKey, exact: true })
  }
  const create = useMutation({
    mutationFn: async (linked: Linked) => {
      const made = newArtefact(
        linked,
        draft.package.version,
        draft.files.map((f) => f.path),
      )
      const next = await saveFile(platform.transport, id, made.path, JSON.stringify(made.resource, null, 2), draft.revision)
      return { next, path: made.path, kind: linked.kind }
    },
    onSuccess: async ({ next, path: made, kind }) => {
      await settle(next)
      const editor = kind === 'Questionnaire' ? '/studio/$id/forms/$' : kind === 'Library' ? '/studio/$id/workflows/$' : '/studio/$id/files/$'
      await navigate({ to: editor, params: { id, _splat: made } })
    },
  })
  const reorder = useMutation({
    mutationFn: async (next: Plan) => {
      const text = JSON.stringify(next, null, 2)
      return { page: await saveFile(platform.transport, id, path, text, draft.revision), text }
    },
    onSuccess: async ({ page: next, text }) => {
      queryClient.setQueryData(fileQuery(platform.transport, id, path).queryKey, { path, content: text, revision: next.revision })
      await settle(next)
    },
  })
  const stale = [create.error, reorder.error].some((e) => e instanceof StudioError && e.status === 412)
  const context: Context = { id, draft, workflows, create: (linked) => create.mutate(linked), creating: create.isPending }
  const move = (stage: string, index: number, by: -1 | 1) => reorder.mutate(moveRule(plan, stage, index, by))

  return (
    <>
      <header>
        <h1 className={heading}>{intl.formatMessage(m.pdHeading, { title: program.title })}</h1>
        <p className={mono}>{intl.formatMessage(m.pdFile, { path, url: program.url, version: program.version })}</p>
        {draft.editable || draft.editRefusal === null || draft.editRefusal === undefined ? null : <p className={muted}>{intl.formatMessage(m.pdReadOnly, { reason: refusalText(intl, draft.editRefusal) })}</p>}
      </header>
      {stale ? (
        <div role="alert" className={css({ display: 'flex', gap: '3', alignItems: 'center', flexWrap: 'wrap' })}>
          <span className={alert}>{intl.formatMessage(m.pdStale)}</span>
          <button
            type="button"
            className={button}
            onClick={() => {
              create.reset()
              reorder.reset()
              onReload()
            }}
          >
            {intl.formatMessage(m.pdReload)}
          </button>
        </div>
      ) : null}
      {create.isError && !stale ? (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.pdCreateFailed, { reason: why(create.error) })}
        </p>
      ) : null}
      {reorder.isError && !stale ? (
        <p role="alert" className={alert}>
          {intl.formatMessage(m.pdSaveFailed, { reason: why(reorder.error) })}
        </p>
      ) : null}
      {reorder.isSuccess ? <p role="status">{intl.formatMessage(m.pdMoved)}</p> : null}

      <Strip program={program} care={care} draft={draft} points={overridePoints(overrides.text)} context={context} />

      <div className={css({ display: 'grid', gap: '4', alignItems: 'start', gridTemplateColumns: { base: '1fr', xl: 'minmax(0, 1fr) 20rem' } })}>
        <section aria-labelledby="program-board" className={css({ display: 'flex', flexDirection: 'column', gap: '2', minWidth: '0' })}>
          <h2 id="program-board" className={subheading}>
            {intl.formatMessage(m.pdBoard)}
          </h2>
          <p className={muted}>{intl.formatMessage(m.pdBoardHint)}</p>
          {columns === null ? (
            careFile.settled ? (
              <p>{intl.formatMessage(m.pdNoBoard)}</p>
            ) : (
              <p role="status">{intl.formatMessage(m.pdLoading)}</p>
            )
          ) : (
            <ol className={css({ display: 'grid', gridAutoFlow: 'column', gridAutoColumns: 'minmax(16rem, 1fr)', gap: '3', overflowX: 'auto', pb: '2' })}>
              {columns.map((column, n) => (
                <Stage key={column.id} n={n + 1} column={column} program={program} context={context} moving={reorder.isPending} onMove={move} />
              ))}
            </ol>
          )}
        </section>
        <Panel found={found} context={context} />
      </div>
    </>
  )
}

/** What applies to the whole program: who it is for, the care workflow, where it is activated
 * and how many override points a facility admin may fill in. */
function Strip({ program, care, draft, points, context }: { program: Program; care: Linked | null; draft: StudioPackagePage; points: number; context: Context }) {
  const intl = useIntl()
  return (
    <section aria-label={intl.formatMessage(m.pdStrip)} className={cx(box, css({ display: 'grid', gap: '3', gridTemplateColumns: { base: '1fr', lg: 'repeat(4, minmax(0, 1fr))' } }))}>
      <div className={list}>
        <h2 className={small}>{intl.formatMessage(m.pdWho)}</h2>
        {program.eligibility === null ? <span>{intl.formatMessage(m.pdEveryone)}</span> : <code className={mono}>{program.eligibility}</code>}
        {program.episodeType === '' ? null : <span className={muted}>{intl.formatMessage(m.pdEpisode, { type: program.episodeType })}</span>}
      </div>
      <div className={list}>
        <h2 className={small}>{intl.formatMessage(m.pdRunsOn)}</h2>
        {care === null ? <span className={alert}>{intl.formatMessage(m.pdNoCareWorkflow)}</span> : <Artefact linked={care} context={context} />}
      </div>
      <div className={list}>
        <h2 className={small}>{intl.formatMessage(m.pdScopes)}</h2>
        {draft.package.active.length === 0 ? (
          <span>{intl.formatMessage(m.pdScopesNone)}</span>
        ) : (
          <ul className={list}>
            {draft.package.active.map((scope) => (
              <li key={scope.scope}>{intl.formatMessage(m.pdScope, { scope: scope.scope, version: scope.version })}</li>
            ))}
          </ul>
        )}
      </div>
      <div className={list}>
        <span>{intl.formatMessage(m.pdOverrides, { count: points })}</span>
      </div>
    </section>
  )
}

function Stage({ n, column, program, context, moving, onMove }: { n: number; column: Column; program: Program; context: Context; moving: boolean; onMove: (stage: string, index: number, by: -1 | 1) => void }) {
  const intl = useIntl()
  const headingId = `program-stage-${column.id}`
  return (
    <li aria-labelledby={headingId} className={cx(box, css({ display: 'flex', flexDirection: 'column', gap: '3', fontSize: 'sm' }))}>
      <h3 id={headingId} className={css({ fontWeight: 'semibold', fontSize: 'md' })}>
        {intl.formatMessage(m.pdStage, { n, id: column.id })}
        {column.kind === 'final' ? <span className={muted}> · {intl.formatMessage(m.pdFinal)}</span> : null}
      </h3>
      {column.forms.length === 0 ? null : (
        <Part title={intl.formatMessage(m.pdForm)}>
          <ul className={list}>
            {column.forms.map((form) => (
              <li key={form.canonical}>
                <Artefact linked={form} context={context} />
              </li>
            ))}
          </ul>
        </Part>
      )}
      <Part title={intl.formatMessage(m.pdHappens)}>
        {column.happens.length === 0 ? (
          <span className={muted}>{intl.formatMessage(m.pdNothingHappens)}</span>
        ) : (
          <ul className={bullets}>
            {column.happens.map((h, i) => (
              <li key={i}>{happening(intl, h)}</li>
            ))}
          </ul>
        )}
      </Part>
      {column.investigations.length === 0 ? null : (
        <Part title={intl.formatMessage(m.pdInvestigations)}>
          <ul className={list}>
            {column.investigations.map((linked) => (
              <li key={linked.canonical}>
                <Artefact linked={linked} context={context} />
              </li>
            ))}
          </ul>
        </Part>
      )}
      {column.regimens === null ? null : (
        <Part title={intl.formatMessage(m.pdRegimens, { count: column.regimens.length })}>
          <p className={muted}>{intl.formatMessage(m.pdRegimensHint)}</p>
          <ol className={list}>
            {column.regimens.map((row, index) => {
              const title = row.rule.title === '' ? String(index + 1) : row.rule.title
              const last = (column.regimens?.length ?? 0) - 1
              const source = row.rule.cites.find((c) => program.sources.includes(c))
              return (
                <li key={`${index}:${row.rule.title}`} className={cx(box, css({ p: '2', display: 'flex', gap: '2', alignItems: 'flex-start' }))}>
                  {context.draft.editable ? (
                    <div className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
                      <button type="button" className={ghost} aria-label={intl.formatMessage(m.pdMoveUp, { title })} disabled={index === 0 || moving} onClick={() => onMove(column.id, index, -1)}>
                        ▲
                      </button>
                      <button type="button" className={ghost} aria-label={intl.formatMessage(m.pdMoveDown, { title })} disabled={index === last || moving} onClick={() => onMove(column.id, index, 1)}>
                        ▼
                      </button>
                    </div>
                  ) : null}
                  <div className={css({ display: 'flex', flexDirection: 'column', gap: '1', minWidth: '0' })}>
                    <strong>{intl.formatMessage(m.pdRule, { n: index + 1, title })}</strong>
                    {row.rule.condition === null ? (
                      <span className={alert}>{intl.formatMessage(m.pdRuleNoCondition)}</span>
                    ) : (
                      <code className={mono}>{intl.formatMessage(m.pdRuleWhen, { condition: row.rule.condition })}</code>
                    )}
                    {row.activity === null ? <span className={alert}>{intl.formatMessage(m.pdRuleNoActivity)}</span> : <Artefact linked={row.activity} context={context} />}
                    {source === undefined ? (
                      <span className={alert}>{intl.formatMessage(m.pdRuleUncited)}</span>
                    ) : (
                      <span className={muted}>{intl.formatMessage(m.pdRuleSource, { source: program.sourceNames[source] ?? source })}</span>
                    )}
                  </div>
                </li>
              )
            })}
          </ol>
        </Part>
      )}
      {column.schedule.length === 0 ? null : (
        <Part title={intl.formatMessage(m.pdSchedule)}>
          <ul className={bullets}>
            {column.schedule.map((t) => (
              <li key={`${t.event}:${t.after}`}>{intl.formatMessage(m.pdTimer, { event: t.event, after: t.after })}</li>
            ))}
          </ul>
        </Part>
      )}
      {column.outcomes.length === 0 && column.indicators.length === 0 ? null : (
        <Part title={intl.formatMessage(column.outcomes.length === 0 ? m.pdIndicators : m.pdOutcomes)}>
          {column.outcomes.length === 0 ? null : <span>{column.outcomes.join(', ')}</span>}
          {column.indicators.length === 0 ? null : (
            <ul className={list} aria-label={intl.formatMessage(m.pdIndicators)}>
              {column.indicators.map((linked) => (
                <li key={linked.canonical}>
                  <Artefact linked={linked} context={context} />
                </li>
              ))}
            </ul>
          )}
        </Part>
      )}
      <div className={css({ mt: 'auto', pt: '2', borderTop: '1px solid', borderColor: 'border' })}>
        <Part title={intl.formatMessage(m.pdLeaves)}>
          {column.kind === 'final' ? (
            <span>{intl.formatMessage(m.pdLeavesFinal)}</span>
          ) : column.leaves.length === 0 ? (
            <span className={alert}>{intl.formatMessage(m.pdLeavesNever)}</span>
          ) : (
            <ul className={bullets}>
              {column.leaves.map((exit, i) => (
                <li key={i}>{exitText(intl, exit)}</li>
              ))}
            </ul>
          )}
        </Part>
      </div>
    </li>
  )
}

function Part({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className={list}>
      <h4 className={small}>{title}</h4>
      {children}
    </div>
  )
}

type Intl = ReturnType<typeof useIntl>

function happening(intl: Intl, h: Happening): string {
  switch (h.kind) {
    case 'inside':
      return intl.formatMessage(m.pdInside, { states: h.states.join(', ') })
    case 'task':
      return intl.formatMessage(m.pdTask, { role: h.role, worklist: h.worklist })
    case 'gate':
      return intl.formatMessage(m.pdGate, { gate: h.gate })
    case 'invoke':
      return intl.formatMessage(m.pdInvoke, { src: h.src })
    case 'execute':
      return intl.formatMessage(m.pdExecute, { action: h.action })
    case 'raise':
      return intl.formatMessage(m.pdRaise, { event: h.event })
    case 'assign':
      return intl.formatMessage(m.pdAssign, { field: h.field, value: String(h.value) })
  }
}

function exitText(intl: Intl, exit: Exit): string {
  const targets = exit.targets.join(', ')
  const base =
    exit.after !== null
      ? intl.formatMessage(m.pdExitTimer, { targets, after: exit.after })
      : exit.events.length === 0
        ? intl.formatMessage(m.pdExitEventless, { targets })
        : intl.formatMessage(m.pdExit, { targets, events: exit.events.join(', ') })
  return exit.guard === null || exit.guard === '' ? base : `${base} ${intl.formatMessage(m.pdGuard, { guard: exit.guard })}`
}

/** One linked artefact: its kind, its `url|version`, where it stands, and *Open* into its
 * editor or, when missing, *Create*. */
function Artefact({ linked, context }: { linked: Linked; context: Context }) {
  const intl = useIntl()
  const version = linked.version ?? linked.pinned ?? ''
  return (
    <span className={css({ display: 'inline-flex', gap: '2', alignItems: 'baseline', flexWrap: 'wrap', minWidth: '0' })}>
      <span className={muted}>{intl.formatMessage(m.pdKind, { kind: linked.kind })}</span>
      <code className={mono}>{linked.canonical}</code>
      <span className={cx(css({ fontSize: 'xs' }), tone[linked.where])}>{intl.formatMessage(m.pdWhere, { where: linked.where, version })}</span>
      {linked.path !== null ? (
        <EditorLink id={context.id} kind={linked.kind} path={linked.path} workflows={context.workflows} label={intl.formatMessage(m.pdOpenArtefact, { canonical: linked.canonical })} />
      ) : linked.where === 'missing' ? (
        <button
          type="button"
          className={ghost}
          aria-label={intl.formatMessage(m.pdCreateLabel, { canonical: linked.canonical })}
          disabled={!context.draft.editable || context.creating}
          onClick={() => context.create(linked)}
        >
          {intl.formatMessage(m.pdCreate)}
        </button>
      ) : null}
    </span>
  )
}

function EditorLink({ id, kind, path, workflows, label, children }: { id: string; kind: string | null; path: string; workflows: Set<string>; label: string; children?: React.ReactNode }) {
  const intl = useIntl()
  const text = children ?? intl.formatMessage(m.pdOpenText)
  switch (editorOf(kind, path, workflows)) {
    case 'forms':
      return (
        <Link to="/studio/$id/forms/$" params={{ id, _splat: path }} aria-label={label}>
          {text}
        </Link>
      )
    case 'workflows':
      return (
        <Link to="/studio/$id/workflows/$" params={{ id, _splat: path }} aria-label={label}>
          {text}
        </Link>
      )
    case 'terminology':
      return (
        <Link to="/studio/$id/terminology/$" params={{ id, _splat: path }} aria-label={label}>
          {text}
        </Link>
      )
    default:
      return (
        <Link to="/studio/$id/files/$" params={{ id, _splat: path }} aria-label={label}>
          {text}
        </Link>
      )
  }
}

/** The package panel: the files by kind, the missing ones marked, and what is still missing. */
function Panel({ found, context }: { found: Gap[]; context: Context }) {
  const intl = useIntl()
  const missing = found.flatMap((g) => (g.kind === 'missing' ? [g.linked] : []))
  const groups = panel(context.draft.files, context.workflows, missing)
  return (
    <aside aria-labelledby="program-panel" className={cx(box, css({ display: 'flex', flexDirection: 'column', gap: '3' }))}>
      <h2 id="program-panel" className={subheading}>
        {intl.formatMessage(m.pdPanel)} <span className={mono}>{context.draft.package.id}</span>
      </h2>
      <p className={muted}>{intl.formatMessage(m.pdPanelHint)}</p>
      <ul className={cx(list, mono)}>
        {groups.map((group) => (
          <li key={group.folder}>
            {group.folder}
            <ul className={css({ pl: '4', display: 'flex', flexDirection: 'column', gap: '1' })}>
              {group.entries.map((entry) => (
                <li key={entry.label} className={entry.missing === null ? undefined : tone.missing}>
                  {entry.path === null ? (
                    <>
                      {entry.label} ({intl.formatMessage(m.pdMissingFile)})
                    </>
                  ) : (
                    <EditorLink id={context.id} kind={entry.resourceType} path={entry.path} workflows={context.workflows} label={intl.formatMessage(sm.sdOpenFile, { path: entry.path })}>
                      {entry.label}
                    </EditorLink>
                  )}
                </li>
              ))}
            </ul>
          </li>
        ))}
      </ul>
      <section aria-labelledby="program-missing" className={list}>
        <h2 id="program-missing" className={subheading}>
          {intl.formatMessage(m.pdStillMissing, { count: found.length })}
        </h2>
        {found.length === 0 ? (
          <p>{intl.formatMessage(m.pdNothingMissing)}</p>
        ) : (
          <ul className={bullets}>
            {found.map((gap, n) => (
              <li key={n}>{gapText(intl, gap)}</li>
            ))}
          </ul>
        )}
        <p className={muted}>{intl.formatMessage(m.pdAuthority)}</p>
      </section>
    </aside>
  )
}

function gapText(intl: Intl, gap: Gap): string {
  switch (gap.kind) {
    case 'missing':
      return intl.formatMessage(m.pdGapMissing, { kind: gap.linked.kind, canonical: gap.linked.canonical })
    case 'no-care-workflow':
      return intl.formatMessage(m.pdGapNoCareWorkflow)
    case 'unknown-stage':
      return intl.formatMessage(m.pdGapUnknownStage, { stage: gap.stage })
    case 'uncited':
      return intl.formatMessage(m.pdGapUncited, { stage: gap.stage, title: gap.title })
    case 'no-condition':
      return intl.formatMessage(m.pdGapNoCondition, { stage: gap.stage, title: gap.title })
    case 'no-activity':
      return intl.formatMessage(m.pdGapNoActivity, { stage: gap.stage, title: gap.title })
    case 'pinned':
      return intl.formatMessage(m.pdGapPinned, { kind: gap.linked.kind, canonical: gap.linked.canonical, version: gap.linked.version ?? '' })
  }
}
