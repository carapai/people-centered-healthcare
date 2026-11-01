import { expect, test } from '@playwright/test'

import { newPage, noViolations, press, signIn, signInByPhone, type } from './helpers'
import { USERS } from './users'

// Caregiver delegation (#113): staff record a mother as her adolescent daughter's guardian; the
// mother sees her daughter in the portal with her sensitive records withheld until the daughter
// consents, and nothing once the access is revoked.

test("a guardian sees her daughter, but not her sensitive records without the daughter's consent", async ({ browser }) => {
  // The mother has signed in to the portal once.
  const mother = await newPage(browser)
  await signInByPhone(mother, '0772100003', '+256772100003')

  // At reception, the daughter (14) is registered, and her mother recorded as her guardian.
  const clerk = await newPage(browser)
  await signIn(clerk, USERS.clerk)
  await press(clerk, 'Register a patient', 'link')
  await type(clerk, /Family name/, 'Okot')
  await type(clerk, /Given names/, 'Amina')
  await clerk.getByLabel(/^Sex/).selectOption('female')
  await clerk.getByLabel(/Date of birth/).fill('2012-05-05')
  await press(clerk, 'Check for this patient')
  await expect(clerk.getByRole('status').filter({ hasText: 'Registered. MRN:' })).toBeVisible()
  await press(clerk, 'Open patient record', 'link')
  await expect(clerk.getByRole('heading', { name: 'Amina Okot' })).toBeVisible()
  await expect(clerk.getByText('No caregivers are recorded.')).toBeVisible()
  await noViolations(clerk)
  await type(clerk, /Caregiver's phone/, '0772100003')
  await clerk.getByLabel(/Last day of access/).fill('2030-12-31')
  await type(clerk, /Document checked/, 'birth certificate')
  await press(clerk, 'Record caregiver')
  await expect(clerk.getByText(/sensitive records are not shown to this guardian/)).toBeVisible()
  await noViolations(clerk)

  // The mother sees her daughter, with some records private to her.
  await press(mother, 'Refresh the people you care for')
  await expect(mother.getByRole('heading', { name: 'People you care for' })).toBeVisible()
  await expect(mother.getByText('Amina Okot')).toBeVisible()
  await expect(mother.getByText(/some of their records stay private to them/)).toBeVisible()
  await noViolations(mother)

  // With the daughter's consent, nothing is withheld.
  await press(clerk, "Record the young person's consent")
  await expect(clerk.getByText(/The young person consents/)).toBeVisible()
  await press(mother, 'Refresh the people you care for')
  await expect(mother.getByText(/some of their records stay private to them/)).toBeHidden()

  // Revoked: the daughter is gone from the mother's portal.
  await press(clerk, 'Revoke access')
  await expect(clerk.getByText(/Revoked/)).toBeVisible()
  await press(mother, 'Refresh the people you care for')
  await expect(mother.getByText('Amina Okot')).toBeHidden()
})
