import { expect, test } from '@playwright/test'

import { noViolations, press, signIn } from './helpers'
import { USERS } from './users'

// #498: the OPD draft's author opens its Workflow Definition in the Workflow Studio from the
// draft page. The reference definition lays out with no overlapping nodes and passes the
// checks. With the keyboard alone the author selects a state, opens it in the inspector, adds a
// state (listed as unreachable, outlined, and selected from its problem), then adds the
// transitions that reach it and leave it, renames it, and saves through the same If-Match path
// as the file editor. Opened again, the outline holds what was saved. With axe.
test('the author edits the OPD workflow in the outline, canvas and inspector, with the keyboard alone, and saves it', async ({ page }) => {
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await page.getByRole('grid', { name: 'Studio' }).getByText('ug.moh.opd', { exact: false }).click()
  await page.getByRole('region', { name: 'What changed' }).getByRole('link', { name: 'Library-opd-visit.json in the Workflow Studio' }).focus()
  await page.keyboard.press('Enter')
  await expect(page.getByRole('heading', { name: 'Workflow Studio: OPD visit', level: 1 })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'No publication problems found while editing' })).toBeVisible()
  const states = page.getByRole('navigation', { name: 'States' })
  const inspector = page.getByRole('region', { name: 'Inspect' })
  const canvas = page.getByRole('region', { name: 'Canvas' })

  // Every state is on the canvas, and no two at the same level overlap.
  const nodes = canvas.locator('.react-flow__node')
  await expect(nodes).toHaveCount(16)
  const top = ['reception', 'triage', 'payment', 'consultation', 'orders', 'review', 'disposition', 'discharged', 'admitted', 'referred']
  const boxes = await Promise.all(top.map(async (id) => (await canvas.locator(`.react-flow__node[data-id="${id}"]`).boundingBox()) ?? { x: 0, y: 0, width: 0, height: 0 }))
  for (const [n, a] of boxes.entries())
    for (const b of boxes.slice(n + 1)) expect(a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height).toBe(false)
  await expect(page.getByRole('link', { name: 'React Flow attribution' })).toBeVisible()
  await noViolations(page)

  // Keyboard: select a state in the outline; the inspector shows it.
  await press(page, /^consultation /)
  await expect(inspector.getByRole('heading', { name: 'State consultation' })).toBeVisible()
  await expect(inspector.getByLabel('Role')).toHaveValue('clinician')

  // A new state is unreachable: listed, outlined, and its problem selects it.
  await press(page, /^Add a state$/)
  await expect(page.getByRole('button', { name: '“state” cannot be reached' })).toBeVisible()
  await expect(states.getByRole('button', { name: /^state / })).toHaveAttribute('data-flagged', 'true')
  await press(page, /^reception /)
  await expect(inspector.getByRole('heading', { name: 'State reception' })).toBeVisible()
  await press(page, '“state” cannot be reached')
  await expect(inspector.getByRole('heading', { name: 'State state' })).toBeVisible()

  // A transition from consultation reaches it; one from it to discharged gives it a way out.
  await press(page, /^consultation /)
  await inspector.getByRole('combobox', { name: 'To', exact: true }).selectOption('state')
  await press(page, 'Add a transition')
  await expect(inspector.getByRole('heading', { name: 'Transition from consultation to state' })).toBeVisible()
  await inspector.getByLabel('Events (comma separated)').fill('opd.left-early')
  await expect(page.getByRole('button', { name: '“state” has no way out' })).toBeVisible()
  await press(page, /^state /)
  await inspector.getByRole('combobox', { name: 'To', exact: true }).selectOption('discharged')
  await press(page, 'Add a transition')
  await expect(page.getByRole('heading', { name: 'No publication problems found while editing' })).toBeVisible()
  await expect(states.getByRole('button', { name: '→ discharged on state.to-discharged' })).toBeVisible()

  // Renamed, its references follow.
  await press(page, /^state /)
  await inspector.getByLabel('Id', { exact: true }).fill('left-early')
  await page.keyboard.press('Enter')
  await expect(inspector.getByRole('heading', { name: 'State left-early' })).toBeVisible()
  await expect(states.getByRole('button', { name: '→ left-early on opd.left-early' })).toBeVisible()
  await expect(canvas.locator('.react-flow__node[data-id="left-early"]')).toBeVisible()
  await noViolations(page)

  await press(page, /^Save$/)
  await expect(page.getByRole('status').filter({ hasText: 'Saved.' })).toBeVisible()

  // Opened again, the definition is as saved.
  await press(page, 'Back to the package', 'link')
  await page.getByRole('region', { name: 'Files' }).getByRole('link', { name: 'Library-opd-visit.json in the Workflow Studio' }).click()
  await expect(states.getByRole('button', { name: /^left-early / })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'No publication problems found while editing' })).toBeVisible()
})

// #498, part 2: typed actions are picked from the release's catalogue, which central serves at
// /api/studio/workflow-actions. No Bounded Context registers an action in this release yet, so
// the catalogue is stubbed here with one action; central's own test covers the endpoint. The
// author adds the action to a timer step, fills its parameters from fields generated from its
// schema, sees what it needs and how it is undone, and declares the permission the workflow
// principal lacks from the check. Saved and opened again, the principal holds it. With axe.
test('the author picks a typed action from the catalogue, fills its parameters and declares the principal permission a timer step needs', async ({ page }) => {
  await page.route('**/api/studio/workflow-actions', (route) =>
    route.fulfill({
      json: {
        actions: [
          {
            reference: 'patient-flow/open-queue-entry@1',
            id: 'patient-flow/open-queue-entry',
            major: 1,
            permission: 'queues.work',
            compensation: { kind: 'action', action: 'patient-flow/close-queue-entry' },
            parameterSchema: {
              $schema: 'https://json-schema.org/draft/2020-12/schema',
              title: 'patient-flow/open-queue-entry@1',
              type: 'object',
              properties: { priority: { type: 'string', enum: ['routine', 'urgent'] }, service_point: { type: 'string', minLength: 1 } },
              required: ['service_point'],
              additionalProperties: false,
            },
          },
        ],
      },
    }),
  )
  await signIn(page, USERS.author)
  await press(page, 'Studio', 'link')
  await page.getByRole('grid', { name: 'Studio' }).getByText('ug.moh.opd', { exact: false }).click()
  await page.getByRole('region', { name: 'Files' }).getByRole('link', { name: 'Library-opd-visit.json in the Workflow Studio' }).click()
  const states = page.getByRole('navigation', { name: 'States' })
  const inspector = page.getByRole('region', { name: 'Inspect' })
  await expect(page.getByText('The workflow principal holds no permissions.', { exact: false })).toBeVisible()

  // reception's first exit becomes a timer, and runs the action.
  await states.getByRole('button', { name: '→ triage on opd.arrived' }).click()
  await expect(inspector.getByRole('heading', { name: 'Transition from reception to triage' })).toBeVisible()
  await inspector.getByLabel('Triggered by').selectOption('timer')
  await expect(inspector.getByLabel('Typed action to add')).toHaveValue('patient-flow/open-queue-entry@1')
  await press(page, 'Add: typed action')
  await expect(inspector.getByLabel('Typed action 1')).toHaveValue('patient-flow/open-queue-entry@1')
  await expect(inspector.getByText('Needs the permission queues.work')).toBeVisible()
  await expect(inspector.getByText('Undone by patient-flow/close-queue-entry')).toBeVisible()
  await expect(page.getByRole('button', { name: '“patient-flow/open-queue-entry@1” needs its parameter “service_point”' })).toBeVisible()

  // Its parameters are fields from its schema.
  await inspector.getByLabel('service_point of action 1 (required)').fill('opd-main')
  await inspector.getByLabel('priority of action 1').selectOption('urgent')
  await expect(page.getByRole('button', { name: /needs its parameter/ })).toHaveCount(0)

  // The timer runs as the workflow principal, which lacks the permission: declared from the check.
  const lacks = page.getByRole('button', { name: 'A timer or eventless step runs “patient-flow/open-queue-entry@1” as the workflow principal, which lacks “queues.work”' })
  await expect(lacks).toBeVisible()
  await lacks.click()
  await expect(inspector.getByRole('heading', { name: 'Transition from reception to triage' })).toBeVisible()
  await press(page, 'Declare “queues.work” for the workflow principal')
  await expect(page.getByRole('heading', { name: 'No publication problems found while editing' })).toBeVisible()
  await expect(page.getByText('The workflow principal holds queues.work.', { exact: false })).toBeVisible()
  await noViolations(page)

  await press(page, /^Save$/)
  await expect(page.getByRole('status').filter({ hasText: 'Saved.' })).toBeVisible()
  await press(page, 'Back to the package', 'link')
  await page.getByRole('region', { name: 'Files' }).getByRole('link', { name: 'Library-opd-visit.json in the Workflow Studio' }).click()
  await expect(page.getByText('The workflow principal holds queues.work.', { exact: false })).toBeVisible()
  await states.getByRole('button', { name: /^→ triage on / }).first().click()
  await expect(inspector.getByLabel('service_point of action 1 (required)')).toHaveValue('opd-main')
})
