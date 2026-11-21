import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { useId } from 'react'
import { useIntl } from 'react-intl'

import { designMessages as dm } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { button } from '../reception/styles'
import { StudioError, packageQuery } from './api'
import { openDraft } from './create'
import { changedKeys, designsQuery, editPlan, packageOf } from './designs'
import { refusalText, stageLabel } from './wording'

const bar = css({
  display: 'flex',
  flexWrap: 'wrap',
  gap: '3',
  alignItems: 'center',
  justifyContent: 'space-between',
  p: '3',
  borderRadius: 'md',
  border: '1px solid',
  borderColor: 'border',
  bg: 'bg.subtle',
  maxWidth: '6xl',
})
const row = css({ display: 'flex', flexWrap: 'wrap', gap: '3', alignItems: 'center' })
const muted = css({ color: 'fg.muted', fontSize: 'sm' })

/**
 * The header above every Studio editor (#855): the package and version the design belongs to,
 * where it stands, and a link to its draft page (to check, submit, see the diff and discuss).
 * Read only, it offers *Edit*: the draft the person holds is reopened, or the next version of
 * the published package is started for them (#837), and `onOpen` reopens the editor there. When
 * someone else holds the draft, or it is submitted, it says so instead.
 */
export function EditorBar({ id, onOpen }: { id: string; onOpen: (id: string) => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const hint = useId()
  const draft = useQuery(packageQuery(platform.transport, id))
  const designs = useQuery(designsQuery(platform.transport))
  const summary = draft.data?.package
  const plan = summary === undefined || designs.data === undefined ? null : editPlan(packageOf(designs.data, summary.name))
  const edit = useMutation({
    mutationFn: async () => {
      if (plan === null || (plan.kind !== 'reuse' && plan.kind !== 'start')) throw new Error('nothing to edit')
      return openDraft(platform.transport, plan)
    },
    onSuccess: async (next) => {
      queryClient.setQueryData(packageQuery(platform.transport, next.package.id).queryKey, next)
      await Promise.all(changedKeys.map((queryKey) => queryClient.invalidateQueries({ queryKey, exact: true })))
      onOpen(next.package.id)
    },
  })
  if (summary === undefined || draft.data === undefined) return null
  const published = summary.stage === 'published'
  const reason =
    edit.error instanceof StudioError ? (edit.error.refusal === null || edit.error.refusal === undefined ? edit.error.detail : refusalText(intl, edit.error.refusal)) : edit.error instanceof Error ? edit.error.message : ''
  const offer = !draft.data.editable && plan !== null && ((plan.kind === 'reuse' && plan.id !== id) || plan.kind === 'start')
  return (
    <nav aria-label={intl.formatMessage(dm.dhBarLabel)} className={bar}>
      <div className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
        <span>{intl.formatMessage(dm.dhBarPackage, { title: summary.title ?? summary.name, name: summary.name, version: summary.version, stage: stageLabel(intl, summary.stage) })}</span>
        <span className={row}>
          <Link to="/studio">{intl.formatMessage(dm.dhHome)}</Link>
          <Link to="/studio/$id" params={{ id }}>
            {intl.formatMessage(published ? dm.dhBarPackagePage : dm.dhBarDraftPage)}
          </Link>
        </span>
      </div>
      {draft.data.editable || plan === null ? null : offer ? (
        <div className={row}>
          <button type="button" className={button} aria-describedby={hint} disabled={edit.isPending} onClick={() => edit.mutate()}>
            {intl.formatMessage(dm.dhBarEdit)}
          </button>
          <span id={hint} className={muted}>
            {plan.kind === 'reuse' ? intl.formatMessage(dm.dhBarEditReuse, { id: plan.id }) : intl.formatMessage(dm.dhBarEditStart, { name: summary.name })}
          </span>
          {edit.isPending ? <span role="status">{intl.formatMessage(dm.dhBarEditing)}</span> : null}
          {edit.isError ? (
            <p role="alert" className={css({ color: 'danger.text' })}>
              {intl.formatMessage(dm.dhBarEditFailed, { reason })}
            </p>
          ) : null}
        </div>
      ) : plan.kind === 'held' ? (
        <p className={muted}>{intl.formatMessage(dm.dhBarHeld, { holder: plan.holder })}</p>
      ) : plan.kind === 'submitted' ? (
        <p className={muted}>{intl.formatMessage(dm.dhBarSubmitted)}</p>
      ) : null}
    </nav>
  )
}
