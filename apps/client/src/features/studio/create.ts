import type { StudioNewDraft, StudioPackagePage } from '@health/api-client'

import type { Transport } from '../../platform/types'
import { fileQuery, packageQuery, saveFile, startDraft } from './api'
import { newDesign, withNewTable, type NewKind } from './designs'

// *Edit* and *New* (#855): the draft a design goes into, started or reused through the package
// workspace's own API (#494, #837), and the new design saved through the file editor's
// `If-Match` path. The server checks everything again.

/** The draft to work in: one the person holds, the next version of a published package, or a
 * blank new package. */
export type DraftPlan = { kind: 'reuse'; id: string } | { kind: 'start'; copyOf: string } | { kind: 'blank'; draft: StudioNewDraft }

/** The draft `plan` names, started if it must be. */
export async function openDraft(transport: Transport, plan: DraftPlan): Promise<StudioPackagePage> {
  switch (plan.kind) {
    case 'reuse':
      return packageQuery(transport, plan.id).queryFn()
    case 'start':
      return startDraft(transport, { copyOf: plan.copyOf })
    case 'blank':
      return startDraft(transport, plan.draft)
  }
}

/** The canonical base of a package version, from its manifest's URL (`…/Library/x` gives `…`). */
export function canonicalOf(page: StudioPackagePage): string | null {
  const manifest = page.files.find((f) => f.path === 'Library-manifest.json')?.url
  const at = manifest?.lastIndexOf('/Library/') ?? -1
  return manifest === undefined || manifest === null || at < 0 ? null : manifest.slice(0, at)
}

/** What was created: the draft it went into, its file, and a decision table's id. */
export interface Created {
  page: StudioPackagePage
  path: string
  table?: string
}

/** Creates a design of `kind` titled `title` in the draft `plan` names, and saves it. */
export async function createDesign(transport: Transport, plan: DraftPlan, kind: NewKind, title: string, canonical: string | null): Promise<Created> {
  const page = await openDraft(transport, plan)
  const base = canonical ?? canonicalOf(page) ?? (plan.kind === 'blank' ? (plan.draft.canonical ?? null) : null)
  if (base === null || base === '') throw new Error('the package has no canonical base URL')
  const made = newDesign(
    kind,
    title,
    base,
    page.package.version,
    page.files.map((f) => f.path),
  )
  const saved = await saveFile(transport, page.package.id, made.path, JSON.stringify(made.resource, null, 2), page.revision)
  return { page: saved, path: made.path }
}

/** Adds a decision table titled `title` to the workflow at `path` in the draft `plan` names. */
export async function createTable(transport: Transport, plan: DraftPlan, path: string, title: string): Promise<Created> {
  const page = await openDraft(transport, plan)
  const file = await fileQuery(transport, page.package.id, path).queryFn()
  const added = withNewTable(file.content, title)
  if ('refusal' in added) throw new Error(`${path} is not a workflow the Studio can edit (${added.refusal})`)
  const saved = await saveFile(transport, page.package.id, path, added.content, file.revision)
  return { page: saved, path, table: added.table }
}
