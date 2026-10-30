import { createListCollection } from '@ark-ui/react/collection'
import { parseDate } from '@internationalized/date'
import { useEffect, useMemo, type ReactNode } from 'react'

import { css } from '../styled-system/css'
import {
  Combobox,
  DatePicker,
  Dialog,
  Field,
  I18nProvider,
  Menu,
  Select,
  Toaster,
  createToaster,
  englishMessages,
} from '../src'

// The design system's example page (#45): every slot recipe, in every theme and density, with
// the overlays open and rendered in place so each one is styled by its section's theme.

export const themes = ['light', 'dark', 'high-contrast'] as const
export const densities = ['comfortable', 'compact'] as const

const wards = createListCollection({
  items: [
    { label: 'Outpatients', value: 'opd' },
    { label: 'Maternity', value: 'maternity' },
    { label: 'Paediatrics', value: 'paediatrics' },
    { label: 'Theatre', value: 'theatre', disabled: true },
  ],
})

const facilities = createListCollection({
  items: [
    { label: 'Mulago National Referral Hospital', value: 'mulago' },
    { label: 'Kawempe Health Centre IV', value: 'kawempe' },
    { label: 'Kiruddu Referral Hospital', value: 'kiruddu' },
  ],
})

// Open popups stay below their triggers whatever the viewport.
const noFlip = { flip: false }

function Card({ title, wide = false, children }: { title: string; wide?: boolean; children: ReactNode }) {
  return (
    <div className={css({ display: 'flex', flexDirection: 'column', gap: '3', minWidth: '0', gridColumn: wide ? 'span 2' : undefined })}>
      <h3 className={css({ fontSize: 'sm', fontWeight: 'semibold', color: 'fg.muted' })}>{title}</h3>
      {children}
    </div>
  )
}

function Section({ theme, density }: { theme: (typeof themes)[number]; density: (typeof densities)[number] }) {
  const id = `${theme}-${density}`
  const toaster = useMemo(() => createToaster({ placement: 'bottom-end', overlap: false, gap: 8, duration: Infinity }), [])
  useEffect(() => {
    toaster.create({ title: 'Encounter saved', description: 'Synced to the facility server.', type: 'success' })
    toaster.create({ title: 'Sync failed', description: 'Retrying in 30 seconds.', type: 'error', action: { label: 'Retry now', onClick: () => {} } })
    return () => toaster.remove()
  }, [toaster])

  return (
    <section
      aria-labelledby={id}
      data-theme={theme}
      data-density={density}
      data-testid={id}
      className={css({ bg: 'bg.canvas', color: 'fg.default', p: 'surface', display: 'flex', flexDirection: 'column', gap: '6' })}
    >
      <h2 id={id} className={css({ fontSize: 'xl', fontWeight: 'bold' })}>
        {theme}, {density}
      </h2>
      <div className={css({ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(22rem, 1fr))', gap: '8', alignItems: 'start' })}>
        <Card title="Field">
          <Field.Root required>
            <Field.Label>
              Family name <Field.RequiredIndicator />
            </Field.Label>
            <Field.Input defaultValue="Akello" />
            <Field.HelperText>As written on the national ID.</Field.HelperText>
          </Field.Root>
          <Field.Root invalid>
            <Field.Label>Phone number</Field.Label>
            <Field.Input defaultValue="0772" />
            <Field.ErrorText>Enter all ten digits.</Field.ErrorText>
          </Field.Root>
          <Field.Root disabled>
            <Field.Label>Patient number</Field.Label>
            <Field.Input defaultValue="Assigned on save" />
          </Field.Root>
          <Field.Root>
            <Field.Label>Notes</Field.Label>
            <Field.Textarea defaultValue="Referred from Kawempe." />
          </Field.Root>
        </Card>

        <Card title="Select">
          <div className={css({ minHeight: '22rem' })}>
            <Select.Root collection={wards} defaultValue={['maternity']} open positioning={noFlip}>
              <Select.Label>Ward</Select.Label>
              <Select.Control>
                <Select.Trigger>
                  <Select.ValueText placeholder="Choose a ward" />
                  <Select.Indicator />
                </Select.Trigger>
              </Select.Control>
              <Select.Positioner>
                <Select.Content>
                  {wards.items.map((ward) => (
                    <Select.Item key={ward.value} item={ward}>
                      <Select.ItemText>{ward.label}</Select.ItemText>
                      <Select.ItemIndicator />
                    </Select.Item>
                  ))}
                </Select.Content>
              </Select.Positioner>
              <Select.HiddenSelect />
            </Select.Root>
          </div>
        </Card>

        <Card title="Combobox">
          <div className={css({ minHeight: '18rem' })}>
            <Combobox.Root collection={facilities} open positioning={noFlip}>
              <Combobox.Label>Referring facility</Combobox.Label>
              <Combobox.Control>
                <Combobox.Input placeholder="Search facilities" />
                <Combobox.ClearTrigger />
                <Combobox.Trigger />
              </Combobox.Control>
              <Combobox.Positioner>
                <Combobox.Content>
                  {facilities.items.map((facility) => (
                    <Combobox.Item key={facility.value} item={facility}>
                      <Combobox.ItemText>{facility.label}</Combobox.ItemText>
                      <Combobox.ItemIndicator />
                    </Combobox.Item>
                  ))}
                </Combobox.Content>
              </Combobox.Positioner>
            </Combobox.Root>
          </div>
        </Card>

        <Card title="Menu">
          <div className={css({ minHeight: '16rem' })}>
            <Menu.Root open positioning={noFlip}>
              <Menu.Trigger className={css({ textDecoration: 'underline', color: 'accent.text', cursor: 'pointer' })}>Encounter actions</Menu.Trigger>
              <Menu.Positioner>
                <Menu.Content>
                  <Menu.ItemGroup>
                    <Menu.ItemGroupLabel>Encounter</Menu.ItemGroupLabel>
                    <Menu.Item value="vitals">Record vitals</Menu.Item>
                    <Menu.Item value="refer">Refer</Menu.Item>
                  </Menu.ItemGroup>
                  <Menu.Separator />
                  <Menu.Item value="discharge" disabled>
                    Discharge
                  </Menu.Item>
                </Menu.Content>
              </Menu.Positioner>
            </Menu.Root>
          </div>
        </Card>

        <Card title="Date Picker">
          <DatePicker.Root defaultValue={[parseDate('2026-10-30')]}>
            <DatePicker.Label>Date of birth</DatePicker.Label>
            <DatePicker.Control>
              <DatePicker.Input />
              <DatePicker.ClearTrigger />
              <DatePicker.Trigger />
            </DatePicker.Control>
          </DatePicker.Root>
          <DatePicker.Root inline defaultValue={[parseDate('2026-10-30')]} defaultFocusedValue={parseDate('2026-10-30')}>
            <DatePicker.Content>
              <DatePicker.Calendar />
            </DatePicker.Content>
          </DatePicker.Root>
        </Card>

        <Card title="Dialog" wide>
          <div className={css({ position: 'relative', minHeight: '16rem', overflow: 'hidden', borderRadius: 'lg' })}>
            <Dialog.Root open modal={false} trapFocus={false} preventScroll={false} closeOnInteractOutside={false}>
              <Dialog.Backdrop className={css({ position: 'absolute' })} />
              <Dialog.Positioner className={css({ position: 'absolute' })}>
                <Dialog.Content>
                  <Dialog.Title>Discard this encounter?</Dialog.Title>
                  <Dialog.Description>The vitals recorded so far are not saved.</Dialog.Description>
                  <Dialog.CloseTrigger />
                </Dialog.Content>
              </Dialog.Positioner>
            </Dialog.Root>
          </div>
        </Card>

        <Card title="Toast" wide>
          <div className={css({ position: 'relative', minHeight: '14rem' })}>
            <Toaster toaster={toaster} className={css({ position: 'absolute!', inset: '0!', translate: 'none!' })} />
          </div>
        </Card>
      </div>
    </section>
  )
}

export function Gallery() {
  return (
    <I18nProvider locale="en" messages={englishMessages}>
      <main>
        <h1 className={css({ srOnly: true })}>Design system example</h1>
        {themes.flatMap((theme) => densities.map((density) => <Section key={`${theme}-${density}`} theme={theme} density={density} />))}
      </main>
    </I18nProvider>
  )
}
