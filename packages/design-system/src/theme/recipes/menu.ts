import { defineSlotRecipe } from '@pandacss/dev'
import { menuAnatomy } from '@ark-ui/react/anatomy'

import { floating, groupLabel, item } from './shared'

export const menu = defineSlotRecipe({
  className: 'menu',
  slots: menuAnatomy.keys(),
  base: {
    trigger: {},
    contextTrigger: {},
    positioner: {},
    content: { ...floating, outline: 'none' },
    item: { ...item, justifyContent: 'flex-start' },
    triggerItem: { ...item, justifyContent: 'space-between' },
    itemText: { flex: '1' },
    itemIndicator: { display: 'inline-flex' },
    itemGroup: {},
    itemGroupLabel: groupLabel,
    indicator: { display: 'inline-flex' },
    separator: { my: '1', borderTopWidth: '1px', borderColor: 'border.default' },
    arrow: {},
    arrowTip: {},
  },
})
