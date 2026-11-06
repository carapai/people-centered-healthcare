import { expect, test } from '@playwright/test'

import { newPage, noViolations, press, signIn, signInByPhone, type, unique } from './helpers'
import { USERS } from './users'

// #114 (§60): a patient's request from the portal is a proposal. It becomes an order only when a
// member of staff accepts it; until then it waits, and the patient may withdraw it.

test('no patient request becomes an order without staff action', async ({ browser }) => {
  test.setTimeout(90_000)
  const { family, digits } = unique()
  const national = `0772${digits.slice(0, 6)}`
  const what = `Blood sugar test ${family}`
  const withdrawn = `Eye check ${family}`

  // A new patient registers themselves and asks for a test, and for something they then withdraw.
  const patient = await newPage(browser)
  await signInByPhone(patient, national, `+256${national.slice(1)}`)
  await type(patient, /Given names/, 'Grace')
  await type(patient, /Family name/, family)
  await patient.getByLabel(/Date of birth/).fill('1991-04-12')
  await patient.getByLabel(/^Sex/).selectOption('female')
  await press(patient, 'Register')
  await expect(patient.getByRole('status').filter({ hasText: 'You are registered.' })).toBeVisible()
  await expect(patient.getByRole('heading', { name: 'Your requests' })).toBeVisible()
  for (const asked of [what, withdrawn]) {
    await type(patient, 'What you need', asked)
    await type(patient, /Facility code/, 'KAM013')
    await press(patient, 'Send request')
    await expect(patient.getByRole('status').filter({ hasText: 'Your request was sent.' })).toBeVisible()
  }
  const mine = patient.getByRole('list', { name: 'Your requests' })
  const request = mine.getByRole('listitem').filter({ hasText: what })
  await expect(request).toContainText('Waiting for staff')
  await noViolations(patient)
  await press(patient, `Withdraw ${withdrawn}`)
  await expect(mine.getByRole('listitem').filter({ hasText: withdrawn })).toContainText('Closed')

  // The clinician finds the request in the inbox: still only a proposal. They accept it.
  const clinician = await newPage(browser)
  await signIn(clinician, USERS.clinician)
  await press(clinician, 'Inbox', 'link')
  await expect(clinician.getByRole('link', { name: `Open ${withdrawn}` })).toHaveCount(0)
  await press(clinician, `Open ${what}`, 'link')
  await expect(clinician.getByRole('heading', { name: 'Patient request' })).toBeVisible()
  await expect(clinician.locator('pre')).toContainText('"intent": "proposal"')
  await noViolations(clinician)
  await press(clinician, 'Accept')
  await expect(clinician.getByRole('status').filter({ hasText: 'Request accepted.' })).toBeVisible()

  // Only now is it accepted, for the patient too, and it has left the inbox.
  await press(patient, 'Check for updates')
  await expect(patient.getByRole('list', { name: 'Your requests' }).getByRole('listitem').filter({ hasText: what })).toContainText('Accepted')
  await clinician.goBack()
  await expect(clinician.getByRole('link', { name: `Open ${what}` })).toHaveCount(0)
})
