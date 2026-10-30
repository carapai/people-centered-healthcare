import { defineTokens } from '@pandacss/dev'

// Panda 2's presets have no z-index scale; without these, `zIndex: 'modal'` is emitted verbatim
// and ignored by the browser.
export const tokens = defineTokens({
  zIndex: {
    base: { value: 0 },
    sticky: { value: 100 },
    dropdown: { value: 1000 },
    overlay: { value: 1300 },
    modal: { value: 1400 },
    toast: { value: 1700 },
  },
})
