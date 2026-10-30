import { createListCollection } from '@ark-ui/react/collection'
import type { ReactNode } from 'react'
import { expect, test } from 'vitest'
import { userEvent } from 'vitest/browser'
import { render } from 'vitest-browser-react'

import { Combobox, DatePicker, Dialog, Field, I18nProvider, Menu, Select, Toaster, createToaster, englishMessages } from '../src'

// Component tests (testing.md): what each component does for keyboard and assistive-technology
// users, and that its strings come from the ICU catalog.

function inEnglish(children: ReactNode, messages: Record<string, string> = englishMessages) {
  return (
    <I18nProvider locale="en" messages={messages}>
      {children}
    </I18nProvider>
  )
}

const wards = createListCollection({
  items: [
    { label: 'Outpatients', value: 'opd' },
    { label: 'Maternity', value: 'maternity' },
  ],
})

test('a field associates its label, helper and error text with the input', async () => {
  const screen = await render(
    inEnglish(
      <Field.Root invalid required>
        <Field.Label>Phone number</Field.Label>
        <Field.Input />
        <Field.ErrorText>Enter all ten digits.</Field.ErrorText>
      </Field.Root>,
    ),
  )
  const input = screen.getByRole('textbox', { name: 'Phone number' })
  await expect.element(input).toHaveAttribute('aria-invalid', 'true')
  await expect.element(input).toBeRequired()
  await expect.element(input).toHaveAccessibleDescription('Enter all ten digits.')
})

test('a select opens from the keyboard and selects an option', async () => {
  const screen = await render(
    inEnglish(
      <Select.Root collection={wards}>
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
      </Select.Root>,
    ),
  )
  const trigger = screen.getByRole('combobox', { name: 'Ward' })
  trigger.element().focus()
  await userEvent.keyboard('{Enter}')
  await expect.element(screen.getByRole('listbox')).toBeVisible()
  await userEvent.keyboard('{ArrowDown}{Enter}')
  await expect.element(trigger).toHaveTextContent('Maternity')
  await expect.element(trigger).toHaveAttribute('aria-expanded', 'false')

  await trigger.click()
  await screen.getByRole('option', { name: 'Outpatients' }).click()
  await expect.element(trigger).toHaveTextContent('Outpatients')
})

test('a combobox names its buttons from the catalog', async () => {
  const screen = await render(
    inEnglish(
      <Combobox.Root collection={wards}>
        <Combobox.Label>Ward</Combobox.Label>
        <Combobox.Control>
          <Combobox.Input />
          <Combobox.Trigger />
        </Combobox.Control>
        <Combobox.Positioner>
          <Combobox.Content>
            {wards.items.map((ward) => (
              <Combobox.Item key={ward.value} item={ward}>
                <Combobox.ItemText>{ward.label}</Combobox.ItemText>
              </Combobox.Item>
            ))}
          </Combobox.Content>
        </Combobox.Positioner>
      </Combobox.Root>,
    ),
  )
  await screen.getByRole('button', { name: 'Show options' }).click()
  await expect.element(screen.getByRole('option', { name: 'Outpatients' })).toBeVisible()
})

test('a dialog opens, is named by its title, and closes from its close button or Escape', async () => {
  const screen = await render(
    inEnglish(
      <Dialog.Root>
        <Dialog.Trigger>Discard</Dialog.Trigger>
        <Dialog.Backdrop />
        <Dialog.Positioner>
          <Dialog.Content>
            <Dialog.Title>Discard this encounter?</Dialog.Title>
            <Dialog.Description>The vitals recorded so far are not saved.</Dialog.Description>
            <Dialog.CloseTrigger />
          </Dialog.Content>
        </Dialog.Positioner>
      </Dialog.Root>,
    ),
  )
  await screen.getByRole('button', { name: 'Discard' }).click()
  const dialog = screen.getByRole('dialog', { name: 'Discard this encounter?' })
  await expect.element(dialog).toBeVisible()
  await screen.getByRole('button', { name: 'Close' }).click()
  await expect.element(dialog).not.toBeInTheDocument()

  await screen.getByRole('button', { name: 'Discard' }).click()
  await expect.element(dialog).toBeVisible()
  await userEvent.keyboard('{Escape}')
  await expect.element(dialog).not.toBeInTheDocument()
})

test('a menu opens from its trigger and lists its items', async () => {
  const screen = await render(
    inEnglish(
      <Menu.Root>
        <Menu.Trigger>Actions</Menu.Trigger>
        <Menu.Positioner>
          <Menu.Content>
            <Menu.Item value="vitals">Record vitals</Menu.Item>
            <Menu.Item value="refer">Refer</Menu.Item>
          </Menu.Content>
        </Menu.Positioner>
      </Menu.Root>,
    ),
  )
  const trigger = screen.getByRole('button', { name: 'Actions' })
  await trigger.click()
  await expect.element(screen.getByRole('menuitem', { name: 'Record vitals' })).toBeVisible()
  await userEvent.keyboard('{Escape}')
  await expect.element(trigger).toHaveAttribute('aria-expanded', 'false')
  await expect.element(trigger).toHaveFocus()
})

test('toasts appear in a named region with a dismiss button', async () => {
  const toaster = createToaster({ placement: 'bottom-end', duration: Infinity })
  const screen = await render(inEnglish(<Toaster toaster={toaster} />))
  toaster.create({ title: 'Encounter saved', type: 'success' })
  const region = screen.getByRole('region', { name: /Notifications/ })
  await expect.element(region.getByText('Encounter saved')).toBeVisible()
  await region.getByRole('button', { name: 'Dismiss notification' }).click()
  await expect.element(region.getByText('Encounter saved')).not.toBeInTheDocument()
})

test('the date picker opens its calendar and names its controls', async () => {
  const screen = await render(
    inEnglish(
      <DatePicker.Root>
        <DatePicker.Label>Date of birth</DatePicker.Label>
        <DatePicker.Control>
          <DatePicker.Input />
          <DatePicker.Trigger />
        </DatePicker.Control>
        <DatePicker.Positioner>
          <DatePicker.Content>
            <DatePicker.Calendar />
          </DatePicker.Content>
        </DatePicker.Positioner>
      </DatePicker.Root>,
    ),
  )
  await screen.getByRole('button', { name: 'Open calendar' }).click()
  await expect.element(screen.getByRole('button', { name: 'Previous month' })).toBeVisible()
  await expect.element(screen.getByRole('button', { name: 'Next month' })).toBeVisible()
  await expect.element(screen.getByRole('grid')).toBeVisible()
})

test('strings come from the locale catalog', async () => {
  const swahili = { ...englishMessages, 'ds.dialog.close': 'Funga' }
  const screen = await render(
    inEnglish(
      <Dialog.Root open>
        <Dialog.Positioner>
          <Dialog.Content>
            <Dialog.Title>Futa?</Dialog.Title>
            <Dialog.CloseTrigger />
          </Dialog.Content>
        </Dialog.Positioner>
      </Dialog.Root>,
      swahili,
    ),
  )
  await expect.element(screen.getByRole('button', { name: 'Funga' })).toBeVisible()
})
