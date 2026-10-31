import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useIntl } from 'react-intl'

import { messages } from '../../i18n/messages'
import { usePlatform, worklistKey } from '../../platform'
import { ALERTS, acknowledge, alertsQuery } from './api'
import { GrantTable, formatTime } from './GrantTable'
import { ListState, action, heading, page } from './states'

/**
 * The facility in-charge's break-glass alerts (offline-auth.md § Break-glass): every grant at the
 * facility until acknowledged. A new grant refreshes the list at once through the server's
 * `break-glass-alerts` invalidation hint.
 */
export function AlertsPage() {
  const intl = useIntl()
  const { transport } = usePlatform()
  const queryClient = useQueryClient()
  const alerts = useQuery(alertsQuery(transport))
  const acknowledged = useMutation({
    mutationFn: (id: string) => acknowledge(transport, id),
    onSettled: () => queryClient.invalidateQueries({ queryKey: worklistKey(ALERTS) }),
  })
  return (
    <section className={page}>
      <h1 className={heading}>{intl.formatMessage(messages.bgAlertsTitle)}</h1>
      {alerts.data === undefined ? (
        <ListState loading={alerts.isPending} error={alerts.error} />
      ) : alerts.data.length === 0 ? (
        <p role="status">{intl.formatMessage(messages.bgAlertsEmpty)}</p>
      ) : (
        <GrantTable
          caption={intl.formatMessage(messages.bgAlertsTitle)}
          grants={alerts.data}
          extra={[
            { id: 'until', header: intl.formatMessage(messages.bgColUntil), cell: (g) => formatTime(intl, g.expires_at), sortValue: (g) => g.expires_at },
            {
              id: 'action',
              header: intl.formatMessage(messages.bgColAction),
              cell: (g) => (
                <button type="button" className={action} disabled={acknowledged.isPending} onClick={() => acknowledged.mutate(g.id)}>
                  {intl.formatMessage(messages.bgAcknowledge)}
                </button>
              ),
            },
          ]}
        />
      )}
    </section>
  )
}
