import { defineSlotRecipe } from '@pandacss/dev'
import { comboboxAnatomy } from '@ark-ui/react/anatomy'

import { control, floating, groupLabel, iconButton, item, label } from './shared'

export const combobox = defineSlotRecipe({
  className: 'combobox',
  slots: comboboxAnatomy.keys(),
  base: {
    root: { display: 'flex', flexDirection: 'column', gap: '1.5', width: 'full' },
    label,
    control: { ...control, px: '0', ps: 'controlX', _focusWithin: { outline: '2px solid', outlineColor: 'focusRing', outlineOffset: '2px' } },
    input: { flex: '1', minWidth: '0', height: 'full', bg: 'transparent', outline: 'none', fontSize: 'control', color: 'fg.default', _placeholder: { color: 'fg.muted' } },
    trigger: { ...iconButton, _hover: {} },
    clearTrigger: { ...iconButton, _hover: {} },
    positioner: {},
    content: floating,
    list: { display: 'flex', flexDirection: 'column' },
    item,
    itemText: { flex: '1' },
    itemIndicator: { display: 'inline-flex' },
    itemGroup: {},
    itemGroupLabel: groupLabel,
    empty: { px: 'controlX', py: '2', fontSize: 'control', color: 'fg.muted' },
  },
})
