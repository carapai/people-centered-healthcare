import { useState } from 'react'
import { expect, test } from 'vitest'
import { userEvent } from 'vitest/browser'
import { render } from 'vitest-browser-react'

import { patientColumns } from '../example/gallery'
import { patients, type Patient } from '../example/patients'
import { DataTable, I18nProvider, emptyView, englishMessages, viewFromSearch, viewToSearch, type DataTableView } from '../src'

// The DataTable (#46): the ARIA grid keyboard model over a virtualized 10,000-row table, sorting,
// column visibility, and a view that round-trips through URL search parameters.

const tenThousand = patients(10_000)

function Harness({
  data,
  initial = emptyView,
  onView,
  onActivate,
}: {
  data: Patient[]
  initial?: DataTableView
  onView?: (view: DataTableView) => void
  onActivate?: (row: Patient) => void
}) {
  const [view, setView] = useState(initial)
  return (
    <I18nProvider locale="en" messages={englishMessages}>
      <DataTable
        caption="Patients"
        columns={patientColumns}
        data={data}
        getRowId={(p) => p.id}
        view={view}
        onViewChange={(next) => {
          setView(next)
          onView?.(next)
        }}
        onRowActivate={onActivate}
        height="24rem"
      />
    </I18nProvider>
  )
}

function focused(): HTMLElement {
  const element = document.activeElement
  if (!(element instanceof HTMLElement)) throw new Error('nothing focused')
  return element
}

function rowIndexOf(element: HTMLElement): number {
  return Number(element.closest('[role=row]')?.getAttribute('aria-rowindex'))
}

test('keyboard-only navigation reaches every corner of a 10,000-row table', async () => {
  const screen = await render(<Harness data={tenThousand} />)
  const grid = screen.getByRole('grid', { name: 'Patients' })
  await expect.element(grid).toHaveAttribute('aria-rowcount', '10001')
  await expect.element(grid).toHaveAttribute('aria-colcount', '5')

  // One tab stop: Tab from the Columns button lands on the first header cell.
  await screen.getByRole('button', { name: 'Columns' }).click()
  await userEvent.keyboard('{Escape}')
  await userEvent.tab()
  expect(focused().getAttribute('role')).toBe('columnheader')
  expect(focused().textContent).toContain('Name')

  await userEvent.keyboard('{ArrowDown}')
  expect(rowIndexOf(focused())).toBe(2)
  expect(focused().textContent).toBe(tenThousand[0]?.name)

  await userEvent.keyboard('{ArrowRight}{ArrowRight}')
  expect(focused().getAttribute('aria-colindex')).toBe('3')
  expect(focused().textContent).toBe(String(tenThousand[0]?.age))

  await userEvent.keyboard('{PageDown}')
  const afterPage = rowIndexOf(focused())
  expect(afterPage).toBeGreaterThan(5)

  await userEvent.keyboard('{Control>}{End}{/Control}')
  expect(rowIndexOf(focused())).toBe(10_001)
  expect(focused().getAttribute('aria-colindex')).toBe('5')
  expect(focused().textContent).toBe(tenThousand[9_999]?.phone)

  await userEvent.keyboard('{Home}')
  expect(focused().textContent).toBe(tenThousand[9_999]?.name)
  await userEvent.keyboard('{ArrowDown}')
  expect(rowIndexOf(focused())).toBe(10_001)

  await userEvent.keyboard('{PageUp}{ArrowUp}')
  expect(rowIndexOf(focused())).toBeLessThan(10_001)

  await userEvent.keyboard('{Control>}{Home}{/Control}')
  expect(rowIndexOf(focused())).toBe(1)
  expect(focused().getAttribute('aria-colindex')).toBe('1')

  // Virtualized: a few dozen rows are in the DOM, not 10,000.
  expect(document.querySelectorAll('[data-body-row]').length).toBeLessThan(80)
  // Exactly one cell is tabbable.
  expect(document.querySelectorAll('[role=grid] [tabindex="0"]').length).toBe(1)
})

test('the focused row stays in the DOM when the grid is scrolled away from it', async () => {
  const screen = await render(<Harness data={tenThousand} />)
  await screen.getByRole('columnheader', { name: /Name/ }).click()
  await userEvent.keyboard('{ArrowDown}')
  const grid = document.querySelector<HTMLElement>('.dataTable__grid')
  if (grid === null) throw new Error('no grid')
  grid.scrollTop = grid.scrollHeight / 2
  grid.dispatchEvent(new Event('scroll'))
  await expect.poll(() => document.querySelector('[aria-rowindex="2"]')).not.toBeNull()
  expect(document.querySelectorAll('[role=grid] [tabindex="0"]').length).toBe(1)
})

test('Enter on a header sorts ascending, descending, then not at all, and announces it', async () => {
  const views: DataTableView[] = []
  const rows = patients(50)
  const ages = rows.map((p) => p.age)
  const screen = await render(<Harness data={rows} onView={(v) => views.push(v)} />)
  const age = screen.getByRole('columnheader', { name: /Age/ })
  await expect.element(age).toHaveAttribute('aria-sort', 'none')
  age.element().focus()

  await userEvent.keyboard('{Enter}')
  await expect.element(age).toHaveAttribute('aria-sort', 'ascending')
  await expect.element(screen.getByRole('status')).toHaveTextContent('Sorted by Age, ascending')
  expect(document.querySelector('[aria-rowindex="2"] [aria-colindex="3"]')?.textContent).toBe(String(Math.min(...ages)))

  await userEvent.keyboard('{Enter}')
  await expect.element(age).toHaveAttribute('aria-sort', 'descending')
  expect(document.querySelector('[aria-rowindex="2"] [aria-colindex="3"]')?.textContent).toBe(String(Math.max(...ages)))

  await userEvent.keyboard(' ')
  await expect.element(age).toHaveAttribute('aria-sort', 'none')
  await expect.element(screen.getByRole('status')).toHaveTextContent('Not sorted')
  expect(views.map((v) => v.sort)).toEqual([[{ id: 'age', desc: false }], [{ id: 'age', desc: true }], []])

  // Phone has no sort value, so it is not sortable.
  await expect.element(screen.getByRole('columnheader', { name: 'Phone' })).not.toHaveAttribute('aria-sort')
})

test('columns are hidden and shown from the Columns menu; the last visible one stays', async () => {
  const views: DataTableView[] = []
  const screen = await render(<Harness data={patients(20)} onView={(v) => views.push(v)} />)
  await screen.getByRole('button', { name: 'Columns' }).click()
  await screen.getByRole('menuitemcheckbox', { name: 'Phone' }).click()
  await expect.element(screen.getByRole('grid')).toHaveAttribute('aria-colcount', '4')
  await expect.element(screen.getByRole('columnheader', { name: 'Phone' })).not.toBeInTheDocument()
  expect(views.at(-1)?.hidden).toEqual(['phone'])

  // Name cannot be hidden, so it is not offered.
  await expect.element(screen.getByRole('menuitemcheckbox', { name: 'Name' })).not.toBeInTheDocument()

  await screen.getByRole('menuitemcheckbox', { name: 'Phone' }).click()
  await expect.element(screen.getByRole('grid')).toHaveAttribute('aria-colcount', '5')
})

test('a view restored from the URL sorts and hides on first render', async () => {
  const ids = patientColumns.map((c) => c.id)
  const sortable = patientColumns.filter((c) => c.sortValue !== undefined).map((c) => c.id)
  const view = viewFromSearch({ sort: '-age', hide: 'village,phone' }, ids, sortable)
  const screen = await render(<Harness data={patients(30)} initial={view} />)
  await expect.element(screen.getByRole('columnheader', { name: /Age/ })).toHaveAttribute('aria-sort', 'descending')
  await expect.element(screen.getByRole('grid')).toHaveAttribute('aria-colcount', '3')
})

test('view and search parameters round-trip, and unknown or unsortable ids are dropped', () => {
  const ids = ['name', 'age', 'phone']
  const sortable = ['name', 'age']
  const view: DataTableView = { sort: [{ id: 'age', desc: true }], hidden: ['phone'] }
  expect(viewToSearch(view)).toEqual({ sort: '-age', hide: 'phone' })
  expect(viewFromSearch(viewToSearch(view), ids, sortable)).toEqual(view)
  expect(viewToSearch(emptyView)).toEqual({})
  expect(viewFromSearch({}, ids, sortable)).toEqual(emptyView)
  expect(viewFromSearch({ sort: 'phone', hide: 'nope,age,age' }, ids, sortable)).toEqual({ sort: [], hidden: ['age'] })
  expect(viewFromSearch({ sort: 'name,-age' }, ids, sortable).sort).toEqual([{ id: 'name', desc: false }])
  // Hiding every column would leave an empty grid.
  expect(viewFromSearch({ hide: 'name,age,phone' }, ids, sortable).hidden).toEqual([])
})

test('Enter on a body cell activates its row', async () => {
  const activated: string[] = []
  const rows = patients(10)
  const screen = await render(<Harness data={rows} onActivate={(p) => activated.push(p.id)} />)
  await screen.getByRole('gridcell', { name: rows[2]?.phone ?? '', exact: true }).click()
  await userEvent.keyboard('{ArrowUp}{Enter}')
  expect(activated).toEqual([rows[1]?.id])
})

test('an empty table says so in a grid cell', async () => {
  const screen = await render(<Harness data={[]} />)
  await expect.element(screen.getByRole('gridcell', { name: 'No rows to show' })).toBeVisible()
  await expect.element(screen.getByRole('grid')).toHaveAttribute('aria-rowcount', '1')
})
