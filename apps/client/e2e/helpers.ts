import { readFileSync } from 'node:fs'

import AxeBuilder from '@axe-core/playwright'
import { expect, type Browser, type Page } from '@playwright/test'

import { OTP_OUTBOX } from './users'

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

/** A page of its own browser context: each role keeps its own session. */
export async function newPage(browser: Browser) {
  return (await browser.newContext()).newPage()
}

/** The last code the development outbox holds for `phone` (E.164). */
export function code(phone: string): string {
  const line = readFileSync(OTP_OUTBOX, 'utf8')
    .split('\n')
    .filter((l) => l.startsWith(`${phone} `))
    .at(-1)
  if (line === undefined) throw new Error(`no code was sent to ${phone}`)
  return line.split(' ')[1] ?? ''
}

/** Signs a patient in to the portal with the code sent to their phone. */
export async function signInByPhone(page: Page, national: string, e164: string, check = false) {
  await page.goto('/portal')
  await expect(page.getByRole('heading', { name: 'Your health portal' })).toBeVisible()
  await press(page, 'Sign in with your phone')
  await page.waitForURL(/\/sign-in\?/)
  if (check) await noViolations(page)
  await type(page, /Phone number/, national)
  await press(page, 'Send me a code')
  await expect(page.getByRole('status').filter({ hasText: 'We sent a code' })).toBeVisible()
  if (check) await noViolations(page)
  await type(page, /Code from the text message/, code(e164))
  await press(page, 'Sign in')
  await page.waitForURL(/\/portal\/?$/)
}
