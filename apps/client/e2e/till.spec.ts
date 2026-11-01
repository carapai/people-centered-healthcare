import { expect, test } from '@playwright/test'

import { newPage, noViolations, press, signIn, type } from './helpers'
import { USERS } from './users'

// The till (#254): a mobile-money receipt stays provisional until a supervisor enters the
// provider's statement line; a short close waits for the supervisor's approval.

test('a provisional receipt is confirmed only by the provider statement', async ({ browser }) => {
  const cashier = await newPage(browser)
  await signIn(cashier, USERS.cashier)
  await press(cashier, 'Till', 'link')
  await expect(cashier.getByRole('status').filter({ hasText: 'No shift is open at this till.' })).toBeVisible()
  await noViolations(cashier)
  await press(cashier, 'Open shift')
  await expect(cashier.getByRole('status').filter({ hasText: 'cash expected' })).toBeVisible()

  await type(cashier, /^Amount/, '30000')
  await type(cashier, /Paid for/, 'Account/acc-e2e')
  await press(cashier, 'Record payment')
  await expect(cashier.getByRole('listitem').filter({ hasText: '30000' })).toContainText('Confirmed')

  await cashier.getByLabel(/^Method/).selectOption('mtn-momo')
  await type(cashier, /^Amount/, '20000')
  await type(cashier, /Provider reference/, 'x-ref-e2e-1')
  await noViolations(cashier)
  await press(cashier, 'Record payment')
  const mtn = cashier.getByRole('listitem').filter({ hasText: 'MTN Mobile Money' })
  await expect(mtn).toContainText('Provisional')

  // The supervisor enters the statement line.
  const supervisor = await newPage(browser)
  await signIn(supervisor, USERS.supervisor)
  await press(supervisor, 'Till', 'link')
  await press(supervisor, 'Supervise payments', 'link')
  await noViolations(supervisor)
  await type(supervisor, /Provider reference/, 'x-ref-e2e-1')
  await type(supervisor, /^Amount/, '20000')
  await press(supervisor, 'Reconcile')
  await expect(supervisor.getByRole('status').filter({ hasText: 'Confirmed: 1' })).toBeVisible()

  await press(cashier, 'Refresh receipts')
  await expect(mtn).toContainText('Confirmed')

  // 30,000 expected, 29,500 counted: closed, and waiting for the supervisor.
  await type(cashier, /Cash counted/, '29500')
  await press(cashier, 'Close shift')
  const closed = cashier.getByRole('status').filter({ hasText: 'a supervisor must approve' })
  await expect(closed).toContainText('-500')
  const shift = (await closed.textContent())?.match(/shift-[0-9a-f-]+/)?.[0] ?? ''
  expect(shift).not.toBe('')
  await type(supervisor, /^Shift/, shift)
  await press(supervisor, 'Approve variance')
  await expect(supervisor.getByRole('status').filter({ hasText: 'Shift settled.' })).toBeVisible()
})
