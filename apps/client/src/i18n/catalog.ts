import { englishMessages as designSystem } from '@health/design-system'

import { edgeStatusMessages, messages } from './messages'

/** `id → ICU message` for English: the app's messages and the design system's. */
export const englishCatalog: Record<string, string> = {
  ...designSystem,
  ...Object.fromEntries([...Object.values(messages), ...Object.values(edgeStatusMessages)].map((m) => [m.id, m.defaultMessage as string])),
}
