import { defineSlotRecipe } from '@pandacss/dev'
import { datePickerAnatomy } from '@ark-ui/react/anatomy'

import { control, floating, focusVisible, iconButton, label } from './shared'

export const datePicker = defineSlotRecipe({
  className: 'datePicker',
  slots: datePickerAnatomy.keys(),
  base: {
    root: { display: 'flex', flexDirection: 'column', gap: '1.5', width: 'full' },
    label,
    control: { display: 'flex', alignItems: 'center', gap: 'gap' },
    input: control,
    trigger: iconButton,
    clearTrigger: iconButton,
    positioner: {},
    content: { ...floating, width: 'fit-content', maxHeight: 'none', minWidth: 'auto', p: '3', gap: '2', '&[data-inline]': { boxShadow: 'none' } },
    view: { display: 'flex', flexDirection: 'column', gap: '2' },
    viewControl: { display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 'gap' },
    viewTrigger: { flex: '1', minHeight: 'control', fontWeight: 'semibold', borderRadius: 'md', cursor: 'pointer', _hover: { bg: 'bg.hover' }, _focusVisible: focusVisible },
    prevTrigger: iconButton,
    nextTrigger: iconButton,
    rangeText: { fontWeight: 'semibold' },
    valueText: {},
    table: { borderCollapse: 'separate', borderSpacing: '1' },
    tableHead: {},
    tableBody: {},
    tableRow: {},
    tableHeader: { fontSize: 'xs', fontWeight: 'semibold', color: 'fg.muted', height: 'cell' },
    tableCell: { textAlign: 'center' },
    tableCellTrigger: {
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      minWidth: 'cell',
      height: 'cell',
      px: '1',
      fontSize: 'control',
      borderRadius: 'md',
      cursor: 'pointer',
      _hover: { bg: 'bg.hover' },
      _focusVisible: focusVisible,
      '&[data-today]': { fontWeight: 'bold', textDecoration: 'underline' },
      _selected: { bg: 'accent.default', color: 'accent.fg', _hover: { bg: 'accent.emphasized' } },
      // Outside-month days stay readable: they are real, selectable dates.
      '&[data-outside-range]': { color: 'fg.muted' },
      _disabled: { color: 'fg.disabled', cursor: 'not-allowed', _hover: { bg: 'transparent' } },
    },
    monthSelect: control,
    yearSelect: control,
    presetTrigger: {},
  },
})
