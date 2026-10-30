import { Toast as ArkToast, Toaster as ArkToaster, createToaster, type CreateToasterReturn } from '@ark-ui/react/toast'
import { X } from 'lucide-react'
import { useIntl } from 'react-intl'

import { toast } from '../../styled-system/recipes'
import { messages } from '../i18n/messages'
import { slot } from './slot'

const classes = toast()

const Root = slot(ArkToast.Root, classes.root)
const Title = slot(ArkToast.Title, classes.title)
const Description = slot(ArkToast.Description, classes.description)
const ActionTrigger = slot(ArkToast.ActionTrigger, classes.actionTrigger)
const CloseTrigger = slot(ArkToast.CloseTrigger, classes.closeTrigger)

export { createToaster }

export interface ToasterProps {
  toaster: CreateToasterReturn
  className?: string | undefined
}

/** The notification region: each toast with its title, description, action and close button. */
export function Toaster({ toaster, className }: ToasterProps) {
  const intl = useIntl()
  const close = intl.formatMessage(messages.toastClose)
  return (
    <ArkToaster toaster={toaster} label={intl.formatMessage(messages.toastRegion)} className={className}>
      {(t) => (
        <Root>
          {t.title !== undefined && <Title>{t.title}</Title>}
          {t.description !== undefined && <Description>{t.description}</Description>}
          {t.action !== undefined && <ActionTrigger>{t.action.label}</ActionTrigger>}
          <CloseTrigger aria-label={close}>
            <X size={16} aria-hidden />
          </CloseTrigger>
        </Root>
      )}
    </ArkToaster>
  )
}
