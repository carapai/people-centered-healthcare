import { defineSlotRecipe } from '@pandacss/dev'
import { toastAnatomy } from '@ark-ui/react/anatomy'

import { focusVisible, iconButton } from './shared'

// Ark sets data-type (info, success, warning, error, loading) on the root.
export const toast = defineSlotRecipe({
  className: 'toast',
  slots: toastAnatomy.keys(),
  base: {
    group: {},
    root: {
      position: 'relative',
      display: 'flex',
      flexDirection: 'column',
      gap: '1',
      width: 'sm',
      maxWidth: 'calc(100vw - token(spacing.8))',
      p: '4',
      pe: '12',
      color: 'fg.default',
      bg: 'bg.surface',
      borderWidth: '1px',
      borderStartWidth: '4px',
      borderColor: 'border.default',
      borderRadius: 'md',
      boxShadow: 'lg',
      translate: 'var(--x) var(--y)',
      scale: 'var(--scale)',
      opacity: 'var(--opacity)',
      zIndex: 'var(--z-index)',
      height: 'var(--height)',
      willChange: 'translate, opacity, scale',
      transition: 'translate 400ms, scale 400ms, opacity 400ms, height 400ms',
      '&[data-type=success]': { borderStartColor: 'success.text' },
      '&[data-type=error]': { borderStartColor: 'danger.border' },
      '&[data-type=info]': { borderStartColor: 'accent.default' },
      _focusVisible: focusVisible,
    },
    title: { fontWeight: 'semibold', fontSize: 'md' },
    description: { fontSize: 'sm', color: 'fg.muted' },
    actionTrigger: { alignSelf: 'flex-start', fontSize: 'sm', fontWeight: 'semibold', color: 'accent.text', textDecoration: 'underline', cursor: 'pointer', _focusVisible: focusVisible },
    closeTrigger: { ...iconButton, position: 'absolute', top: '1', insetEnd: '1' },
  },
})
