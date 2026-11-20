import type { Coding, Questionnaire, QuestionnaireItem } from '@health/fhir-types'

import type { Path } from './model'

// *Becomes a record* (studio.md § The form designer; questionnaire-extraction.md; #497): what a
// question's answer becomes, read from and written to the package's Extraction Mapping, a
// Library of contained SDC templates. A template is about the question its
// `templateExtractContext` ends on; the designer shows its target resource and code, edits the
// code, adds an Observation for a question that has none, and removes one. Whatever else a
// template says (components, other values, its mode and Owner) is kept as read.

export const EXTRACT_CONTEXT = 'http://hl7.org/fhir/uv/sdc/StructureDefinition/sdc-questionnaire-templateExtractContext'
export const EXTRACT_VALUE = 'http://hl7.org/fhir/uv/sdc/StructureDefinition/sdc-questionnaire-templateExtractValue'
const PLATFORM = 'https://carapai.github.io/people-centered-healthcare/fhir'
export const EXTRACTION_TEMPLATE = `${PLATFORM}/StructureDefinition/extraction-template`
export const TARGET_QUESTIONNAIRE = `${PLATFORM}/StructureDefinition/extraction-target-questionnaire`
export const TARGET_VERSIONS = `${PLATFORM}/StructureDefinition/extraction-target-versions`
const PLATFORM_CODES = `${PLATFORM}/CodeSystem/platform`

/** A FHIR resource as JSON, as the mapping holds it. */
export type Json = Record<string, unknown>

interface Extension {
  url: string
  valueString?: string
  valueUri?: string
  valueCode?: string
  extension?: Extension[]
}

/** An Extraction Mapping: a Library holding the templates. */
export interface Mapping extends Json {
  resourceType: 'Library'
  contained?: Json[]
  relatedArtifact?: Json[]
  extension?: Extension[]
}

/** One record a question becomes: the template's id, its resource type and its code. */
export interface Target {
  template: string
  resourceType: string
  code: Coding | undefined
}

const extensions = (value: Json): Extension[] => (Array.isArray(value.extension) ? (value.extension as Extension[]) : [])

/** Whether `value` is the Extraction Mapping of the Questionnaire at `url`. */
export function isMappingOf(value: unknown, url: string | undefined): value is Mapping {
  if (typeof value !== 'object' || value === null || (value as Json).resourceType !== 'Library' || url === undefined) return false
  const library = value as Json
  const type = library.type as { coding?: Coding[] } | undefined
  const isMapping = (type?.coding ?? []).some((coding) => coding.code === 'extraction-mapping')
  return isMapping && extensions(library).some((ext) => ext.url === TARGET_QUESTIONNAIRE && ext.valueUri === url)
}

/** The linkId a template's `templateExtractContext` ends on, if it names one. */
export function contextLinkId(template: Json): string | undefined {
  const context = extensions(template).find((ext) => ext.url === EXTRACT_CONTEXT)?.valueString
  if (context === undefined) return undefined
  const found = [...context.matchAll(/linkId\s*=\s*'([^']*)'/g)]
  return found[found.length - 1]?.[1]
}

/** The records `linkId` becomes under `mapping`, in the mapping's order. */
export function targets(mapping: Mapping | undefined, linkId: string): Target[] {
  return (mapping?.contained ?? []).flatMap((template) => {
    if (contextLinkId(template) !== linkId || typeof template.id !== 'string') return []
    const code = (template.code as { coding?: Coding[] } | undefined)?.coding?.[0]
    return [{ template: template.id, resourceType: String(template.resourceType), code }]
  })
}

/** The mapping with template `id` given the code `coding` (its first coding; others are kept). */
export function withCode(mapping: Mapping, id: string, coding: Coding): Mapping {
  return {
    ...mapping,
    contained: (mapping.contained ?? []).map((template) => {
      if (template.id !== id) return template
      const current = (template.code as { coding?: Coding[] } | undefined) ?? {}
      const [, ...others] = current.coding ?? []
      const clean = Object.fromEntries(Object.entries(coding).filter(([, v]) => v !== undefined && v !== '')) as Coding
      return { ...template, code: { ...current, coding: [clean, ...others] } }
    }),
  }
}

/** The mapping without template `id` and the reference to it. */
export function withoutTarget(mapping: Mapping, id: string): Mapping {
  const contained = (mapping.contained ?? []).filter((template) => template.id !== id)
  const related = (mapping.relatedArtifact ?? []).filter((artifact) => (artifact.resourceReference as { reference?: string } | undefined)?.reference !== `#${id}`)
  const { contained: _c, relatedArtifact: _r, ...rest } = mapping
  return { ...rest, ...(contained.length === 0 ? {} : { contained }), ...(related.length === 0 ? {} : { relatedArtifact: related }) } as Mapping
}

/** The FHIRPath that selects the answered item at `path`, from the response down. */
export function contextPath(questionnaire: Questionnaire, path: Path): string | undefined {
  const steps: string[] = []
  let items = questionnaire.item
  for (const index of path) {
    const item: QuestionnaireItem | undefined = items?.[index]
    if (item === undefined) return undefined
    steps.push(item.linkId)
    items = item.item
  }
  if (steps.length === 0) return undefined
  return steps.map((linkId, n) => `item.where(linkId = '${linkId.replace(/'/g, "\\'")}'${n === steps.length - 1 ? ' and answer.exists()' : ''})`).join('.')
}

const extractValue = (expression: string): Extension => ({ url: EXTRACT_VALUE, valueString: expression })

/** A primitive element set to the answer. */
const primitive = (element: string) => ({ [`_${element}`]: { extension: [extractValue('answer.value')] } })

/** The Observation value the answer to an item of `type` fills, or undefined if none fits. */
function observationValue(type: string): Json | undefined {
  switch (type) {
    case 'decimal':
    case 'integer':
      return { valueQuantity: { _value: { extension: [extractValue('answer.value')] } } }
    case 'boolean':
      return primitive('valueBoolean')
    case 'string':
    case 'text':
      return primitive('valueString')
    case 'date':
    case 'dateTime':
      return primitive('valueDateTime')
    case 'time':
      return primitive('valueTime')
    case 'coding':
      return { valueCodeableConcept: { coding: [{ extension: [extractValue('answer.value')] }] } }
    case 'quantity':
      return { valueQuantity: { extension: [extractValue('answer.value')] } }
    default:
      return undefined
  }
}

/** Whether an answer of the item's type can become an Observation's value. */
export const recordable = (item: QuestionnaireItem) => observationValue(item.type) !== undefined

/**
 * The mapping with a new Observation the answer to the item at `path` becomes, coded `coding`:
 * managed, retracted as entered-in-error, accepted by clinical-data's record-observation, its
 * subject, encounter and time those of the response. Answers its template id.
 */
export function addObservation(mapping: Mapping, questionnaire: Questionnaire, path: Path, coding: Coding): { mapping: Mapping; template: string } | undefined {
  let item: QuestionnaireItem | undefined
  let items = questionnaire.item
  for (const index of path) {
    item = items?.[index]
    items = item?.item
  }
  const context = contextPath(questionnaire, path)
  const value = item === undefined ? undefined : observationValue(item.type)
  if (item === undefined || context === undefined || value === undefined) return undefined
  const taken = new Set((mapping.contained ?? []).map((template) => template.id))
  const base = item.linkId.replace(/[^A-Za-z0-9\-.]/g, '-').slice(0, 60) || 'record'
  let id = base
  for (let n = 2; taken.has(id); n += 1) id = `${base}-${n}`
  const clean = Object.fromEntries(Object.entries(coding).filter(([, v]) => v !== undefined && v !== '')) as Coding
  const template: Json = {
    resourceType: 'Observation',
    id,
    extension: [
      {
        url: EXTRACTION_TEMPLATE,
        extension: [
          { url: 'mode', valueCode: 'managed' },
          { url: 'retraction', valueCode: 'entered-in-error' },
          { url: 'ownerUseCase', valueString: 'clinical-data.record-observation' },
        ],
      },
      { url: EXTRACT_CONTEXT, valueString: context },
    ],
    status: 'final',
    code: { coding: [clean] },
    subject: { extension: [extractValue('%resource.subject')] },
    encounter: { extension: [extractValue('%resource.encounter')] },
    _effectiveDateTime: { extension: [extractValue('%resource.authored')] },
    ...value,
  }
  return {
    template: id,
    mapping: {
      ...mapping,
      contained: [...(mapping.contained ?? []), template],
      relatedArtifact: [...(mapping.relatedArtifact ?? []), { type: 'composed-of', resourceReference: { reference: `#${id}` } }],
    },
  }
}

/** A new Extraction Mapping for `questionnaire`, applying to its major version, with no templates. */
export function blankMapping(questionnaire: Questionnaire): Mapping {
  const url = questionnaire.url ?? ''
  const version = questionnaire.version ?? '1.0.0'
  const major = Number.parseInt(version.split('.')[0] ?? '1', 10)
  const range = Number.isNaN(major) ? version : `>=${major}.0.0 <${major + 1}.0.0`
  const name = questionnaire.name ?? 'Form'
  return {
    resourceType: 'Library',
    url: url.includes('/Questionnaire/') ? `${url.replace('/Questionnaire/', '/Library/')}-mapping` : `${url}-mapping`,
    version,
    name: `${name}Mapping`,
    title: `${questionnaire.title ?? name}: what the answers become`,
    status: questionnaire.status,
    type: { coding: [{ system: PLATFORM_CODES, code: 'extraction-mapping' }] },
    extension: [
      { url: TARGET_QUESTIONNAIRE, valueUri: url },
      { url: TARGET_VERSIONS, valueString: range },
    ],
  }
}

/** Where a new Extraction Mapping for the Questionnaire at `path` goes. */
export const newMappingPath = (path: string) => `Library-${(path.split('/').pop() ?? path).replace(/^Questionnaire-/, '').replace(/\.json$/, '')}-mapping.json`

/** A mapping problem: a record of a question with no code, or one whose question is gone. */
export type RecordProblem = { code: 'record-no-code'; linkId: string; template: string } | { code: 'record-orphan'; linkId: string; template: string }

/** The mapping's problems against the form as it stands. */
export function recordProblems(mapping: Mapping | undefined, questionnaire: Questionnaire): RecordProblem[] {
  if (mapping === undefined) return []
  const ids = new Set<string>()
  const walk = (items: QuestionnaireItem[] | undefined) =>
    (items ?? []).forEach((item) => {
      ids.add(item.linkId)
      walk(item.item)
    })
  walk(questionnaire.item)
  return (mapping.contained ?? []).flatMap((template): RecordProblem[] => {
    const linkId = contextLinkId(template)
    if (linkId === undefined || typeof template.id !== 'string') return []
    if (!ids.has(linkId)) return [{ code: 'record-orphan', linkId, template: template.id }]
    const code = (template.code as { coding?: Coding[] } | undefined)?.coding?.[0]
    return code?.code === undefined || code.code === '' ? [{ code: 'record-no-code', linkId, template: template.id }] : []
  })
}

/** A linkId as a FHIRPath string literal. */
const quoted = (id: string) => `'${id.replace(/'/g, "\\'")}'`

/** The mapping with every template context naming `from` naming `to` instead (a new id renamed). */
export function renameInMapping(mapping: Mapping | undefined, from: string, to: string): Mapping | undefined {
  if (mapping === undefined || from === to) return mapping
  const source = `linkId(\\s*)=(\\s*)${quoted(from).replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`
  const pattern = new RegExp(source, 'g')
  const names = new RegExp(source)
  let changed = false
  const contained = (mapping.contained ?? []).map((template) => {
    const list = extensions(template)
    if (!list.some((ext) => ext.url === EXTRACT_CONTEXT && ext.valueString !== undefined && names.test(ext.valueString))) return template
    changed = true
    return { ...template, extension: list.map((ext) => (ext.url === EXTRACT_CONTEXT && ext.valueString !== undefined ? { ...ext, valueString: ext.valueString.replace(pattern, (_m, a: string, b: string) => `linkId${a}=${b}${quoted(to)}`) } : ext)) }
  })
  return changed ? { ...mapping, contained } : mapping
}
