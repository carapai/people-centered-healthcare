import type { StudioDesign, StudioDesignKind, StudioDesignPackage, StudioDesigns, StudioProblem } from '@health/api-client'

import type { Transport } from '../../platform/types'
import { StudioError, studioKey } from './api'
import { newAgeBandSet } from './indicators/ageBands'
import { newIndicator } from './indicators/indicator'
import { newArtefact } from './programs/model'
import { addTable } from './workflows/decisions'
import { readDefinition, writeDefinition } from './workflows/model'

// What the Studio designs, across packages (studio.md § The Studio home; #855): the Studio starts
// from forms, workflows, concepts and decision tables, and the packages they live in stay behind
// them. Central lists every design in the package version it opens in (`GET /api/studio/designs`),
// with each package's published version and open draft; this module sorts them into sections,
// says where each stands, and works out what *Edit* and *New* need. It decides nothing the
// server checks: starting a draft and saving go through the same API as the package workspace.

export type { StudioDesign, StudioDesignKind, StudioDesignPackage, StudioDesigns } from '@health/api-client'

export const designsKey = ['studio', 'designs'] as const

/** Every design and the packages that hold them. */
export function designsQuery(transport: Transport) {
  return {
    queryKey: designsKey,
    queryFn: async ({ signal }: { signal?: AbortSignal } = {}): Promise<StudioDesigns> => {
      const response = await transport.request({ method: 'GET', path: '/studio/designs', ...(signal === undefined ? {} : { signal }) })
      const { parseResponse } = await import('@health/api-client')
      const value = parseResponse('listStudioDesigns', response.status, response.body)
      if (response.status === 200) return value as StudioDesigns
      const problem = value as StudioProblem | null
      throw new StudioError(response.status, problem?.detail ?? '', problem?.refusal ?? null)
    },
  }
}

/** An ICU select key for `id` (`decision-tables` gives `decisionTables`): ICU keys have no `-`. */
export const icu = (id: string) => id.replace(/-(\w)/g, (_, c: string) => c.toUpperCase())

/** The Studio's sections: what a person designs. */
export type SectionId = 'forms' | 'workflows' | 'concepts' | 'decision-tables' | 'programs' | 'age-bands' | 'indicators'

/** Each section and the kinds of design it lists, in the order the home shows them. */
export const sections: readonly { id: SectionId; kinds: readonly StudioDesignKind[]; creates: boolean }[] = [
  { id: 'forms', kinds: ['form'], creates: true },
  { id: 'workflows', kinds: ['workflow'], creates: true },
  { id: 'concepts', kinds: ['code-system', 'value-set'], creates: true },
  { id: 'decision-tables', kinds: ['decision-table'], creates: true },
  { id: 'programs', kinds: ['program'], creates: false },
  { id: 'age-bands', kinds: ['age-band-set'], creates: true },
  { id: 'indicators', kinds: ['indicator'], creates: true },
]

export const sectionOf = (id: SectionId) => sections.find((s) => s.id === id) ?? sections[0]!

/** The designs a section lists. */
export const inSection = (designs: readonly StudioDesign[], id: SectionId) => designs.filter((d) => sectionOf(id).kinds.includes(d.kind))

/** Where a design stands for the person: published, in a draft they hold, in someone else's
 * draft, or submitted and on its way to publication. */
export type DesignStatus = 'published' | 'yours' | 'theirs' | 'submitted'

export function statusOf(pkg: StudioDesignPackage | undefined): DesignStatus {
  const draft = pkg?.draft
  if (draft === null || draft === undefined) return 'published'
  if (draft.stage !== 'draft') return 'submitted'
  return draft.mine ? 'yours' : 'theirs'
}

export const packageOf = (designs: StudioDesigns, name: string) => designs.packages.find((p) => p.name === name)

export interface DesignFilter {
  text: string
  /** A package name, or '' for every package. */
  package: string
  /** A status, or '' for any. */
  status: DesignStatus | ''
}

export const noFilter: DesignFilter = { text: '', package: '', status: '' }

/** The designs that match `filter`: every word in the title, path, url or package, the package
 * and the status. */
export function filterDesigns(designs: StudioDesigns, list: readonly StudioDesign[], filter: DesignFilter): StudioDesign[] {
  const words = filter.text.toLowerCase().split(/\s+/).filter((w) => w !== '')
  return list.filter((d) => {
    const pkg = packageOf(designs, d.package)
    if (filter.package !== '' && d.package !== filter.package) return false
    if (filter.status !== '' && statusOf(pkg) !== filter.status) return false
    const haystack = [d.title, d.path, d.url ?? '', d.package, pkg?.title ?? ''].join(' ').toLowerCase()
    return words.every((w) => haystack.includes(w))
  })
}

/** The editor a design opens in, as a route and its params. */
export function editorOf(design: Pick<StudioDesign, 'kind' | 'path' | 'table'>, id: string) {
  switch (design.kind) {
    case 'form':
      return { to: '/studio/$id/forms/$', params: { id, _splat: design.path } } as const
    case 'workflow':
    case 'decision-table':
      return { to: '/studio/$id/workflows/$', params: { id, _splat: design.path }, search: design.table === null || design.table === undefined ? {} : { table: design.table } } as const
    case 'code-system':
    case 'value-set':
      return { to: '/studio/$id/terminology/$', params: { id, _splat: design.path } } as const
    case 'program':
      return { to: '/studio/$id/programs/$', params: { id, _splat: design.path } } as const
    case 'age-band-set':
      return { to: '/studio/$id/age-bands/$', params: { id, _splat: design.path } } as const
    case 'indicator':
      return { to: '/studio/$id/indicators/$', params: { id, _splat: design.path } } as const
  }
}

/** What *Edit* does for package `name`: reopen the draft the person holds, start the next
 * version of its published one, or nothing, because someone else holds its draft or it is
 * submitted. */
export type EditPlan = { kind: 'reuse'; id: string } | { kind: 'start'; copyOf: string } | { kind: 'held'; holder: string } | { kind: 'submitted'; id: string } | { kind: 'unknown' }

export function editPlan(pkg: StudioDesignPackage | undefined): EditPlan {
  if (pkg === undefined) return { kind: 'unknown' }
  const draft = pkg.draft
  if (draft !== null && draft !== undefined) {
    if (draft.stage !== 'draft') return { kind: 'submitted', id: draft.id }
    return draft.mine ? { kind: 'reuse', id: draft.id } : { kind: 'held', holder: draft.holder }
  }
  if (pkg.published !== null && pkg.published !== undefined) return { kind: 'start', copyOf: pkg.published }
  return { kind: 'unknown' }
}

/** Where *New* may put a design: a draft the person holds, or a published package whose next
 * version they start. A package someone else is drafting, or that is submitted, is left out. */
export interface PackageChoice {
  name: string
  title: string
  plan: { kind: 'reuse'; id: string } | { kind: 'start'; copyOf: string }
}

export function packageChoices(designs: StudioDesigns): PackageChoice[] {
  return designs.packages.flatMap((pkg) => {
    const plan = editPlan(pkg)
    if (plan.kind !== 'reuse' && plan.kind !== 'start') return []
    return [{ name: pkg.name, title: pkg.title ?? pkg.name, plan }]
  })
}

/** An id from a title: lower case, words joined by `-`. */
export function slug(title: string): string {
  return (
    title
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'new'
  )
}

/** What *New* creates, for a section: a form, a workflow, a code system, a value set, an Age
 * Band Set or an indicator. A decision table goes into a workflow instead ([`withNewTable`]). */
export type NewKind = 'form' | 'workflow' | 'code-system' | 'value-set' | 'age-band-set' | 'indicator'

/** A new, empty design of `kind` titled `title`, at a top-level path the package does not use
 * yet, its url under the package's canonical base and its version the package's. */
export function newDesign(kind: NewKind, title: string, canonical: string, packageVersion: string, taken: readonly string[]): { path: string; resource: Record<string, unknown> } {
  const id = slug(title)
  const type = { form: 'Questionnaire', workflow: 'Library', 'code-system': 'CodeSystem', 'value-set': 'ValueSet', 'age-band-set': 'Library', indicator: 'Measure' }[kind]
  const url = `${canonical.replace(/\/+$/, '')}/${type}/${id}`
  if (kind === 'form' || kind === 'workflow') {
    const made = newArtefact({ kind: kind === 'form' ? 'Questionnaire' : 'Library', canonical: url, url, pinned: null, where: 'missing', path: null, version: null }, packageVersion, [...taken])
    return { path: made.path, resource: { ...made.resource, title: title.trim() } }
  }
  const base = `${type}-${id}`
  let path = `${base}.json`
  for (let n = 2; taken.includes(path); n += 1) path = `${base}-${n}.json`
  const name = id
    .split('-')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('')
  const common = { id, url, version: packageVersion, name: /^[A-Za-z]/.test(name) ? name : `X${name}`, title: title.trim(), status: 'draft' }
  if (kind === 'age-band-set') return { path, resource: newAgeBandSet(common) }
  if (kind === 'indicator') return { path, resource: newIndicator(common) }
  return kind === 'code-system'
    ? { path, resource: { resourceType: 'CodeSystem', ...common, content: 'complete', concept: [] } }
    : { path, resource: { resourceType: 'ValueSet', ...common, compose: { include: [] } } }
}

/** The Workflow Definition Library `text` with a new decision table, whose id comes from
 * `title`; or why it cannot take one. */
export function withNewTable(text: string, title: string): { content: string; table: string } | { refusal: string } {
  const read = readDefinition(text)
  if ('refusal' in read) return { refusal: read.refusal }
  const tables = read.definition.decisionTables ?? []
  const wanted = slug(title)
  let id = wanted
  for (let n = 2; tables.some((t) => t.id === id); n += 1) id = `${wanted}-${n}`
  const added = addTable(read.definition)
  const definition = { ...added.definition, decisionTables: (added.definition.decisionTables ?? []).map((t, i) => (i === added.index ? { ...t, id } : t)) }
  return { content: JSON.stringify(writeDefinition(read.library, definition), null, 2), table: id }
}

/** Every Studio read a new design or a new draft may have changed. */
export const changedKeys = [designsKey, studioKey] as const
