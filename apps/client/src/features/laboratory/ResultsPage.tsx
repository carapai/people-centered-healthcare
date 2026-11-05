import { css } from '@health/design-system/css'
import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useIntl } from 'react-intl'

import { labMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { patientName } from '../reception/api'
import { button } from '../reception/styles'
import { worklistQuery, worklistQueryKey } from '../worklists/api'
import { getTest, takeStep, testKey, testOf } from './api'
import { Results } from './LaboratoryPage'

/**
 * The clinician's results to review (#109): verified results only, a critical one marked urgent.
 * Reviewing one closes it.
 */
export function ResultsPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const [done, setDone] = useState('')
  const tasks = useQuery(worklistQuery(platform.transport, 'clinician-results'))
  const tests = useQueries({ queries: (tasks.data ?? []).map((t) => ({ queryKey: testKey(testOf(t.context)), queryFn: () => getTest(platform.transport, testOf(t.context)) })) })
  const names = useQueries({
    queries: tests.map((t) => ({
      queryKey: ['fhir', 'Patient', t.data?.patient ?? ''],
      queryFn: () => platform.fhir.read('Patient', (t.data?.patient ?? '').replace(/^Patient\//, '')),
      enabled: t.data !== undefined,
      staleTime: 300_000,
    })),
  })
  const review = useMutation({
    mutationFn: (id: string) => takeStep(platform.transport, id, { step: 'review' }),
    onSuccess: () => setDone(intl.formatMessage(m.rsReviewed)),
    onSettled: () => queryClient.invalidateQueries({ queryKey: worklistQueryKey('clinician-results') }),
  })
  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '4xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(m.rsTitle)}</h1>
      {done === '' ? null : <p role="status">{done}</p>}
      {tasks.data?.length === 0 ? <p>{intl.formatMessage(m.rsNone)}</p> : null}
      <ol aria-label={intl.formatMessage(m.rsTitle)} className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
        {(tasks.data ?? []).map((task, i) => {
          const test = tests[i]?.data
          if (test === undefined) return null
          return (
            <li key={task.id} className={css({ borderBottom: '1px solid', borderColor: 'border', pb: '2' })}>
              <p className={css({ fontWeight: 'semibold' })}>{intl.formatMessage(m.rsFor, { test: test.test.text, name: patientName(names[i]?.data) || test.patient })}</p>
              {task.priority === 'stat' ? <p className={css({ color: 'danger.text', fontWeight: 'semibold' })}>{intl.formatMessage(m.rsUrgent)}</p> : null}
              <Results test={test} />
              <button type="button" className={button} disabled={review.isPending} onClick={() => review.mutate(test.id)}>
                {intl.formatMessage(m.rsReview, { test: test.test.text })}
              </button>
            </li>
          )
        })}
      </ol>
    </section>
  )
}
