import { Combobox as ArkCombobox, type ComboboxRootProps } from '@ark-ui/react/combobox'
import type { CollectionItem } from '@ark-ui/react/collection'
import { Check, ChevronDown, X } from 'lucide-react'
import { useIntl } from 'react-intl'

import { combobox } from '../../styled-system/recipes'
import { messages } from '../i18n/messages'
import { slot } from './slot'

const classes = combobox()

const Root = slot(ArkCombobox.Root, classes.root)

function LocalizedRoot<T extends CollectionItem>(props: ComboboxRootProps<T>) {
  const intl = useIntl()
  return (
    <Root
      translations={{
        triggerLabel: intl.formatMessage(messages.comboboxTrigger),
        clearTriggerLabel: intl.formatMessage(messages.comboboxClear),
      }}
      {...(props as ComboboxRootProps<CollectionItem>)}
    />
  )
}

const Trigger = slot(ArkCombobox.Trigger, classes.trigger)
const ClearTrigger = slot(ArkCombobox.ClearTrigger, classes.clearTrigger)
const ItemIndicator = slot(ArkCombobox.ItemIndicator, classes.itemIndicator)
const Empty = slot(ArkCombobox.Empty, classes.empty)

export const Combobox = {
  Root: LocalizedRoot,
  Label: slot(ArkCombobox.Label, classes.label),
  Control: slot(ArkCombobox.Control, classes.control),
  Input: slot(ArkCombobox.Input, classes.input),
  Trigger: () => (
    <Trigger>
      <ChevronDown size={16} aria-hidden />
    </Trigger>
  ),
  ClearTrigger: () => (
    <ClearTrigger>
      <X size={16} aria-hidden />
    </ClearTrigger>
  ),
  Positioner: slot(ArkCombobox.Positioner, classes.positioner),
  Content: slot(ArkCombobox.Content, classes.content),
  /** Only when Content holds more than the options; otherwise items go in Content (the listbox). */
  List: slot(ArkCombobox.List, classes.list),
  ItemGroup: slot(ArkCombobox.ItemGroup, classes.itemGroup),
  ItemGroupLabel: slot(ArkCombobox.ItemGroupLabel, classes.itemGroupLabel),
  Item: slot(ArkCombobox.Item, classes.item),
  ItemText: slot(ArkCombobox.ItemText, classes.itemText),
  ItemIndicator: () => (
    <ItemIndicator>
      <Check size={16} aria-hidden />
    </ItemIndicator>
  ),
  Empty: () => {
    const intl = useIntl()
    return <Empty>{intl.formatMessage(messages.comboboxEmpty)}</Empty>
  },
}
