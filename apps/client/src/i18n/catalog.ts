import { englishMessages as designSystem } from '@health/design-system'

import { accessMessages, edgeStatusMessages, enrollmentMessages, consultationMessages, formMessages, messages, pharmacyMessages, receptionMessages, worklistMessages } from './messages'

/** `id → ICU message` for English: the app's messages and the design system's. */
export const englishCatalog: Record<string, string> = {
  ...designSystem,
  ...Object.fromEntries([...Object.values(messages), ...Object.values(edgeStatusMessages), ...Object.values(enrollmentMessages), ...Object.values(accessMessages), ...Object.values(worklistMessages), ...Object.values(formMessages), ...Object.values(receptionMessages), ...Object.values(consultationMessages), ...Object.values(pharmacyMessages)].map((m) => [m.id, m.defaultMessage as string])),
}
