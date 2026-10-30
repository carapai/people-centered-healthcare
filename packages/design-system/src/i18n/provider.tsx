import { LocaleProvider } from '@ark-ui/react/locale'
import type { ReactNode } from 'react'
import { IntlProvider } from 'react-intl'

export interface I18nProviderProps {
  /** A BCP 47 tag; it also sets Ark's locale (date formats, reading direction). */
  locale: string
  /** `id → ICU message` for the locale: the app's catalog merged with the design system's. */
  messages: Record<string, string>
  children: ReactNode
}

/** ICU messages (react-intl) and Ark's locale, from the first screen. */
export function I18nProvider({ locale, messages, children }: I18nProviderProps) {
  return (
    <IntlProvider locale={locale} defaultLocale="en" messages={messages}>
      <LocaleProvider locale={locale}>{children}</LocaleProvider>
    </IntlProvider>
  )
}
