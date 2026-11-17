// PROTOTYPE (#544), throwaway: the Report designer layout variants. Never merged to main.
import { createFileRoute } from '@tanstack/react-router'

import { ReportDesignerPrototype } from '../features/studio/prototype/ReportDesignerPrototype'

export const Route = createFileRoute('/prototype/report-designer')({
  component: ReportDesignerPrototype,
})
