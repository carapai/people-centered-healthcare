// Themes and density are attributes on any element, usually <html>; a subtree can set its own
// (client-application.md § Design system).
export const conditions = {
  dark: '[data-theme=dark] &',
  contrast: '[data-theme=high-contrast] &',
  compact: '[data-density=compact] &',
}
