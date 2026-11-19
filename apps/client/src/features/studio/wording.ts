import type { ActionRefusal, PackageStage, StudioActionName, StudioCheck } from '@health/api-client'
import type { IntlShape } from 'react-intl'

import { studioMessages as m } from '../../i18n/messages'

// How the Studio words what the server decided. Nothing here decides anything: the actions, their
// order and every reason come from the server.

const actionLabels = {
  submit: m.sdActionSubmit,
  review: m.sdActionReview,
  approve: m.sdActionApprove,
  publish: m.sdActionPublish,
  activate: m.sdActionActivate,
  'request-changes': m.sdActionRequestChanges,
} as const satisfies Record<StudioActionName, unknown>

const stageLabels = {
  draft: m.sdStageDraft,
  validation: m.sdStageValidation,
  simulation: m.sdStageSimulation,
  review: m.sdStageReview,
  approval: m.sdStageApproval,
  published: m.sdStagePublished,
} as const satisfies Record<PackageStage, unknown>

const checkLabels = { validation: m.sdCheckValidation, simulation: m.sdCheckSimulation } as const satisfies Record<StudioCheck, unknown>

export function actionLabel(intl: IntlShape, action: StudioActionName): string {
  return intl.formatMessage(actionLabels[action])
}

export function stageLabel(intl: IntlShape, stage: PackageStage): string {
  return intl.formatMessage(stageLabels[stage])
}

export function checkLabel(intl: IntlShape, check: StudioCheck): string {
  return intl.formatMessage(checkLabels[check])
}

/** Why the server refused a step, in the person's language; a refusal it has no words for is
 * shown as the server said it. */
export function refusalText(intl: IntlShape, refusal: ActionRefusal): string {
  switch (refusal.code) {
    case 'permission':
      return intl.formatMessage(m.sdRefusalPermission, { permission: refusal.permission ?? '' })
    case 'stage':
      return refusal.stage === null || refusal.stage === undefined ? refusal.message : intl.formatMessage(m.sdRefusalStage, { stage: stageLabel(intl, refusal.stage) })
    case 'author':
      return intl.formatMessage(m.sdRefusalAuthor)
    case 'not-author':
      return intl.formatMessage(m.sdRefusalNotAuthor)
    case 'already-approved':
      return intl.formatMessage(m.sdRefusalAlreadyApproved)
    case 'approvals-needed':
      return intl.formatMessage(m.sdRefusalApprovalsNeeded, { missing: refusal.missing ?? 0 })
    case 'not-published':
      return intl.formatMessage(m.sdRefusalNotPublished)
    case 'other':
      return refusal.message
  }
}
