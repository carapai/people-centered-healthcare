import { defineSlotRecipe } from '@pandacss/dev'
import { selectAnatomy } from '@ark-ui/react/anatomy'

import { control, floating, groupLabel, iconButton, item, label } from './shared'

export const select = defineSlotRecipe({
  className: 'select',
  slots: selectAnatomy.keys(),
  base: {
    root: { display: 'flex', flexDirection: 'column', gap: '1.5', width: 'full' },
    label,
    control: { position: 'relative', display: 'flex', alignItems: 'center' },
    trigger: { ...control, justifyContent: 'space-between', cursor: 'pointer' },
    valueText: { flex: '1', truncate: true, _placeholder: { color: 'fg.muted' } },
    indicator: { display: 'inline-flex', color: 'fg.muted', _open: { transform: 'rotate(180deg)' } },
    clearTrigger: iconButton,
    positioner: {},
    content: floating,
    list: { display: 'flex', flexDirection: 'column' },
    item,
    itemText: { flex: '1' },
    itemIndicator: { display: 'inline-flex' },
    itemGroup: {},
    itemGroupLabel: groupLabel,
  },
})
