import axe from 'axe-core'
import { expect, test } from 'vitest'
import { render } from 'vitest-browser-react'

import { Gallery, densities, themes } from '../example/gallery'

// The example page is the acceptance surface of #45: every slot recipe in every theme and density,
// with no axe violations against WCAG 2.2 AA.

const recipeParts = [
  'field__input',
  'field__errorText',
  'select__trigger',
  'select__content',
  'combobox__control',
  'combobox__content',
  'menu__content',
  'datePicker__input',
  'datePicker__table',
  'dialog__content',
  'toast__root',
]

const sections = themes.flatMap((theme) => densities.map((density) => `${theme}-${density}`))

async function renderGallery() {
  const screen = await render(<Gallery />)
  // Toasts are created after mount.
  await expect.element(screen.getByText('Sync failed').first()).toBeVisible()
  return screen
}

function section(id: string): HTMLElement {
  const element = document.querySelector<HTMLElement>(`[data-testid="${id}"]`)
  if (element === null) throw new Error(`no section ${id}`)
  return element
}

function style(id: string, selector: string): CSSStyleDeclaration {
  const element = section(id).querySelector(selector)
  if (element === null) throw new Error(`${id}: no ${selector}`)
  return getComputedStyle(element)
}

function canvas(id: string): string {
  return getComputedStyle(section(id)).backgroundColor
}

test('every recipe renders in every theme and density', async () => {
  await renderGallery()
  for (const id of sections) {
    for (const part of recipeParts) {
      expect(section(id).querySelector(`.${part}`), `${id}: ${part}`).not.toBeNull()
    }
  }
})

test('themes and densities change the tokens', async () => {
  await renderGallery()
  expect(canvas('light-comfortable')).toBe('rgb(255, 255, 255)')
  expect(canvas('dark-comfortable')).not.toBe(canvas('light-comfortable'))
  expect(style('high-contrast-comfortable', '.field__input').borderColor).toBe('rgb(0, 0, 0)')
  // 44 px touch targets when comfortable, 32 px when compact.
  expect(style('light-comfortable', '.field__input').height).toBe('44px')
  expect(style('light-compact', '.field__input').height).toBe('32px')
  expect(style('dark-compact', '.select__trigger').height).toBe('32px')
})

test('axe finds no WCAG 2.2 AA violations in any theme or density', async () => {
  await renderGallery()
  const result = await axe.run(document, {
    runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'] },
  })
  const violations = result.violations.map(
    (v) => `${v.id}: ${v.help}\n${v.nodes.map((n) => `  ${n.target.join(' ')}: ${n.failureSummary ?? ''}`).join('\n')}`,
  )
  expect(violations.join('\n\n')).toBe('')
  // Contrast is checked, not skipped: axe could compute it for the text it found.
  expect(result.passes.some((p) => p.id === 'color-contrast')).toBe(true)
})
