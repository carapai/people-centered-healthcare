import { defineSlotRecipe } from '@pandacss/dev'
import { fieldAnatomy } from '@ark-ui/react/anatomy'

import { control, label } from './shared'

export const field = defineSlotRecipe({
  className: 'field',
  slots: fieldAnatomy.keys(),
  base: {
    root: { display: 'flex', flexDirection: 'column', gap: '1.5' },
    label: { ...label, display: 'inline-flex', gap: '1' },
    requiredIndicator: { color: 'danger.text' },
    input: control,
    textarea: { ...control, minHeight: '6rem', py: '2' },
    select: control,
    helperText: { fontSize: 'sm', color: 'fg.muted' },
    errorText: { fontSize: 'sm', fontWeight: 'medium', color: 'danger.text' },
  },
})
