import { DataTable, emptyView, type DataTableColumn, type DataTableView } from '@health/design-system'
import { css } from '@health/design-system/css'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useId, useState, type FormEvent } from 'react'
import { useIntl, type IntlShape } from 'react-intl'

import { equipmentMessages as m } from '../../i18n/messages'
import { usePlatform } from '../../platform'
import { button, control, field } from '../reception/styles'
import {
  EQUIPMENT_STATUSES,
  StockError,
  dispatchEquipment,
  equipmentKey,
  equipmentQuery,
  moveEquipment,
  receiveEquipment,
  setEquipmentStatus,
  totalBookValue,
  type Equipment,
  type EquipmentStatus,
} from './api'

const statusName = (intl: IntlShape, status: EquipmentStatus) =>
  intl.formatMessage({ available: m.eqAvailable, faulty: m.eqFaulty, 'under-maintenance': m.eqUnderMaintenance, retired: m.eqRetired }[status])

const form = css({ display: 'flex', flexDirection: 'column', gap: '3', p: '3', border: '1px solid', borderColor: 'border', borderRadius: 'md' })

/**
 * The facility's Equipment (ADR 0064; #453): each unit, where it is, its status and its latest
 * change. A unit is marked faulty, under maintenance, retired or available again with a note of
 * why, moved between locations, or sent to another facility, which receives it into one of its
 * own locations. Units arrive by receiving goods of a product tracked as equipment.
 */
export function EquipmentPage() {
  const intl = useIntl()
  const platform = usePlatform()
  const queryClient = useQueryClient()
  const equipment = useQuery(equipmentQuery(platform.transport))
  const [view, setView] = useState<DataTableView>(emptyView)
  const [chosen, setChosen] = useState('')
  const [status, setStatus] = useState<EquipmentStatus>('faulty')
  const [note, setNote] = useState('')
  const [location, setLocation] = useState('')
  const [facility, setFacility] = useState('')
  const [done, setDone] = useState('')
  const ids = { unit: useId(), status: useId(), note: useId(), noteHint: useId(), location: useId(), facility: useId(), facilityHint: useId() }
  const unit = equipment.data?.find((u) => u.id === chosen)
  const total = equipment.data === undefined ? null : totalBookValue(equipment.data)
  const statuses = EQUIPMENT_STATUSES.filter((s) => s !== unit?.status)
  const next = statuses.includes(status) ? status : (statuses[0] ?? 'available')

  const change = useMutation({
    mutationFn: (apply: () => Promise<Equipment>) => apply(),
    onSuccess: (changed) => {
      setDone(intl.formatMessage(m.eqDone, { name: changed.name, serial: changed.serial }))
      setNote('')
      setLocation('')
      setFacility('')
      void queryClient.invalidateQueries({ queryKey: equipmentKey })
    },
  })
  const submit = (apply: () => Promise<Equipment>) => (event: FormEvent) => {
    event.preventDefault()
    setDone('')
    change.mutate(apply)
  }

  const columns: DataTableColumn<Equipment>[] = [
    { id: 'name', header: intl.formatMessage(m.eqName), cell: (u) => u.name, sortValue: (u) => u.name },
    { id: 'serial', header: intl.formatMessage(m.eqSerial), cell: (u) => u.serial, sortValue: (u) => u.serial },
    { id: 'location', header: intl.formatMessage(m.eqLocation), cell: (u) => u.location, sortValue: (u) => u.location },
    {
      id: 'status',
      header: intl.formatMessage(m.eqStatus),
      cell: (u) => (u.transfer === null ? statusName(intl, u.status) : intl.formatMessage(m.eqInTransit, { from: u.transfer.from })),
      sortValue: (u) => u.status,
    },
    { id: 'cost', header: intl.formatMessage(m.eqCost), cell: (u) => u.cost ?? '' },
    { id: 'bookValue', header: intl.formatMessage(m.eqBookValue), cell: (u) => u.bookValue ?? '' },
    {
      id: 'change',
      header: intl.formatMessage(m.eqLastChange),
      cell: (u) => (u.change === null ? '' : intl.formatMessage(m.eqChange, { note: u.change.note, by: u.change.by, at: u.change.at.slice(0, 10) })),
    },
  ]

  return (
    <section className={css({ display: 'flex', flexDirection: 'column', gap: '4', maxWidth: '5xl' })}>
      <h1 className={css({ fontSize: '2xl', fontWeight: 'semibold' })}>{intl.formatMessage(m.eqTitle)}</h1>
      <p>{intl.formatMessage(m.eqIntro)}</p>
      {equipment.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.eqFailed)}
        </p>
      ) : equipment.data?.length === 0 ? (
        <p role="status">{intl.formatMessage(m.eqNone)}</p>
      ) : equipment.data === undefined ? null : (
        <>
          {total === null ? null : <p>{intl.formatMessage(m.eqTotal, { value: total })}</p>}
          <DataTable caption={intl.formatMessage(m.eqCaption)} columns={columns} data={equipment.data} getRowId={(u) => u.id} view={view} onViewChange={setView} height="18rem" onRowActivate={(u) => setChosen(u.id)} />
          <div className={field}>
            <label htmlFor={ids.unit}>{intl.formatMessage(m.eqUnit)}</label>
            <select id={ids.unit} value={chosen} onChange={(event) => setChosen(event.target.value)} className={control}>
              <option value="" />
              {equipment.data.map((u) => (
                <option key={u.id} value={u.id}>
                  {intl.formatMessage(m.eqSelected, { name: u.name, serial: u.serial })}
                </option>
              ))}
            </select>
          </div>
        </>
      )}
      {unit === undefined ? null : (
        <div className={css({ display: 'flex', flexDirection: 'column', gap: '3' })}>
          <h2 className={css({ fontSize: 'lg', fontWeight: 'semibold' })}>{intl.formatMessage(m.eqSelected, { name: unit.name, serial: unit.serial })}</h2>
          {unit.transfer !== null ? (
            <form className={form} onSubmit={submit(() => receiveEquipment(platform.transport, unit.id, location.trim()))}>
              <p>{intl.formatMessage(m.eqInTransit, { from: unit.transfer.from })}</p>
              <div className={field}>
                <label htmlFor={ids.location}>{intl.formatMessage(m.eqReceiveInto)}</label>
                <input id={ids.location} value={location} onChange={(event) => setLocation(event.target.value)} className={control} autoComplete="off" />
              </div>
              <button type="submit" className={button} disabled={location.trim() === '' || change.isPending}>
                {intl.formatMessage(m.eqReceive)}
              </button>
            </form>
          ) : unit.status === 'retired' ? (
            <p>{intl.formatMessage(m.eqRetired)}</p>
          ) : (
            <>
              <form className={form} onSubmit={submit(() => setEquipmentStatus(platform.transport, unit.id, next, note.trim()))}>
                <div className={field}>
                  <label htmlFor={ids.status}>{intl.formatMessage(m.eqStatusField)}</label>
                  <select id={ids.status} value={next} onChange={(event) => setStatus(event.target.value as EquipmentStatus)} className={control}>
                    {statuses.map((s) => (
                      <option key={s} value={s}>
                        {statusName(intl, s)}
                      </option>
                    ))}
                  </select>
                </div>
                <div className={field}>
                  <label htmlFor={ids.note}>{intl.formatMessage(m.eqNote)}</label>
                  <input id={ids.note} aria-describedby={ids.noteHint} value={note} onChange={(event) => setNote(event.target.value)} className={control} autoComplete="off" />
                  <span id={ids.noteHint}>{intl.formatMessage(m.eqNoteHint)}</span>
                </div>
                <button type="submit" className={button} disabled={(next !== 'available' && note.trim() === '') || change.isPending}>
                  {intl.formatMessage(m.eqSetStatus)}
                </button>
              </form>
              <form className={form} onSubmit={submit(() => moveEquipment(platform.transport, unit.id, location.trim()))}>
                <div className={field}>
                  <label htmlFor={ids.location}>{intl.formatMessage(m.eqNewLocation)}</label>
                  <input id={ids.location} value={location} onChange={(event) => setLocation(event.target.value)} className={control} autoComplete="off" />
                </div>
                <button type="submit" className={button} disabled={location.trim() === '' || change.isPending}>
                  {intl.formatMessage(m.eqMove)}
                </button>
              </form>
              <form className={form} onSubmit={submit(() => dispatchEquipment(platform.transport, unit.id, facility.trim()))}>
                <div className={field}>
                  <label htmlFor={ids.facility}>{intl.formatMessage(m.eqFacility)}</label>
                  <input id={ids.facility} aria-describedby={ids.facilityHint} value={facility} onChange={(event) => setFacility(event.target.value)} className={control} autoComplete="off" />
                  <span id={ids.facilityHint}>{intl.formatMessage(m.eqFacilityHint)}</span>
                </div>
                <button type="submit" className={button} disabled={facility.trim() === '' || change.isPending}>
                  {intl.formatMessage(m.eqDispatch)}
                </button>
              </form>
            </>
          )}
        </div>
      )}
      {done === '' ? null : <p role="status">{done}</p>}
      {change.isError ? (
        <p role="alert" className={css({ color: 'danger.text' })}>
          {intl.formatMessage(m.eqRefused, { reason: change.error instanceof StockError ? change.error.reason : '' })}
        </p>
      ) : null}
    </section>
  )
}
