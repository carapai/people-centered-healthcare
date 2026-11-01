import { css } from '@health/design-system/css'
import { useQuery } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { useIntl } from 'react-intl'

import { edgeStatusMessages } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { EdgeStatusError, statusQuery, usedPercent } from './api'
import type { EdgeReport } from './api'

type Messages = typeof edgeStatusMessages

/** Alerts without a subject. */
const alertLabels: Record<string, Messages['esAlertBackupsOff']> = {
  'backups-off': edgeStatusMessages.esAlertBackupsOff,
  'backup-failed': edgeStatusMessages.esAlertBackupFailed,
  'backup-stale': edgeStatusMessages.esAlertBackupStale,
  'backup-test-failed': edgeStatusMessages.esAlertBackupTestFailed,
  'ups-on-battery': edgeStatusMessages.esAlertOnBattery,
  'ups-low-battery': edgeStatusMessages.esAlertLowBattery,
  'ups-unreachable': edgeStatusMessages.esAlertUpsUnreachable,
}

/** Alerts about a volume or a task. */
const subjectAlertLabels: Record<string, Messages['esAlertDiskFull']> = {
  'disk-full': edgeStatusMessages.esAlertDiskFull,
  'task-restarts': edgeStatusMessages.esAlertTaskRestarts,
}

const powerLabels: Record<string, Messages['esPowerOnline']> = {
  online: edgeStatusMessages.esPowerOnline,
  'on-battery': edgeStatusMessages.esPowerOnBattery,
  'low-battery': edgeStatusMessages.esPowerLowBattery,
  unreachable: edgeStatusMessages.esPowerUnreachable,
}

const page = css({ display: 'flex', flexDirection: 'column', gap: '6' })
const heading = css({ fontSize: '2xl', fontWeight: 'semibold' })
const section = css({ display: 'flex', flexDirection: 'column', gap: '2' })
const subheading = css({ fontSize: 'lg', fontWeight: 'semibold' })
const facts = css({ display: 'grid', gridTemplateColumns: 'max-content 1fr', columnGap: '6', rowGap: '1' })
const term = css({ color: 'fg.muted' })

/**
 * The edge admin status page (facility-edge.md § Health monitoring): the node's indicators and
 * the alerts they raise, from the edge itself, so they are visible with central unreachable.
 */
export function StatusPage() {
  const intl = useIntl()
  const { transport } = usePlatform()
  const status = useQuery(statusQuery(transport))
  return (
    <section className={page}>
      <h1 className={heading}>{intl.formatMessage(edgeStatusMessages.esTitle)}</h1>
      {status.data === undefined ? <Pending loading={status.isPending} error={status.error} /> : <Report report={status.data} />}
    </section>
  )
}

function Pending({ loading, error }: { loading: boolean; error: unknown }) {
  const intl = useIntl()
  if (loading) return <p role="status">{intl.formatMessage(edgeStatusMessages.esLoading)}</p>
  const status = error instanceof EdgeStatusError ? error.status : 0
  const message = status === 403 ? edgeStatusMessages.esForbidden : status === 404 ? edgeStatusMessages.esNotEdge : edgeStatusMessages.esFailed
  return (
    <p role="alert" className={css({ color: status === 403 || status === 404 ? 'fg.muted' : 'danger.text' })}>
      {intl.formatMessage(message)}
    </p>
  )
}

function Report({ report }: { report: EdgeReport }) {
  const intl = useIntl()
  const time = (iso: string | null) => (iso === null ? intl.formatMessage(edgeStatusMessages.esNever) : intl.formatDate(new Date(iso), { dateStyle: 'medium', timeStyle: 'short' }))
  const bytes = (n: number | null) => (n === null ? intl.formatMessage(edgeStatusMessages.esUnknown) : formatBytes(intl, n))
  return (
    <>
      <section className={section} aria-labelledby="edge-alerts">
        <h2 id="edge-alerts" className={subheading}>
          {intl.formatMessage(edgeStatusMessages.esAlerts)}
        </h2>
        {report.alerts.length === 0 ? (
          <p role="status">{intl.formatMessage(edgeStatusMessages.esNoAlerts)}</p>
        ) : (
          <div role="alert">
            <ul className={css({ display: 'flex', flexDirection: 'column', gap: '1', color: 'danger.text' })}>
              {report.alerts.map((a) => (
                <li key={`${a.code}:${a.subject ?? ''}`}>{alertText(intl, a)}</li>
              ))}
            </ul>
          </div>
        )}
      </section>
      <Facts id="edge-node" title={intl.formatMessage(edgeStatusMessages.esNode)}>
        <Fact label={intl.formatMessage(edgeStatusMessages.esNodeId)}>{report.node.id}</Fact>
        <Fact label={intl.formatMessage(edgeStatusMessages.esVersion)}>{report.node.version}</Fact>
        <Fact label={intl.formatMessage(edgeStatusMessages.esStarted)}>{time(report.node.started)}</Fact>
        <Fact label={intl.formatMessage(edgeStatusMessages.esChecked)}>{time(report.at)}</Fact>
      </Facts>
      <Facts id="edge-storage" title={intl.formatMessage(edgeStatusMessages.esStorage)}>
        <Fact label={intl.formatMessage(edgeStatusMessages.esStoreSize)}>{bytes(report.store_bytes)}</Fact>
        {report.volumes.map((v) => (
          <Fact key={v.name} label={intl.formatMessage(v.name === 'data' ? edgeStatusMessages.esDataDisk : edgeStatusMessages.esBackupDisk)}>
            {intl.formatMessage(edgeStatusMessages.esVolumeUse, { percent: usedPercent(v), free: formatBytes(intl, v.free_bytes) })}
          </Fact>
        ))}
      </Facts>
      <Facts id="edge-backups" title={intl.formatMessage(edgeStatusMessages.esBackups)}>
        {report.backup === null ? (
          <Fact label={intl.formatMessage(edgeStatusMessages.esBackups)}>{intl.formatMessage(edgeStatusMessages.esBackupsOff)}</Fact>
        ) : (
          <>
            <Fact label={intl.formatMessage(edgeStatusMessages.esLastBackup)}>{time(report.backup.last_backup)}</Fact>
            {report.backup.failed === null ? null : <Fact label={intl.formatMessage(edgeStatusMessages.esLastFailure)}>{time(report.backup.failed)}</Fact>}
            <Fact label={intl.formatMessage(edgeStatusMessages.esLastTest)}>
              {report.backup.last_test === null
                ? intl.formatMessage(edgeStatusMessages.esNever)
                : intl.formatMessage(report.backup.test_passed === true ? edgeStatusMessages.esTestPassed : edgeStatusMessages.esTestFailed, { at: time(report.backup.last_test) })}
            </Fact>
          </>
        )}
      </Facts>
      <Facts id="edge-power" title={intl.formatMessage(edgeStatusMessages.esPower)}>
        <Fact label={intl.formatMessage(edgeStatusMessages.esUps)}>
          {report.ups === null
            ? intl.formatMessage(edgeStatusMessages.esUpsOff)
            : report.ups.power === null
              ? intl.formatMessage(edgeStatusMessages.esUnknown)
              : intl.formatMessage(powerLabels[report.ups.power] ?? edgeStatusMessages.esUnknown)}
        </Fact>
      </Facts>
      <Facts id="edge-telemetry" title={intl.formatMessage(edgeStatusMessages.esTelemetry)}>
        <Fact label={intl.formatMessage(edgeStatusMessages.esBuffered)}>{formatBytes(intl, report.telemetry.buffered_bytes)}</Fact>
        {report.telemetry.dropped_bytes > 0 ? <Fact label={intl.formatMessage(edgeStatusMessages.esDropped)}>{formatBytes(intl, report.telemetry.dropped_bytes)}</Fact> : null}
        <Fact label={intl.formatMessage(edgeStatusMessages.esLastShipped)}>
          {!report.telemetry.shipping
            ? intl.formatMessage(edgeStatusMessages.esShippingOff)
            : report.telemetry.failing
              ? intl.formatMessage(edgeStatusMessages.esShippingFailing, { at: time(report.telemetry.last_shipped) })
              : time(report.telemetry.last_shipped)}
        </Fact>
        {report.tasks.map((t) => (
          <Fact key={t.name} label={intl.formatMessage(edgeStatusMessages.esTaskRestarts, { task: t.name })}>
            {intl.formatNumber(t.recent_failures)}
          </Fact>
        ))}
      </Facts>
      {report.pending.length === 0 ? null : <p className={term}>{intl.formatMessage(edgeStatusMessages.esPending, { indicators: report.pending.join(', ') })}</p>}
    </>
  )
}

function Facts({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <section className={section} aria-labelledby={id}>
      <h2 id={id} className={subheading}>
        {title}
      </h2>
      <dl className={facts}>{children}</dl>
    </section>
  )
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt className={term}>{label}</dt>
      <dd>{children}</dd>
    </>
  )
}

function alertText(intl: ReturnType<typeof useIntl>, alert: { code: string; subject: string | null }): string {
  const plain = alertLabels[alert.code]
  if (plain !== undefined) return intl.formatMessage(plain)
  const about = subjectAlertLabels[alert.code]
  if (about === undefined) return alert.code
  const subject =
    alert.subject === 'data'
      ? intl.formatMessage(edgeStatusMessages.esDataVolume)
      : alert.subject === 'backup'
        ? intl.formatMessage(edgeStatusMessages.esBackupVolume)
        : (alert.subject ?? '')
  return intl.formatMessage(about, { subject })
}

/** `n` bytes in the largest unit that keeps it at least 1. */
export function formatBytes(intl: ReturnType<typeof useIntl>, n: number): string {
  const units = ['byte', 'kilobyte', 'megabyte', 'gigabyte', 'terabyte'] as const
  let value = n
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return intl.formatNumber(value, { style: 'unit', unit: units[unit] ?? 'byte', unitDisplay: 'short', maximumFractionDigits: 1 })
}
