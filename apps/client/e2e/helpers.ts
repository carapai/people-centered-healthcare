import AxeBuilder from '@axe-core/playwright'
import { expect, type Page } from '@playwright/test'

// Keyboard-first helpers shared by the e2e scenarios.

export async function type(page: Page, label: string | RegExp, text: string) {
  await page.getByLabel(label).focus()
  await page.keyboard.type(text)
}

export async function press(page: Page, name: string | RegExp, role: 'button' | 'link' = 'button') {
  await page.getByRole(role, { name }).focus()
  await page.keyboard.press('Enter')
}

/** Signs a member of staff in with their username and password. */
export async function signIn(page: Page, user: { username: string; password: string }) {
  await page.goto('/')
  await press(page, 'Sign in')
  await page.waitForURL(/\/sign-in\?/)
  await type(page, 'Username', user.username)
  await page.keyboard.press('Tab')
  await page.keyboard.type(user.password)
  await page.keyboard.press('Enter')
  await expect(page.getByRole('status').filter({ hasText: 'Signed in' })).toBeVisible()
}

export async function noViolations(page: Page) {
  const results = await new AxeBuilder({ page }).analyze()
  expect(results.violations.map((v) => `${v.id}: ${v.help} at ${v.nodes.map((n) => n.html).join(' | ')}`)).toEqual([])
}
