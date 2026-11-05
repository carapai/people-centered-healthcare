import { css } from '@health/design-system/css'
import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { useIntl } from 'react-intl'

import { labMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { newResourceId } from '../../platform/ids'
import { patientName } from '../reception/api'
import { button, control, field } from '../reception/styles'
import { worklistQuery, worklistQueryKey } from '../worklists/api'
import { LabError, getTest, takeStep, testKey, testOf, type LabTest, type Step } from './api'

const LISTS = ['lab-collect', 'lab-result', 'lab-verify'] as const

/** A range from its two fields; none when both are blank. */
const range = (low: string, high: string) =>
  low.trim() === '' && high.trim() === '' ? null : { ...(low.trim() === '' ? {} : { low: low.trim() }), ...(high.trim() === '' ? {} : { high: high.trim() }) }

/** The test's results, each with its flag. */
export function Results({ test }: { test: LabTest }) {
  const intl = useIntl()
  return (
    <ul>
      {test.results.map((r) => {
        const value =
          r.entry.value.kind === 'quantity' ? `${r.entry.value.value ?? ''} ${r.entry.value.unit ?? ''}`.trim() : (r.entry.value.concept?.text ?? r.entry.value.text ?? '')
        const flag = intl.formatMessage(m.lbFlag, { flag: r.flag.replaceAll('-', '') })
        return (
          <li key={r.entry.code.text} className={css(r.flag.startsWith('critical') ? { color: 'danger.text', fontWeight: 'semibold' } : {})}>
            {intl.formatMessage(m.lbResult, { name: r.entry.code.text, value, flag })}
          </li>
        )
      })}
    </ul>
  )
}

/** The step a test at the lab waits for, with its form. */
function Bench({ test, onDone }: { test: LabTest; onDone: (message: string) => void }) {
  const intl = useIntl()
  const platform = usePlatform()
  const ids = { specimen: useId(), value: useId(), unit: useId(), low: useId(), high: useId(), criticalLow: useId(), criticalHigh: useId(), reason: useId() }
  const [specimen, setSpecimen] = useState('Venous blood')
  const [entry, setEntry] = useState({ value: '', unit: '', low: '', high: '', criticalLow: '', criticalHigh: '' })
  const [reason, setReason] = useState('')
  const step = useMutation({
    mutationFn: ({ next }: { next: Step; done: string }) => takeStep(platform.transport, test.id, next),
    onSuccess: (_, { done }) => onDone(done),
  })
  const input = (id: string, label: string, key: keyof typeof entry) => (
    <div className={field}>
      <label htmlFor={id}>{label}</label>
      <input id={id} value={entry[key]} onChange={(e) => setEntry({ ...entry, [key]: e.target.value })} className={control} />
    </div>
  )
  const failure = step.error instanceof LabError ? step.error.reason || String(step.error.status) : ''
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '3', borderTop: '1px solid', borderColor: 'border', pt: '3' })}>
      {test.stage === 'ordered' ? (
        <>
          <div className={field}>
            <label htmlFor={ids.specimen}>{intl.formatMessage(m.lbSpecimen)}</label>
            <input id={ids.specimen} value={specimen} onChange={(e) => setSpecimen(e.target.value)} className={control} />
          </div>
          <div>
            <button
              type="button"
              className={button}
              disabled={step.isPending || specimen.trim() === ''}
              onClick={() => step.mutate({ next: { step: 'collect', specimen: newResourceId(), kind: specimen.trim() }, done: intl.formatMessage(m.lbCollected) })}
            >
              {intl.formatMessage(m.lbCollect)}
            </button>
          </div>
        </>
      ) : null}
      {test.stage === 'collected' ? (
        <>
          {input(ids.value, intl.formatMessage(m.lbValue), 'value')}
          {input(ids.unit, intl.formatMessage(m.lbUnit), 'unit')}
          {input(ids.low, intl.formatMessage(m.lbReferenceLow), 'low')}
          {input(ids.high, intl.formatMessage(m.lbReferenceHigh), 'high')}
          {input(ids.criticalLow, intl.formatMessage(m.lbCriticalLow), 'criticalLow')}
          {input(ids.criticalHigh, intl.formatMessage(m.lbCriticalHigh), 'criticalHigh')}
          <div>
            <button
              type="button"
              className={button}
              disabled={step.isPending || entry.value.trim() === ''}
              onClick={() =>
                step.mutate({
                  next: {
                    step: 'result',
                    results: [
                      {
                        code: { text: test.test.text },
                        value: { kind: 'quantity', value: entry.value.trim(), unit: entry.unit.trim() },
                        reference: range(entry.low, entry.high),
                        critical: range(entry.criticalLow, entry.criticalHigh),
                      },
                    ],
                  },
                  done: intl.formatMessage(m.lbResulted),
                })
              }
            >
              {intl.formatMessage(m.lbSaveResults)}
            </button>
          </div>
          <div className={field}>
            <label htmlFor={ids.reason}>{intl.formatMessage(m.lbRejectReason)}</label>
            <input id={ids.reason} value={reason} onChange={(e) => setReason(e.target.value)} className={control} />
          </div>
          <div>
            <button
              type="button"
              className={button}
              disabled={step.isPending || reason.trim() === ''}
              onClick={() => step.mutate({ next: { step: 'reject', reason: reason.trim() }, done: intl.formatMessage(m.lbRejected) })}
            >
              {intl.formatMessage(m.lbReject)}
            </button>
          </div>
        </>
      ) : null}
      {test.stage === 'resulted' ? (
        <>
          <Results test={test} />
          <div>
            <button type="button" className={button} disabled={step.isPending} onClick={() => step.mutate({ next: { step: 'verify' }, done: intl.formatMessage(m.lbVerified) })}>
              {intl.formatMessage(m.lbVerify)}
            </button>
          </div>
        </>
      ) : null}
      {step.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.lbFailed, { reason: failure })}
        </p>
      ) : null}
    </section>
  )
}

/**
 * The laboratory (#109): its worklists (specimens to collect, results to enter, results to
 * verify), each task a test. Results are verified by someone other than who entered them, and
 * only then released to the clinician.
 */
export function LaboratoryPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const listId = useId()
  const [list, setList] = useState<(typeof LISTS)[number]>('lab-collect')
  const [open, setOpen] = useState<string | undefined>()
  const [done, setDone] = useState('')
  const tasks = useQuery(worklistQuery(platform.transport, list))
  const tests = useQueries({ queries: (tasks.data ?? []).map((t) => ({ queryKey: testKey(testOf(t.context)), queryFn: () => getTest(platform.transport, testOf(t.context)) })) })
  const names = useQueries({
    queries: tests.map((t) => ({
      queryKey: ['fhir', 'Patient', t.data?.patient ?? ''],
      queryFn: () => platform.fhir.read('Patient', (t.data?.patient ?? '').replace(/^Patient\//, '')),
      enabled: t.data !== undefined,
      staleTime: 300_000,
    })),
  })
  const opened = tests.find((t) => t.data?.id === open)?.data
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '4xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(m.lbTitle)}</h1>
      <div className={field}>
        <label htmlFor={listId}>{intl.formatMessage(m.lbWorklist)}</label>
        <select
          id={listId}
          value={list}
          onChange={(e) => {
            setList(e.target.value as (typeof LISTS)[number])
            setOpen(undefined)
          }}
          className={control}
        >
          {LISTS.map((l) => (
            <option key={l} value={l}>
              {intl.formatMessage(m.lbList, { list: l.replaceAll('-', '') })}
            </option>
          ))}
        </select>
      </div>
      {done === '' ? null : <p role="status">{done}</p>}
      {tasks.data?.length === 0 ? <p>{intl.formatMessage(m.lbNone)}</p> : null}
      <ol aria-label={intl.formatMessage(m.lbList, { list: list.replaceAll('-', '') })} className={css({ display: 'flex', flexDirection: 'column', gap: '2' })}>
        {tests.map((t, i) =>
          t.data === undefined ? null : (
            <li key={t.data.id}>
              <button
                type="button"
                className={button}
                onClick={() => {
                  setOpen(t.data.id)
                  setDone('')
                }}
              >
                {intl.formatMessage(m.lbOpen, { test: t.data.test.text, name: patientName(names[i]?.data) || t.data.patient })}
              </button>
            </li>
          ),
        )}
      </ol>
      {opened === undefined ? null : (
        <Bench
          key={`${opened.id}-${opened.stage}`}
          test={opened}
          onDone={(message) => {
            setDone(message)
            setOpen(undefined)
            void queryClient.invalidateQueries({ queryKey: testKey(opened.id) })
            void queryClient.invalidateQueries({ queryKey: worklistQueryKey(list) })
          }}
        />
      )}
    </section>
  )
}
