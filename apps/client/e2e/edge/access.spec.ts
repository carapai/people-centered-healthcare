import { type Page, expect as baseExpect, test } from '@playwright/test'

import { noViolations } from '../helpers'
import { staff, startEdge } from './edge'

// Offline administration on the edge (#75; offline-auth.md § Offline user and role
// administration): the facility admin creates an account for new staff, whose user replaces the
// temporary password at their first sign-in; then the admin revokes their access here.

// Nearly every step hashes or verifies a password (Argon2id), slow in a debug build.
const expect = baseExpect.configure({ timeout: 30_000 })

const newcomer = { username: 'auma', temporary: 'harbour violet 3318', own: 'copper lantern meadow 52' }

async function signIn(page: Page, username: string, password: string) {
  await page.goto('/')
  await page.getByRole('button', { name: 'Sign in' }).click()
  await page.getByLabel('Username').fill(username)
  await page.getByLabel('Password').fill(password)
  await page.getByRole('button', { name: 'Sign in' }).click()
}

test('an admin creates an account for new staff, who set their own password, and revokes it', async ({ browser }) => {
  // An edge, three sign-ins and a password change: well over the default 30 seconds.
  test.setTimeout(240_000)
  const edge = await startEdge({ signIn: true })
  try {
    // The edge's own screen imports central's bundle and enrolls a device.
    const screen = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
    expect((await screen.request.post('/api/admin/provision/accounts', { data: edge.bundle ?? '' })).status()).toBe(200)
    const device = await (await browser.newContext({ baseURL: edge.origin, serviceWorkers: 'block' })).newPage()
    await device.goto('/enroll')
    await device.getByRole('button', { name: 'Ask to enroll' }).click()
    const code = (await device.getByLabel('Pairing code').textContent())?.trim() ?? ''
    await screen.goto('/edge/status')
    const devices = screen.getByRole('region', { name: 'Devices' })
    await devices.getByLabel('Pairing code shown on the device').fill(code)
    await devices.getByLabel('Device name').fill('Ward 3 PC')
    await devices.getByLabel('Location or ward').fill('Ward 3')
    await devices.getByRole('button', { name: 'Approve a device' }).click()
    await expect(devices.getByRole('status')).toHaveText('Ward 3 PC is enrolled.')

    // The facility admin creates the account, its role a local grant.
    await signIn(device, staff.username, staff.password)
    await expect(device.getByRole('main').getByRole('status').first()).toHaveText('Signed in')
    await device.getByRole('link', { name: 'Access at this facility' }).click()
    const accounts = device.getByRole('region', { name: 'Account for new staff' })
    await accounts.getByLabel('Username').fill(newcomer.username)
    await accounts.getByLabel('Temporary password').fill(newcomer.temporary)
    await accounts.getByLabel('Role').selectOption('clinician')
    await accounts.getByRole('button', { name: 'Create the account' }).click()
    await expect(accounts.getByRole('status')).toContainText(`The account ${newcomer.username} is a clinician here until`)
    await expect(device.getByRole('region', { name: 'Roles granted here' }).getByRole('listitem')).toContainText(`${newcomer.username}: clinician until`)
    // A taken username is refused.
    await accounts.getByLabel('Username').fill(newcomer.username)
    await accounts.getByLabel('Temporary password').fill(newcomer.temporary)
    await accounts.getByLabel('Role').selectOption('clinician')
    await accounts.getByRole('button', { name: 'Create the account' }).click()
    await expect(accounts.getByRole('alert')).toHaveText('Not done: that username is already taken')
    await noViolations(device)
    // Back home in the app: a page load would drop the tokens, which live in memory only.
    await device.goBack()
    await device.getByRole('button', { name: 'Sign out' }).click()

    // Their first sign-in asks for a password of their own.
    await signIn(device, newcomer.username, newcomer.temporary)
    await expect(device.getByRole('heading', { name: 'Choose your own password' })).toBeVisible()
    await noViolations(device)
    const change = async (temporary: string, own: string, again: string) => {
      await device.getByLabel('Username').fill(newcomer.username)
      await device.getByLabel('Temporary password').fill(temporary)
      await device.getByLabel('New password', { exact: true }).fill(own)
      await device.getByLabel('New password again').fill(again)
      await device.getByRole('button', { name: 'Set password and sign in' }).click()
    }
    await change(newcomer.temporary, newcomer.own, 'something else entirely')
    await expect(device.getByRole('alert')).toHaveText('The new passwords do not match.')
    await change(newcomer.temporary, 'password', 'password')
    await expect(device.getByRole('alert')).toHaveText('That password cannot be used: it is too short or too common. Choose another.')
    await change(newcomer.temporary, newcomer.own, newcomer.own)
    await expect(device.getByRole('main').getByRole('status').first()).toHaveText('Signed in')
    // A clinician, not an admin.
    await device.getByRole('link', { name: 'Access at this facility' }).click()
    await expect(device.getByRole('alert')).toHaveText('Your role does not include administering access here.')
    // Back home in the app: a page load would drop the tokens, which live in memory only.
    await device.goBack()
    await device.getByRole('button', { name: 'Sign out' }).click()

    // The admin revokes their access here, after confirming.
    await signIn(device, staff.username, staff.password)
    await expect(device.getByRole('main').getByRole('status').first()).toHaveText('Signed in')
    await device.getByRole('link', { name: 'Access at this facility' }).click()
    const revocations = device.getByRole('region', { name: 'Access revoked here' })
    await revocations.getByLabel('Username').fill(newcomer.username)
    await revocations.getByRole('button', { name: 'Revoke access' }).click()
    await revocations.getByRole('button', { name: `Revoke access for ${newcomer.username}` }).click()
    await expect(revocations.getByRole('status')).toHaveText(`Access for ${newcomer.username} is revoked here.`)
    await expect(revocations.getByRole('listitem')).toContainText(`${newcomer.username}: revoked`)
    // Back home in the app: a page load would drop the tokens, which live in memory only.
    await device.goBack()
    await device.getByRole('button', { name: 'Sign out' }).click()

    // Now they cannot sign in here, even with their own password.
    await signIn(device, newcomer.username, newcomer.own)
    await expect(device.getByRole('alert')).toHaveText('The username or password is incorrect, or the account is locked.')
  } finally {
    await edge.stop()
  }
})
