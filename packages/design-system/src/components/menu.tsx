import { Menu as ArkMenu } from '@ark-ui/react/menu'
import { Check, ChevronRight } from 'lucide-react'

import { menu } from '../../styled-system/recipes'
import { slot } from './slot'

const classes = menu()

const ItemIndicator = slot(ArkMenu.ItemIndicator, classes.itemIndicator)

export const Menu = {
  Root: ArkMenu.Root,
  Trigger: slot(ArkMenu.Trigger, classes.trigger),
  ContextTrigger: slot(ArkMenu.ContextTrigger, classes.contextTrigger),
  Positioner: slot(ArkMenu.Positioner, classes.positioner),
  Content: slot(ArkMenu.Content, classes.content),
  Item: slot(ArkMenu.Item, classes.item),
  CheckboxItem: slot(ArkMenu.CheckboxItem, classes.item),
  RadioItemGroup: slot(ArkMenu.RadioItemGroup, classes.itemGroup),
  RadioItem: slot(ArkMenu.RadioItem, classes.item),
  TriggerItem: (props: Parameters<typeof ArkMenu.TriggerItem>[0]) => (
    <ArkMenu.TriggerItem {...props} className={classes.triggerItem}>
      {props.children}
      <ChevronRight size={16} aria-hidden />
    </ArkMenu.TriggerItem>
  ),
  ItemText: slot(ArkMenu.ItemText, classes.itemText),
  ItemIndicator: () => (
    <ItemIndicator>
      <Check size={16} aria-hidden />
    </ItemIndicator>
  ),
  ItemGroup: slot(ArkMenu.ItemGroup, classes.itemGroup),
  ItemGroupLabel: slot(ArkMenu.ItemGroupLabel, classes.itemGroupLabel),
  Separator: slot(ArkMenu.Separator, classes.separator),
}
