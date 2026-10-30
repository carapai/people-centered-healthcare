import { defineSemanticTokens } from '@pandacss/dev'

// Light, dark and high-contrast values for every colour the recipes use. Text pairs meet WCAG 2.2
// AA 4.5:1 and component boundaries 3:1 in each theme; the axe test checks them on the example
// page.
export const semanticTokens = defineSemanticTokens({
  colors: {
    bg: {
      canvas: { value: { base: '{colors.white}', _dark: '{colors.gray.950}', _contrast: '{colors.white}' } },
      surface: { value: { base: '{colors.white}', _dark: '{colors.gray.900}', _contrast: '{colors.white}' } },
      subtle: { value: { base: '{colors.gray.100}', _dark: '{colors.gray.800}', _contrast: '{colors.white}' } },
      hover: { value: { base: '{colors.gray.100}', _dark: '{colors.gray.800}', _contrast: '{colors.gray.200}' } },
      selected: { value: { base: '{colors.blue.50}', _dark: '{colors.blue.950}', _contrast: '{colors.black}' } },
      backdrop: { value: { base: 'rgb(3 7 18 / 0.5)', _dark: 'rgb(0 0 0 / 0.7)', _contrast: 'rgb(0 0 0 / 0.8)' } },
    },
    fg: {
      default: { value: { base: '{colors.gray.900}', _dark: '{colors.gray.50}', _contrast: '{colors.black}' } },
      muted: { value: { base: '{colors.gray.600}', _dark: '{colors.gray.400}', _contrast: '{colors.black}' } },
      selected: { value: { base: '{colors.blue.800}', _dark: '{colors.blue.200}', _contrast: '{colors.white}' } },
      disabled: { value: { base: '{colors.gray.500}', _dark: '{colors.gray.500}', _contrast: '{colors.gray.700}' } },
    },
    border: {
      default: { value: { base: '{colors.gray.300}', _dark: '{colors.gray.700}', _contrast: '{colors.black}' } },
      // Inputs and controls: 3:1 against their background (WCAG 1.4.11).
      control: { value: { base: '{colors.gray.500}', _dark: '{colors.gray.500}', _contrast: '{colors.black}' } },
    },
    accent: {
      default: { value: { base: '{colors.blue.700}', _dark: '{colors.blue.300}', _contrast: '{colors.blue.900}' } },
      emphasized: { value: { base: '{colors.blue.800}', _dark: '{colors.blue.200}', _contrast: '{colors.black}' } },
      fg: { value: { base: '{colors.white}', _dark: '{colors.gray.950}', _contrast: '{colors.white}' } },
      text: { value: { base: '{colors.blue.700}', _dark: '{colors.blue.300}', _contrast: '{colors.blue.900}' } },
    },
    focusRing: { value: { base: '{colors.blue.600}', _dark: '{colors.blue.300}', _contrast: '{colors.black}' } },
    danger: {
      text: { value: { base: '{colors.red.700}', _dark: '{colors.red.400}', _contrast: '{colors.red.900}' } },
      border: { value: { base: '{colors.red.700}', _dark: '{colors.red.400}', _contrast: '{colors.red.900}' } },
      subtle: { value: { base: '{colors.red.50}', _dark: '{colors.red.950}', _contrast: '{colors.white}' } },
    },
    success: {
      text: { value: { base: '{colors.green.800}', _dark: '{colors.green.400}', _contrast: '{colors.green.900}' } },
      subtle: { value: { base: '{colors.green.50}', _dark: '{colors.green.950}', _contrast: '{colors.white}' } },
    },
  },
  // Comfortable suits tablets (44 px touch targets); compact suits 1366×768 desktops and still
  // clears the WCAG 2.5.8 24 px minimum.
  sizes: {
    control: { value: { base: '2.75rem', _compact: '2rem' } },
    cell: { value: { base: '2.75rem', _compact: '2rem' } },
  },
  spacing: {
    controlX: { value: { base: '{spacing.3}', _compact: '{spacing.2}' } },
    gap: { value: { base: '{spacing.2}', _compact: '{spacing.1}' } },
    surface: { value: { base: '{spacing.6}', _compact: '{spacing.4}' } },
  },
  fontSizes: {
    control: { value: { base: '{fontSizes.md}', _compact: '{fontSizes.sm}' } },
  },
})
