import { Field } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { areaMessages, placesMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { ASSIGNMENTS, type AssignmentOptions, type PlaceLevel, PlacesError, assign, optionsQuery, placeLabel, userQuery, withdraw } from './api'

const page = css({ display: 'flex', flexDirection: 'column', gap: '6', maxWidth: '3xl' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const section = css({ display: 'flex', flexDirection: 'column', gap: '3' })
const list = css({ display: 'flex', flexDirection: 'column', gap: '2' })
const row = css({ display: 'flex', flexWrap: 'wrap', gap: '2', alignItems: 'center' })
const label = css({ display: 'flex', flexDirection: 'column', gap: '1' })
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

/**
 * Role assignment at a place at central (ADR 0094; #873), for an administrator whose role gives
 * `access.administer`: look a user up, see the places they hold roles at, assign roles at a
 * facility, district, region or the national place within the administrator's area, or withdraw
 * them. The user signs in for the place from their next sign-in.
 */
export function RolesPage() {
  const intl = useIntl()
  const { transport } = usePlatform()
  const options = useQuery(optionsQuery(transport))
  const [draft, setDraft] = useState('')
  const [username, setUsername] = useState<string | null>(null)
  if (options.data === undefined) {
    if (options.isPending) return <p role="status">{intl.formatMessage(m.plLoading)}</p>
    const forbidden = options.error instanceof PlacesError && options.error.status === 403
    return (
      <section className={page}>
        <h1 className={heading}>{intl.formatMessage(m.plRolesTitle)}</h1>
        <p role="alert" className={css({ color: forbidden ? 'fg.muted' : 'danger.text' })}>
          {forbidden ? intl.formatMessage(m.plRolesForbidden) : intl.formatMessage(m.plFailed, { reason: String(options.error) })}
        </p>
      </section>
    )
  }
  return (
    <section className={page}>
      <h1 className={heading}>{intl.formatMessage(m.plRolesTitle)}</h1>
      <p>{intl.formatMessage(m.plRolesIntro)}</p>
      <form
        className={row}
        onSubmit={(event) => {
          event.preventDefault()
          if (draft.trim() !== '') setUsername(draft.trim())
        }}
      >
        <Field.Root required>
          <Field.Label>{intl.formatMessage(m.plUsername)}</Field.Label>
          <Field.Input autoComplete="off" value={draft} onChange={(event) => setDraft(event.target.value)} />
        </Field.Root>
        <button type="submit" className={button}>
          {intl.formatMessage(m.plFind)}
        </button>
      </form>
      {username === null ? null : <UserRoles key={username} username={username} options={options.data} />}
    </section>
  )
}

function useLevelName() {
  const intl = useIntl()
  return (level: PlaceLevel) => intl.formatMessage(areaMessages[`arLevel_${level}`])
}

function reasonOf(error: unknown): string {
  if (error instanceof PlacesError) return error.detail === '' ? String(error.status) : error.detail
  return String(error)
}

/** One user's places and roles, and the forms that change them. */
function UserRoles({ username, options }: { username: string; options: AssignmentOptions }) {
  const intl = useIntl()
  const { transport } = usePlatform()
  const queryClient = useQueryClient()
  const levelName = useLevelName()
  const held = useQuery(userQuery(transport, username))
  const [place, setPlace] = useState('')
  const [roles, setRoles] = useState<string[]>([])
  const [confirming, setConfirming] = useState<string | null>(null)
  const refresh = () => queryClient.invalidateQueries({ queryKey: [ASSIGNMENTS, 'user', username] })
  const assigning = useMutation({ mutationFn: () => assign(transport, username, place, roles), onSettled: refresh })
  const withdrawing = useMutation({
    mutationFn: (code: string) => withdraw(transport, username, code),
    onSettled: () => {
      setConfirming(null)
      return refresh()
    },
  })
  const time = (iso: string) => intl.formatDate(new Date(iso), { dateStyle: 'medium' })
  const nameOf = (code: string) => {
    const known = options.places.find((p) => p.code === code)
    return known === undefined ? code : placeLabel(known, levelName)
  }
  if (held.data === undefined) {
    if (held.isPending) return <p role="status">{intl.formatMessage(m.plLoading)}</p>
    const missing = held.error instanceof PlacesError && held.error.status === 404
    return (
      <p role="alert" className={css({ color: 'danger.text' })}>
        {missing ? intl.formatMessage(m.plNoSuchUser) : intl.formatMessage(m.plFailed, { reason: reasonOf(held.error) })}
      </p>
    )
  }
  const chooseRole = (role: string, on: boolean) => setRoles((current) => options.roles.filter((r) => (r === role ? on : current.includes(r))))
  const choosePlace = (code: string) => {
    setPlace(code)
    // Start from what the user holds there: assigning replaces it.
    setRoles(held.data.assignments.find((a) => a.place.code === code)?.roles ?? [])
    assigning.reset()
  }
  return (
    <>
      <section className={section} aria-labelledby="roles-held">
        <h2 id="roles-held" className={subheading}>
          {intl.formatMessage(m.plHeld, { username: held.data.username })}
        </h2>
        {held.data.assignments.length === 0 ? (
          <p>{intl.formatMessage(m.plNone)}</p>
        ) : (
          <ul className={list}>
            {held.data.assignments.map((a) => (
              <li key={a.place.code} className={row}>
                <span>{intl.formatMessage(m.plHeldLine, { place: placeLabel(a.place, levelName), roles: a.roles.join(', '), until: time(a.not_after) })}</span>
                {a.manageable ? (
                  confirming === a.place.code ? (
                    <button type="button" className={quiet} disabled={withdrawing.isPending} onClick={() => withdrawing.mutate(a.place.code)}>
                      {intl.formatMessage(m.plWithdrawConfirm, { place: a.place.name })}
                    </button>
                  ) : (
                    <button type="button" className={quiet} onClick={() => setConfirming(a.place.code)}>
                      {intl.formatMessage(m.plWithdraw)}
                    </button>
                  )
                ) : null}
              </li>
            ))}
          </ul>
        )}
        {withdrawing.isSuccess ? <p role="status">{intl.formatMessage(m.plWithdrawn, { place: nameOf(withdrawing.variables) })}</p> : null}
        {withdrawing.isError ? <p role="alert">{intl.formatMessage(m.plFailed, { reason: reasonOf(withdrawing.error) })}</p> : null}
      </section>
      <form
        className={section}
        aria-labelledby="roles-assign"
        onSubmit={(event) => {
          event.preventDefault()
          if (place !== '' && roles.length > 0) assigning.mutate()
        }}
      >
        <h2 id="roles-assign" className={subheading}>
          {intl.formatMessage(m.plAssignHeading)}
        </h2>
        <label className={label}>
          {intl.formatMessage(m.plPlace)}
          <select required value={place} onChange={(event) => choosePlace(event.target.value)} className={select}>
            <option value="">{intl.formatMessage(m.plChoosePlace)}</option>
            {options.places.map((p) => (
              <option key={p.code} value={p.code}>
                {placeLabel(p, levelName)}
              </option>
            ))}
          </select>
        </label>
        <fieldset className={section}>
          <legend>{intl.formatMessage(m.plRoles)}</legend>
          {options.roles.map((role) => (
            <label key={role} className={row}>
              <input type="checkbox" checked={roles.includes(role)} onChange={(event) => chooseRole(role, event.target.checked)} />
              {role}
            </label>
          ))}
        </fieldset>
        <p>{intl.formatMessage(m.plReplaceNote)}</p>
        <button type="submit" className={button} disabled={place === '' || roles.length === 0 || assigning.isPending}>
          {intl.formatMessage(m.plSave)}
        </button>
        {assigning.isSuccess ? <p role="status">{intl.formatMessage(m.plAssigned, { username, roles: roles.join(', '), place: nameOf(place) })}</p> : null}
        {assigning.isError ? <p role="alert">{intl.formatMessage(m.plFailed, { reason: reasonOf(assigning.error) })}</p> : null}
      </form>
    </>
  )
}
