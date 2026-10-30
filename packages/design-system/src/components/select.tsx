import { Select as ArkSelect, type SelectRootProps } from '@ark-ui/react/select'
import type { CollectionItem } from '@ark-ui/react/collection'
import { Check, ChevronDown, X } from 'lucide-react'
import { useIntl } from 'react-intl'

import { select } from '../../styled-system/recipes'
import { messages } from '../i18n/messages'
import { slot } from './slot'

const classes = select()

const Root = slot(ArkSelect.Root, classes.root)

function LocalizedRoot<T extends CollectionItem>(props: SelectRootProps<T>) {
  const intl = useIntl()
  return <Root translations={{ clearTriggerLabel: intl.formatMessage(messages.selectClear) }} {...(props as SelectRootProps<CollectionItem>)} />
}

const Indicator = slot(ArkSelect.Indicator, classes.indicator)
const ItemIndicator = slot(ArkSelect.ItemIndicator, classes.itemIndicator)
const ClearTrigger = slot(ArkSelect.ClearTrigger, classes.clearTrigger)

export const Select = {
  Root: LocalizedRoot,
  Label: slot(ArkSelect.Label, classes.label),
  Control: slot(ArkSelect.Control, classes.control),
  Trigger: slot(ArkSelect.Trigger, classes.trigger),
  ValueText: slot(ArkSelect.ValueText, classes.valueText),
  Indicator: () => (
    <Indicator>
      <ChevronDown size={16} aria-hidden />
    </Indicator>
  ),
  ClearTrigger: () => (
    <ClearTrigger>
      <X size={16} aria-hidden />
    </ClearTrigger>
  ),
  Positioner: slot(ArkSelect.Positioner, classes.positioner),
  Content: slot(ArkSelect.Content, classes.content),
  /** Only for a popup that also holds a search input; otherwise items go in Content (the listbox). */
  List: slot(ArkSelect.List, classes.list),
  ItemGroup: slot(ArkSelect.ItemGroup, classes.itemGroup),
  ItemGroupLabel: slot(ArkSelect.ItemGroupLabel, classes.itemGroupLabel),
  Item: slot(ArkSelect.Item, classes.item),
  ItemText: slot(ArkSelect.ItemText, classes.itemText),
  ItemIndicator: () => (
    <ItemIndicator>
      <Check size={16} aria-hidden />
    </ItemIndicator>
  ),
  HiddenSelect: ArkSelect.HiddenSelect,
}
