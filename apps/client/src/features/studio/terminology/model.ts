import type { StudioConceptEditInput } from '@health/api-client'
import type { ValueSet, ValueSetCompose, ValueSetComposeInclude } from '@health/fhir-types'

// A draft ValueSet's compose, edited from the terminology browser (studio.md § Terminology;
// #523). Each change returns a new ValueSet; what this module does not edit (other filters,
// imported value sets, extensions) is kept as read. The server expands the result, so nothing
// here works out which codes a rule selects beyond the codes it names.

/** A rule of the compose, as the browser lists it. */
export type Rule = {
  kind: 'include' | 'exclude'
  /** Its position among the includes or the excludes. */
  index: number
  system: string | undefined
  version: string | undefined
  /** Codes listed by hand. */
  codes: { code: string; display: string | undefined }[]
  /** Its `is-a` filter's code: the concept and everything under it. */
  under: string | undefined
  /** Whether it has filters or imports this browser does not edit. */
  other: boolean
}

type Include = ValueSetComposeInclude

const includes = (vs: ValueSet): Include[] => vs.compose?.include ?? []
const excludes = (vs: ValueSet): Include[] => vs.compose?.exclude ?? []

function withCompose(vs: ValueSet, include: Include[], exclude: Include[]): ValueSet {
  const compose: ValueSetCompose = { ...(vs.compose ?? { include: [] }), include }
  if (exclude.length === 0) delete compose.exclude
  else compose.exclude = exclude
  return { ...vs, compose }
}

/** `filter` is exactly one `concept is-a <code>`. */
function isA(rule: Include): string | undefined {
  const filters = rule.filter ?? []
  const only = filters[0]
  return filters.length === 1 && only !== undefined && only.property === 'concept' && only.op === 'is-a' ? only.value : undefined
}

/** A rule that lists codes of `system` and does nothing else. */
const listing = (rule: Include, system: string) => rule.system === system && (rule.filter ?? []).length === 0 && (rule.valueSet ?? []).length === 0 && (rule.concept ?? []).length > 0

/** The compose's rules, includes first. */
export function rules(vs: ValueSet): Rule[] {
  const read = (kind: Rule['kind']) => (rule: Include, index: number): Rule => {
    const under = isA(rule)
    return {
      kind,
      index,
      system: rule.system,
      version: rule.version,
      codes: (rule.concept ?? []).map((c) => ({ code: c.code, display: c.display })),
      under,
      other: (under === undefined && (rule.filter ?? []).length > 0) || (rule.valueSet ?? []).length > 0,
    }
  }
  return [...includes(vs).map(read('include')), ...excludes(vs).map(read('exclude'))]
}

/** `rules` with `code` of `system` taken out of every listing, and listings left empty dropped. */
function unlisted(list: Include[], system: string, code: string): Include[] {
  return list.flatMap((rule) => {
    if (rule.system !== system || !(rule.concept ?? []).some((c) => c.code === code)) return [rule]
    const concept = (rule.concept ?? []).filter((c) => c.code !== code)
    if (concept.length === 0 && (rule.filter ?? []).length === 0 && (rule.valueSet ?? []).length === 0) return []
    return [{ ...rule, concept }]
  })
}

/** Adds `code` of `system` alone, and takes it out of the excludes. */
export function includeCode(vs: ValueSet, system: string, code: string, display?: string): ValueSet {
  const exclude = unlisted(excludes(vs), system, code)
  const include = includes(vs)
  if (include.some((r) => r.system === system && (r.concept ?? []).some((c) => c.code === code))) return withCompose(vs, include, exclude)
  const listed = { code, ...(display === undefined ? {} : { display }) }
  const at = include.findIndex((r) => listing(r, system))
  const next = at === -1 ? [...include, { system, concept: [listed] }] : include.map((r, i) => (i === at ? { ...r, concept: [...(r.concept ?? []), listed] } : r))
  return withCompose(vs, next, exclude)
}

/** Adds `code` of `system` with everything under it, as an `is-a` filter, so codes added under
 * it later follow; it is no longer excluded or listed on its own. */
export function includeUnder(vs: ValueSet, system: string, code: string): ValueSet {
  const exclude = unlisted(excludes(vs), system, code)
  const include = unlisted(includes(vs), system, code)
  if (include.some((r) => r.system === system && isA(r) === code)) return withCompose(vs, include, exclude)
  return withCompose(vs, [...include, { system, filter: [{ property: 'concept', op: 'is-a', value: code }] }], exclude)
}

/** Takes `code` of `system` out: it is no longer listed, and when another rule (a filter, or the
 * whole system) could still select it, it is excluded. */
export function excludeCode(vs: ValueSet, system: string, code: string, display?: string): ValueSet {
  const include = unlisted(includes(vs), system, code).filter((r) => !(r.system === system && isA(r) === code))
  const stillSelected = include.some((r) => r.system === system && (r.concept ?? []).length === 0)
  const exclude = excludes(vs)
  if (!stillSelected || exclude.some((r) => r.system === system && (r.concept ?? []).some((c) => c.code === code))) return withCompose(vs, include, exclude)
  const listed = { code, ...(display === undefined ? {} : { display }) }
  const at = exclude.findIndex((r) => listing(r, system))
  const next = at === -1 ? [...exclude, { system, concept: [listed] }] : exclude.map((r, i) => (i === at ? { ...r, concept: [...(r.concept ?? []), listed] } : r))
  return withCompose(vs, include, next)
}

/** Removes a whole rule. */
export function removeRule(vs: ValueSet, kind: Rule['kind'], index: number): ValueSet {
  const keep = (list: Include[]) => list.filter((_, i) => i !== index)
  return kind === 'include' ? withCompose(vs, keep(includes(vs)), excludes(vs)) : withCompose(vs, includes(vs), keep(excludes(vs)))
}

/** Removes one listed code from a rule, and the rule once it lists nothing and does nothing else. */
export function removeListed(vs: ValueSet, kind: Rule['kind'], index: number, code: string): ValueSet {
  const edit = (list: Include[]) =>
    list.flatMap((rule, i) => {
      if (i !== index) return [rule]
      const concept = (rule.concept ?? []).filter((c) => c.code !== code)
      return concept.length === 0 && (rule.filter ?? []).length === 0 && (rule.valueSet ?? []).length === 0 ? [] : [{ ...rule, concept }]
    })
  return kind === 'include' ? withCompose(vs, edit(includes(vs)), excludes(vs)) : withCompose(vs, includes(vs), edit(excludes(vs)))
}

/** The key a code is ticked under. */
export const codeKey = (system: string, code: string) => `${system}|${code}`

/** A new draft ValueSet named `name` at `version`, its URL beside `sibling`'s (another canonical
 * of the package, e.g. its CodeSystem's): `…/CodeSystem/x` gives `…/ValueSet/<name>`. */
export function newValueSet(name: string, title: string, version: string, sibling: string | undefined): ValueSet {
  const base = sibling?.replace(/\/[A-Za-z]+\/[^/]+$/, '') ?? 'urn:draft'
  const id = name
    .trim()
    .replace(/[^A-Za-z0-9-.]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return {
    resourceType: 'ValueSet',
    id,
    url: `${base}/ValueSet/${id}`,
    version,
    name: id
      .split(/[^A-Za-z0-9]+/)
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(''),
    ...(title.trim() === '' ? {} : { title: title.trim() }),
    status: 'draft',
    compose: { include: [] },
  }
}

/** Where a new ValueSet `vs` goes in the package. */
export const valueSetPath = (vs: ValueSet) => `ValueSet-${vs.id ?? 'new'}.json`

/** The widget a ValueSet of `codes` codes suggests: radio buttons up to 7, a drop-down up to 30,
 * and search-as-you-type beyond (studio.md § Terminology, Binding from the form designer). */
export function suggestWidget(codes: number): 'radio' | 'select' | 'search' {
  if (codes <= 7) return 'radio'
  if (codes <= 30) return 'select'
  return 'search'
}

/** What a binding records: `url|version`. */
export const bindingOf = (url: string, version: string | null | undefined) => (version === null || version === undefined || version === '' ? url : `${url}|${version}`)

// Editing a local CodeSystem's concepts (#524): the server applies each edit, keeps a published
// code fixed, and works out the version; these build what is sent and what the form offers.

/** What the concept form offers: a published (fixed) code keeps its code and parent and can only
 * be retired, once; a new code can be renamed, moved or deleted. */
export function conceptActions(concept: { fixed: boolean; inactive: boolean }) {
  return { rename: !concept.fixed, move: !concept.fixed, remove: !concept.fixed, retire: concept.fixed && !concept.inactive }
}

/** The concept form's fields, as typed. */
export type ConceptFields = { code: string; display: string; definition: string; parent: string; note: string }

const given = (text: string) => {
  const trimmed = text.trim()
  return trimmed === '' ? undefined : trimmed
}

const shared = (fields: ConceptFields) => {
  const display = given(fields.display)
  const definition = given(fields.definition)
  const parent = given(fields.parent)
  const note = given(fields.note)
  return { ...(display === undefined ? {} : { display }), ...(definition === undefined ? {} : { definition }), ...(parent === undefined ? {} : { parent }), ...(note === undefined ? {} : { note }) }
}

/** A new code in the CodeSystem at `path`. */
export function addCode(path: string, fields: ConceptFields): StudioConceptEditInput {
  return { path, edit: 'add', code: fields.code.trim(), ...shared(fields) }
}

/** Code `code` changed as the form says: its parent is sent as it stands, so an emptied one
 * moves a new code to the top level. */
export function changeCode(path: string, code: string, fields: ConceptFields): StudioConceptEditInput {
  const newCode = fields.code.trim()
  return { path, edit: 'change', code, ...(newCode === '' || newCode === code ? {} : { newCode }), ...shared(fields) }
}

/** Code `code` deleted (a new one) or retired (a published one), with an optional note. */
export function dropCode(path: string, edit: 'delete' | 'retire', code: string, note: string): StudioConceptEditInput {
  const said = given(note)
  return { path, edit, code, ...(said === undefined ? {} : { note: said }) }
}
