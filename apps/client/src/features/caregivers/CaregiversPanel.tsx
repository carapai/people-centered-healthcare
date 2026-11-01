import { Field } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { SCOPES, adolescentConsent, caregiversKey, caregiversQuery, recordCaregiver, revokeCaregiver, type CaregiverScope, type NewCaregiver } from './api'

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

const scopeLabels = {
  'view-records': messages.cgScopeView,
  'book-appointments': messages.cgScopeBook,
  'request-refills': messages.cgScopeRefills,
  'receive-messages': messages.cgScopeMessages,
} as const

const statusLabels = {
  active: messages.cgStatusActive,
  revoked: messages.cgStatusRevoked,
  expired: messages.cgStatusExpired,
  unverified: messages.cgStatusUnverified,
} as const

type Form = Omit<NewCaregiver, 'patient' | 'scopes'> & { scopes: Set<CaregiverScope> }

const empty: Form = { phone: '', relationship: 'MTH', authority: 'guardian', scopes: new Set(['view-records']), expires: '', document: '' }

const RELATIONSHIPS = [
  ['MTH', messages.cgRelMother],
  ['FTH', messages.cgRelFather],
  ['GRD', messages.cgRelGuardian],
  ['FAMMEMB', messages.cgRelFamily],
] as const

/**
 * A patient's caregivers (patient-identity.md § Caregiver delegation; #113): staff record a
 * caregiver by the phone of their portal account, verifying the document in the same step, and
 * may revoke the access or record the adolescent's consent. The server checks everything.
 */
export function CaregiversPanel({ patient }: { patient: string }) {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const grants = useQuery(caregiversQuery(platform.transport, patient))
  const [form, setForm] = useState<Form>(empty)
  const refresh = () => queryClient.invalidateQueries({ queryKey: caregiversKey(patient) })
  const record = useMutation({
    mutationFn: () => recordCaregiver(platform.transport, { ...form, patient, scopes: [...form.scopes] }),
    onSuccess: () => setForm(empty),
    onSettled: refresh,
  })
  const revoke = useMutation({ mutationFn: (id: string) => revokeCaregiver(platform.transport, id), onSettled: refresh })
  const consent = useMutation({
    mutationFn: ({ id, consents }: { id: string; consents: boolean }) => adolescentConsent(platform.transport, id, consents),
    onSettled: refresh,
  })
  const set = (field: 'phone' | 'relationship' | 'authority' | 'expires' | 'document') => (event: { target: { value: string } }) =>
    setForm({ ...form, [field]: event.target.value })
  const toggle = (scope: CaregiverScope) => {
    const scopes = new Set(form.scopes)
    if (scopes.has(scope)) scopes.delete(scope)
    else scopes.add(scope)
    setForm({ ...form, scopes })
  }
  const complete = form.phone.trim() !== '' && form.expires !== '' && form.document.trim() !== '' && form.scopes.size > 0
  const date = (iso: string) => intl.formatDate(new Date(iso), { dateStyle: 'medium' })

  return (
    <section aria-labelledby="caregivers" className={css({ display: 'flex', flexDirection: 'column', gap: '4' })}>
      <h2 id="caregivers" className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>
        {intl.formatMessage(messages.cgTitle)}
      </h2>
      {grants.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(messages.cgFailed)}
        </p>
      ) : grants.data === undefined ? (
        <p role="status">{intl.formatMessage(messages.bgLoading)}</p>
      ) : grants.data.length === 0 ? (
        <p>{intl.formatMessage(messages.cgNone)}</p>
      ) : (
        <ul className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
          {grants.data.map((grant) => (
            <li key={grant.id} className={css({ display: 'flex', flexDirection: 'column', gap: '2', p: '3', border: '1px solid', borderColor: 'border', borderRadius: 'md' })}>
              <p>
                <strong>{intl.formatMessage(grant.authority === 'guardian' ? messages.cgGuardian : messages.cgDelegated)}</strong> · {intl.formatMessage(statusLabels[grant.status])} ·{' '}
                {intl.formatMessage(messages.cgUntil)} {date(grant.expires)}
              </p>
              <p>{grant.scopes.map((s) => intl.formatMessage(scopeLabels[s])).join(', ')}</p>
              {grant.status === 'active' && grant.authority === 'guardian' ? (
                <p>{intl.formatMessage(grant.adolescent_consents ? messages.cgAdolescentConsents : messages.cgAdolescentWithheld)}</p>
              ) : null}
              {grant.status === 'active' ? (
                <div className={css({ display: 'flex', gap: '3', flexWrap: 'wrap' })}>
                  {grant.authority === 'guardian' ? (
                    <button
                      type="button"
                      className={button}
                      disabled={consent.isPending}
                      onClick={() => consent.mutate({ id: grant.id, consents: !grant.adolescent_consents })}
                    >
                      {intl.formatMessage(grant.adolescent_consents ? messages.cgWithdrawConsent : messages.cgRecordConsent)}
                    </button>
                  ) : null}
                  <button type="button" className={button} disabled={revoke.isPending} onClick={() => revoke.mutate(grant.id)}>
                    {intl.formatMessage(messages.cgRevoke)}
                  </button>
                </div>
              ) : null}
            </li>
          ))}
        </ul>
      )}

      <form
        aria-labelledby="add-caregiver"
        className={css({ display: 'flex', flexDirection: 'column', gap: '3', maxWidth: 'xl' })}
        onSubmit={(event) => {
          event.preventDefault()
          if (complete) record.mutate()
        }}
      >
        <h3 id="add-caregiver" className={css({ fontWeight: 'semibold' })}>
          {intl.formatMessage(messages.cgAdd)}
        </h3>
        <p>{intl.formatMessage(messages.cgAddHint)}</p>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.cgPhone)}</Field.Label>
          <Field.Input type="tel" value={form.phone} onChange={set('phone')} autoComplete="off" />
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.cgRelationship)}</Field.Label>
          <Field.Select value={form.relationship} onChange={set('relationship')}>
            {RELATIONSHIPS.map(([code, label]) => (
              <option key={code} value={code}>
                {intl.formatMessage(label)}
              </option>
            ))}
          </Field.Select>
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.cgAuthority)}</Field.Label>
          <Field.Select value={form.authority} onChange={set('authority')}>
            <option value="guardian">{intl.formatMessage(messages.cgGuardian)}</option>
            <option value="delegated">{intl.formatMessage(messages.cgDelegated)}</option>
          </Field.Select>
        </Field.Root>
        <fieldset className={css({ display: 'flex', flexDirection: 'column', gap: '1' })}>
          <legend>{intl.formatMessage(messages.cgScopes)}</legend>
          {SCOPES.map((scope) => (
            <label key={scope} className={css({ display: 'flex', gap: '2', alignItems: 'center' })}>
              <input type="checkbox" checked={form.scopes.has(scope)} onChange={() => toggle(scope)} />
              {intl.formatMessage(scopeLabels[scope])}
            </label>
          ))}
        </fieldset>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.cgExpires)}</Field.Label>
          <Field.Input type="date" value={form.expires} onChange={set('expires')} />
        </Field.Root>
        <Field.Root required>
          <Field.Label>{intl.formatMessage(messages.cgDocument)}</Field.Label>
          <Field.Input value={form.document} onChange={set('document')} autoComplete="off" />
        </Field.Root>
        <button type="submit" className={button} disabled={!complete || record.isPending}>
          {intl.formatMessage(messages.cgRecord)}
        </button>
        {record.isError ? (
          <p role="alert" className={css({ color: 'danger.text' })}>
            {intl.formatMessage(messages.cgRecordFailed)} {record.error instanceof Error && 'reason' in record.error ? String(record.error.reason) : ''}
          </p>
        ) : null}
      </form>
    </section>
  )
}
