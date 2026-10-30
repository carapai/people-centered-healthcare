import { defineStyles } from '@pandacss/dev'

// Style fragments shared by the slot recipes.

export const focusVisible = defineStyles({
  outline: '2px solid',
  outlineColor: 'focusRing',
  outlineOffset: '2px',
})

/** An input or a trigger that looks like one. */
export const control = defineStyles({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'gap',
  width: 'full',
  minHeight: 'control',
  px: 'controlX',
  fontSize: 'control',
  color: 'fg.default',
  bg: 'bg.surface',
  borderWidth: '1px',
  borderColor: 'border.control',
  borderRadius: 'md',
  textAlign: 'start',
  _placeholder: { color: 'fg.muted' },
  _focusVisible: focusVisible,
  _invalid: { borderColor: 'danger.border', borderWidth: '2px' },
  _disabled: { color: 'fg.disabled', cursor: 'not-allowed' },
})

/** A square icon button. */
export const iconButton = defineStyles({
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  flexShrink: '0',
  width: 'control',
  height: 'control',
  color: 'fg.default',
  borderRadius: 'md',
  cursor: 'pointer',
  _hover: { bg: 'bg.hover' },
  _focusVisible: focusVisible,
  _disabled: { color: 'fg.disabled', cursor: 'not-allowed' },
})

/** A floating surface: listbox, menu, popover. */
export const floating = defineStyles({
  display: 'flex',
  flexDirection: 'column',
  minWidth: '12rem',
  maxHeight: '20rem',
  overflowY: 'auto',
  p: '1',
  color: 'fg.default',
  bg: 'bg.surface',
  borderWidth: '1px',
  borderColor: 'border.default',
  borderRadius: 'md',
  boxShadow: 'lg',
  zIndex: 'dropdown',
  _focusVisible: focusVisible,
  _closed: { display: 'none' },
})

/** An option in a listbox or menu. */
export const item = defineStyles({
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: 'gap',
  minHeight: 'control',
  px: 'controlX',
  fontSize: 'control',
  borderRadius: 'sm',
  cursor: 'pointer',
  _highlighted: { bg: 'bg.hover', outline: '2px solid', outlineColor: 'focusRing', outlineOffset: '-2px' },
  _checked: { bg: 'bg.selected', color: 'fg.selected' },
  _selected: { bg: 'bg.selected', color: 'fg.selected' },
  _disabled: { color: 'fg.disabled', cursor: 'not-allowed' },
})

export const label = defineStyles({
  fontSize: 'sm',
  fontWeight: 'medium',
  color: 'fg.default',
})

export const groupLabel = defineStyles({
  px: 'controlX',
  py: '1',
  fontSize: 'xs',
  fontWeight: 'semibold',
  color: 'fg.muted',
})
