import { Field } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { accessMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { ACCESS, AccessError, grant, grantsQuery, provision, revocationsQuery, revoke, rolesQuery } from './api'

const page = css({ display: 'flex', flexDirection: 'column', gap: '6' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const section = css({ display: 'flex', flexDirection: 'column', gap: '3', maxWidth: 'md' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const list = css({ display: 'flex', flexDirection: 'column', gap: '2' })
const select = css({ minHeight: 'control', px: '2', borderRadius: 'md', border: '1px solid', borderColor: 'border' })
const button = css({
  minHeight: 'control',
  px: 'controlX',
  bg: 'accent.default',
  color: 'accent.fg',
  borderRadius: 'md',
  fontSize: 'control',
  cursor: 'pointer',
  alignSelf: 'flex-start',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
  _disabled: { opacity: '0.5', cursor: 'not-allowed' },
})
const quiet = css({
  minHeight: 'control',
  px: 'controlX',
  borderRadius: 'md',
  borderWidth: '1px',
  borderColor: 'border.default',
  cursor: 'pointer',
  _focusVisible: { outline: '2px solid', outlineColor: 'focusRing' },
})

/** At most 7 days (offline-auth.md § Offline user and role administration). */
const DAYS = [1, 2, 3, 4, 5, 6, 7]

/**
 * Offline administration on a facility edge (offline-auth.md § Offline user and role
 * administration), for a facility admin: grant a locally grantable role for up to 7 days, create a
 * provisional account for new staff, and revoke someone's access here at once. Central ratifies
 * or reverses all of it when the edge reconnects.
 */
export function AccessPage() {
  const intl = useIntl()
  const { transport } = usePlatform()
  const roles = useQuery(rolesQuery(transport))
  if (roles.data === undefined) {
    if (roles.isPending) return <p role="status">{intl.formatMessage(m.acLoading)}</p>
    const forbidden = roles.error instanceof AccessError && roles.error.status === 403
    return (
      <section className={page}>
        <h1 className={heading}>{intl.formatMessage(m.acTitle)}</h1>
        <p role="alert" className={css({ color: forbidden ? 'fg.muted' : 'danger.text' })}>
          {intl.formatMessage(forbidden ? m.acForbidden : m.acFailed)}
        </p>
      </section>
    )
  }
  return (
    <section className={page}>
      <h1 className={heading}>{intl.formatMessage(m.acTitle)}</h1>
      <p>{intl.formatMessage(m.acIntro)}</p>
      <GrantSection roles={roles.data} />
      <ProvisionSection roles={roles.data} />
      <RevokeSection />
    </section>
  )
}

/** The edge's reason for a refusal: its detail where it gave one. */
function useReason() {
  const intl = useIntl()
  return (error: unknown) => {
    if (error instanceof AccessError) {
      if (error.code === 'no-such-user') return intl.formatMessage(m.acNoSuchUser)
      if (error.status === 409) return intl.formatMessage(m.acTaken)
      if (error.detail !== '') return error.detail
      return error.code
    }
    return String(error)
  }
}

function RoleAndDays({ roles, role, days, onRole, onDays }: { roles: string[]; role: string; days: number; onRole: (role: string) => void; onDays: (days: number) => void }) {
  const intl = useIntl()
  const label = css({ display: 'flex', flexDirection: 'column', gap: '1' })
  return (
    <>
      <label className={label}>
        {intl.formatMessage(m.acRole)}
        <select required value={role} onChange={(event) => onRole(event.target.value)} className={select}>
          <option value="">{intl.formatMessage(m.acChooseRole)}</option>
          {roles.map((r) => (
            <option key={r} value={r}>
              {r}
            </option>
          ))}
        </select>
      </label>
      <label className={label}>
        {intl.formatMessage(m.acDays)}
        <select value={days} onChange={(event) => onDays(Number(event.target.value))} className={select}>
          {DAYS.map((d) => (
            <option key={d} value={d}>
              {intl.formatMessage(m.acDayCount, { days: d })}
            </option>
          ))}
        </select>
      </label>
    </>
  )
}

function GrantSection({ roles }: { roles: string[] }) {
  const intl = useIntl()
  const { transport } = usePlatform()
  const queryClient = useQueryClient()
  const reason = useReason()
  const grants = useQuery(grantsQuery(transport))
  const [username, setUsername] = useState('')
  const [role, setRole] = useState('')
  const [days, setDays] = useState(7)
  const granting = useMutation({
    mutationFn: () => grant(transport, username.trim(), role, days),
    onSuccess: () => {
      setUsername('')
      setRole('')
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: [ACCESS] }),
  })
  const time = (iso: string) => intl.formatDate(new Date(iso), { dateStyle: 'medium', timeStyle: 'short' })
  return (
    <section className={section} aria-labelledby="access-grants">
      <h2 id="access-grants" className={subheading}>
        {intl.formatMessage(m.acGrants)}
      </h2>
      {grants.data === undefined || grants.data.length === 0 ? (
        <p>{intl.formatMessage(m.acNoGrants)}</p>
      ) : (
        <ul className={list}>
          {grants.data.map((g) => (
            <li key={g.id}>{intl.formatMessage(m.acGrantLine, { username: g.username ?? g.account, role: g.role, until: time(g.not_after) })}</li>
          ))}
        </ul>
      )}
      <form
        className={section}
        onSubmit={(event) => {
          event.preventDefault()
          if (username.trim() !== '' && role !== '') granting.mutate()
        }}
      >
        <Field.Root required>
          <Field.Label>{intl.formatMessage(m.acUsername)}</Field.Label>
          <Field.Input autoComplete="off" value={username} onChange={(event) => setUsername(event.target.value)} />
        </Field.Root>
        <RoleAndDays roles={roles} role={role} days={days} onRole={setRole} onDays={setDays} />
        <button type="submit" className={button} disabled={granting.isPending}>
          {intl.formatMessage(m.acGrant)}
        </button>
        {granting.isSuccess ? (
          <p role="status">{intl.formatMessage(m.acGranted, { username: granting.data.username ?? '', role: granting.data.role, until: time(granting.data.not_after) })}</p>
        ) : null}
        {granting.isError ? <p role="alert">{intl.formatMessage(m.acRefused, { reason: reason(granting.error) })}</p> : null}
      </form>
    </section>
  )
}

function ProvisionSection({ roles }: { roles: string[] }) {
  const intl = useIntl()
  const { transport } = usePlatform()
  const queryClient = useQueryClient()
  const reason = useReason()
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [role, setRole] = useState('')
  const [days, setDays] = useState(7)
  const creating = useMutation({
    mutationFn: () => provision(transport, { username: username.trim(), password, role, days }),
    onSuccess: () => {
      setUsername('')
      setPassword('')
      setRole('')
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: [ACCESS] }),
  })
  const time = (iso: string) => intl.formatDate(new Date(iso), { dateStyle: 'medium', timeStyle: 'short' })
  return (
    <section className={section} aria-labelledby="access-accounts">
      <h2 id="access-accounts" className={subheading}>
        {intl.formatMessage(m.acNewStaff)}
      </h2>
      <p>{intl.formatMessage(m.acNewStaffIntro)}</p>
      <form
        className={section}
        onSubmit={(event) => {
          event.preventDefault()
          if (username.trim() !== '' && password !== '' && role !== '') creating.mutate()
        }}
      >
        <Field.Root required>
          <Field.Label>{intl.formatMessage(m.acUsername)}</Field.Label>
          <Field.Input autoComplete="off" value={username} onChange={(event) => setUsername(event.target.value)} />
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(m.acTemporaryPassword)}</Field.Label>
          <Field.Input type="password" autoComplete="new-password" value={password} onChange={(event) => setPassword(event.target.value)} />
          <Field.HelperText>{intl.formatMessage(m.acTemporaryPasswordHelp)}</Field.HelperText>
        </Field.Root>
        <RoleAndDays roles={roles} role={role} days={days} onRole={setRole} onDays={setDays} />
        <button type="submit" className={button} disabled={creating.isPending}>
          {intl.formatMessage(m.acCreate)}
        </button>
        {creating.isSuccess ? (
          <p role="status">{intl.formatMessage(m.acCreated, { username: creating.data.username, role: creating.data.role, until: time(creating.data.not_after) })}</p>
        ) : null}
        {creating.isError ? <p role="alert">{intl.formatMessage(m.acRefused, { reason: reason(creating.error) })}</p> : null}
      </form>
    </section>
  )
}

function RevokeSection() {
  const intl = useIntl()
  const { transport } = usePlatform()
  const queryClient = useQueryClient()
  const reason = useReason()
  const revocations = useQuery(revocationsQuery(transport))
  const [username, setUsername] = useState('')
  const [confirming, setConfirming] = useState(false)
  const revoking = useMutation({
    mutationFn: () => revoke(transport, username.trim()),
    onSuccess: () => setUsername(''),
    onSettled: () => {
      setConfirming(false)
      return queryClient.invalidateQueries({ queryKey: [ACCESS] })
    },
  })
  const time = (iso: string) => intl.formatDate(new Date(iso), { dateStyle: 'medium', timeStyle: 'short' })
  return (
    <section className={section} aria-labelledby="access-revocations">
      <h2 id="access-revocations" className={subheading}>
        {intl.formatMessage(m.acRevocations)}
      </h2>
      <p>{intl.formatMessage(m.acRevokeIntro)}</p>
      {revocations.data === undefined || revocations.data.length === 0 ? (
        <p>{intl.formatMessage(m.acNoRevocations)}</p>
      ) : (
        <ul className={list}>
          {revocations.data.map((r) => (
            <li key={r.account}>{intl.formatMessage(m.acRevocationLine, { username: r.username ?? r.account, at: time(r.revoked_at) })}</li>
          ))}
        </ul>
      )}
      <form
        className={section}
        onSubmit={(event) => {
          event.preventDefault()
          if (username.trim() !== '') setConfirming(true)
        }}
      >
        <Field.Root required>
          <Field.Label>{intl.formatMessage(m.acUsername)}</Field.Label>
          <Field.Input autoComplete="off" value={username} onChange={(event) => setUsername(event.target.value)} readOnly={confirming} />
        </Field.Root>
        {confirming ? (
          <div className={css({ display: 'flex', flexWrap: 'wrap', gap: '3' })}>
            <button type="button" className={button} disabled={revoking.isPending} onClick={() => revoking.mutate()}>
              {intl.formatMessage(m.acConfirmRevoke, { username: username.trim() })}
            </button>
            <button type="button" className={quiet} onClick={() => setConfirming(false)}>
              {intl.formatMessage(m.acCancel)}
            </button>
          </div>
        ) : (
          <button type="submit" className={button}>
            {intl.formatMessage(m.acRevoke)}
          </button>
        )}
        {revoking.isSuccess ? <p role="status">{intl.formatMessage(m.acRevoked, { username: revoking.data.username ?? '' })}</p> : null}
        {revoking.isError ? <p role="alert">{intl.formatMessage(m.acRefused, { reason: reason(revoking.error) })}</p> : null}
      </form>
    </section>
  )
}
