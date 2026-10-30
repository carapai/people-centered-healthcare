import { defineSlotRecipe } from '@pandacss/dev'
import { dialogAnatomy } from '@ark-ui/react/anatomy'

import { focusVisible, iconButton } from './shared'

export const dialog = defineSlotRecipe({
  className: 'dialog',
  slots: dialogAnatomy.keys(),
  base: {
    backdrop: { position: 'fixed', inset: '0', bg: 'bg.backdrop', zIndex: 'overlay', _closed: { display: 'none' } },
    positioner: {
      position: 'fixed',
      inset: '0',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      p: '4',
      zIndex: 'modal',
      _closed: { display: 'none' },
    },
    content: {
      position: 'relative',
      display: 'flex',
      flexDirection: 'column',
      gap: '3',
      width: 'full',
      maxWidth: 'lg',
      maxHeight: 'calc(100dvh - token(spacing.8))',
      overflowY: 'auto',
      p: 'surface',
      color: 'fg.default',
      bg: 'bg.surface',
      borderWidth: '1px',
      borderColor: 'border.default',
      borderRadius: 'lg',
      boxShadow: 'xl',
      _focusVisible: focusVisible,
      _closed: { display: 'none' },
    },
    title: { fontSize: 'lg', fontWeight: 'semibold', pe: '10' },
    description: { fontSize: 'md', color: 'fg.muted' },
    closeTrigger: { ...iconButton, position: 'absolute', top: '2', insetEnd: '2' },
    trigger: {},
  },
})
