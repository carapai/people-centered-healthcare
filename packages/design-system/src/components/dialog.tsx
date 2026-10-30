import { Dialog as ArkDialog } from '@ark-ui/react/dialog'
import { X } from 'lucide-react'
import { useIntl } from 'react-intl'

import { dialog } from '../../styled-system/recipes'
import { messages } from '../i18n/messages'
import { slot } from './slot'

const classes = dialog()

const CloseTrigger = slot(ArkDialog.CloseTrigger, classes.closeTrigger)

export const Dialog = {
  Root: ArkDialog.Root,
  Trigger: slot(ArkDialog.Trigger, classes.trigger),
  Backdrop: slot(ArkDialog.Backdrop, classes.backdrop),
  Positioner: slot(ArkDialog.Positioner, classes.positioner),
  Content: slot(ArkDialog.Content, classes.content),
  Title: slot(ArkDialog.Title, classes.title),
  Description: slot(ArkDialog.Description, classes.description),
  /** The corner close button, with a localized accessible name. */
  CloseTrigger: () => {
    const intl = useIntl()
    return (
      <CloseTrigger aria-label={intl.formatMessage(messages.dialogClose)}>
        <X size={18} aria-hidden />
      </CloseTrigger>
    )
  },
}
