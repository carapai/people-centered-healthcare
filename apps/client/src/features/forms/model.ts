import type {
  Coding,
  Extension,
  Questionnaire,
  QuestionnaireItem,
  QuestionnaireItemEnableWhen,
  QuestionnaireResponse,
  QuestionnaireResponseItem,
  QuestionnaireResponseItemAnswer,
} from '@health/fhir-types'
import { evaluateRaw, parseLossless, toItem } from '@health/fhirpath-conformance'
import { LosslessNumber, stringify } from 'lossless-json'

// The QuestionnaireRenderer's model (client-application.md § Forms; ADRs 0038, 0052, 0054): a
// QuestionnaireResponse built item by item from its Questionnaire, with live behaviour
// evaluated by fhirpath.js in lossless precise mode. Everything here is a preview: the server
// re-validates every submission and recomputes calculated values (ADR 0052).

const SDC = 'http://hl7.org/fhir/uv/sdc/StructureDefinition'
export const ENABLE_WHEN_EXPRESSION = `${SDC}/sdc-questionnaire-enableWhenExpression`
export const CALCULATED_EXPRESSION = `${SDC}/sdc-questionnaire-calculatedExpression`
export const CONSTRAINT = 'http://hl7.org/fhir/StructureDefinition/questionnaire-constraint'

/** The question types the renderer answers. */
export const ANSWERED = ['boolean', 'decimal', 'integer', 'date', 'dateTime', 'string', 'text', 'coding'] as const
export type AnsweredType = (typeof ANSWERED)[number]

/** A new stable `item.id` for a repeat instance: it never changes while the response is edited. */
export const newItemId = () => crypto.randomUUID()

function expressionOf(extensions: Extension[] | undefined, url: string): string | undefined {
  const found = extensions?.find((e) => e.url === url) as { valueExpression?: { expression?: string } } | undefined
  return found?.valueExpression?.expression
}

/** An empty response item for `item`, with its children. */
function blank(item: QuestionnaireItem): QuestionnaireResponseItem {
  const response: QuestionnaireResponseItem = { linkId: item.linkId, id: newItemId() }
  if (item.text !== undefined) response.text = item.text
  if (item.type === 'group' && item.item !== undefined) response.item = item.item.filter((child) => child.type !== 'display').map(blank)
  return response
}

/** A new, in-progress response to `questionnaire` (`url|version`). */
export function initialResponse(questionnaire: Questionnaire): QuestionnaireResponse {
  const canonical = questionnaire.version === undefined ? questionnaire.url : `${questionnaire.url}|${questionnaire.version}`
  return {
    resourceType: 'QuestionnaireResponse',
    status: 'in-progress',
    ...(canonical === undefined ? {} : { questionnaire: canonical }),
    item: (questionnaire.item ?? []).filter((item) => item.type !== 'display').map(blank),
  } as QuestionnaireResponse
}

/** Every Questionnaire item by linkId. */
export function itemsByLinkId(questionnaire: Questionnaire): Map<string, QuestionnaireItem> {
  const found = new Map<string, QuestionnaireItem>()
  const walk = (items: QuestionnaireItem[] | undefined) => {
    for (const item of items ?? []) {
      found.set(item.linkId, item)
      walk(item.item)
    }
  }
  walk(questionnaire.item)
  return found
}

/** Every response item, depth first. */
export function responseItems(items: QuestionnaireResponseItem[] | undefined): QuestionnaireResponseItem[] {
  return (items ?? []).flatMap((item) => [item, ...responseItems(item.item)])
}

/** The response, re-parsed so fhirpath.js sees each decimal exactly (ADR 0054). */
function forFhirpath(response: QuestionnaireResponse): unknown {
  return parseLossless(stringify(response) ?? '{}')
}

function evaluate(response: QuestionnaireResponse, expression: string): unknown[] {
  const resource = forFhirpath(response)
  return evaluateRaw(resource, expression, { resource, context: resource })
}

/** The value of an answer, whatever its type. */
function answerValue(answer: QuestionnaireResponseItemAnswer): unknown {
  const a = answer as Record<string, unknown>
  for (const key of ['valueBoolean', 'valueDecimal', 'valueInteger', 'valueDate', 'valueDateTime', 'valueString', 'valueCoding']) {
    if (a[key] !== undefined) return a[key]
  }
  return undefined
}

function enableWhenValue(condition: QuestionnaireItemEnableWhen): unknown {
  const c = condition as unknown as Record<string, unknown>
  for (const key of ['answerBoolean', 'answerDecimal', 'answerInteger', 'answerDate', 'answerDateTime', 'answerString', 'answerCoding']) {
    if (c[key] !== undefined) return c[key]
  }
  return undefined
}

const asNumber = (value: unknown) => (value instanceof LosslessNumber ? Number(value.toString()) : typeof value === 'number' ? value : undefined)

function compare(left: unknown, operator: string, right: unknown): boolean {
  if (typeof left === 'object' && left !== null && 'code' in left) {
    const l = left as Coding
    const r = right as Coding
    const same = l.code === r.code && (r.system === undefined || l.system === r.system)
    return operator === '!=' ? !same : operator === '=' && same
  }
  const [l, r] = [asNumber(left) ?? left, asNumber(right) ?? right] as [number | string | boolean, number | string | boolean]
  switch (operator) {
    case '=':
      return l === r
    case '!=':
      return l !== r
    case '>':
      return l > r
    case '<':
      return l < r
    case '>=':
      return l >= r
    case '<=':
      return l <= r
    default:
      return false
  }
}

/** Whether one enableWhen condition holds, against every answer to its question. */
function holds(condition: QuestionnaireItemEnableWhen, response: QuestionnaireResponse): boolean {
  const answers = responseItems(response.item)
    .filter((item) => item.linkId === condition.question)
    .flatMap((item) => item.answer ?? [])
    .map(answerValue)
    .filter((value) => value !== undefined)
  if (condition.operator === 'exists') return (answers.length > 0) === (condition as { answerBoolean?: boolean }).answerBoolean
  const expected = enableWhenValue(condition)
  return answers.some((answer) => compare(answer, condition.operator, expected))
}

/**
 * Whether `item` is enabled now: its `enableWhenExpression` if it has one, else its
 * `enableWhen` conditions under its `enableBehavior` (`all` by default). An expression that
 * fails to evaluate leaves the item enabled, so nothing is hidden by mistake.
 */
export function isEnabled(item: QuestionnaireItem, response: QuestionnaireResponse): boolean {
  const expression = expressionOf(item.extension, ENABLE_WHEN_EXPRESSION)
  if (expression !== undefined) {
    try {
      const result = evaluate(response, expression)
      return result.length === 0 ? false : result[0] === true
    } catch {
      return true
    }
  }
  const conditions = item.enableWhen ?? []
  if (conditions.length === 0) return true
  return item.enableBehavior === 'any' ? conditions.some((c) => holds(c, response)) : conditions.every((c) => holds(c, response))
}

/** The answer key for a question type. */
function answerKey(type: string): string | undefined {
  switch (type) {
    case 'boolean':
      return 'valueBoolean'
    case 'decimal':
      return 'valueDecimal'
    case 'integer':
      return 'valueInteger'
    case 'date':
      return 'valueDate'
    case 'dateTime':
      return 'valueDateTime'
    case 'string':
    case 'text':
      return 'valueString'
    case 'coding':
      return 'valueCoding'
    default:
      return undefined
  }
}

/**
 * The value `item`'s `calculatedExpression` gives now, as an answer, or undefined if it has none,
 * gives nothing, or fails. A preview: the server recomputes it on submit (ADR 0052).
 */
export function calculated(item: QuestionnaireItem, response: QuestionnaireResponse): QuestionnaireResponseItemAnswer | undefined {
  const expression = expressionOf(item.extension, CALCULATED_EXPRESSION)
  const key = answerKey(item.type)
  if (expression === undefined || key === undefined) return undefined
  let first: unknown
  try {
    first = evaluate(response, expression)[0]
  } catch {
    return undefined
  }
  if (first === undefined) return undefined
  const value = toItem(first)
  if ('number' in value) return { [key]: new LosslessNumber(value.number) } as QuestionnaireResponseItemAnswer
  if ('boolean' in value) return { [key]: value.boolean } as QuestionnaireResponseItemAnswer
  if ('text' in value) return { [key]: value.text } as QuestionnaireResponseItemAnswer
  return undefined
}

/** A failed answer constraint (the `questionnaire-constraint` extension). */
export interface Violation {
  /** The item it is on, or none for the whole form. */
  linkId: string | undefined
  key: string
  severity: 'error' | 'warning'
  human: string
}

function constraintsOf(extensions: Extension[] | undefined) {
  return (extensions ?? [])
    .filter((e) => e.url === CONSTRAINT)
    .map((e) => {
      const part = (name: string) => (e.extension ?? []).find((p) => p.url === name) as Record<string, unknown> | undefined
      return {
        key: (part('key')?.['valueId'] as string | undefined) ?? '',
        severity: ((part('severity')?.['valueCode'] as string | undefined) === 'warning' ? 'warning' : 'error') as 'error' | 'warning',
        expression: (part('expression')?.['valueString'] as string | undefined) ?? '',
        human: (part('human')?.['valueString'] as string | undefined) ?? '',
      }
    })
    .filter((c) => c.expression !== '')
}

/**
 * The answer constraints that fail now: on the Questionnaire and on each enabled item. An
 * expression that gives `false` fails; one that fails to evaluate is reported as failing too.
 */
export function violations(questionnaire: Questionnaire, response: QuestionnaireResponse): Violation[] {
  const failing: Violation[] = []
  const check = (linkId: string | undefined, extensions: Extension[] | undefined) => {
    for (const constraint of constraintsOf(extensions)) {
      let ok: boolean
      try {
        ok = evaluate(response, constraint.expression)[0] !== false
      } catch {
        ok = false
      }
      if (!ok) failing.push({ linkId, key: constraint.key, severity: constraint.severity, human: constraint.human })
    }
  }
  check(undefined, questionnaire.extension)
  for (const [linkId, item] of itemsByLinkId(questionnaire)) {
    if (isEnabled(item, response)) check(linkId, item.extension)
  }
  return failing
}

/** Required, enabled questions with no answer. */
export function missing(questionnaire: Questionnaire, response: QuestionnaireResponse): string[] {
  const items = itemsByLinkId(questionnaire)
  return [...items.values()]
    .filter((item) => item.required === true && item.type !== 'group' && item.type !== 'display' && isEnabled(item, response))
    .filter((item) => !responseItems(response.item).some((r) => r.linkId === item.linkId && (r.answer ?? []).length > 0))
    .map((item) => item.linkId)
}

/**
 * The response with the calculated values applied and disabled items removed, as it is saved or
 * submitted: FHIR keeps no answers to disabled questions.
 */
export function settled(questionnaire: Questionnaire, response: QuestionnaireResponse): QuestionnaireResponse {
  const items = itemsByLinkId(questionnaire)
  const settle = (list: QuestionnaireResponseItem[] | undefined): QuestionnaireResponseItem[] =>
    (list ?? []).flatMap((item) => {
      const definition = items.get(item.linkId)
      if (definition !== undefined && !isEnabled(definition, response)) return []
      const answer = definition === undefined ? undefined : calculated(definition, response)
      const next: QuestionnaireResponseItem = { ...item }
      if (answer !== undefined) next.answer = [answer]
      if (item.item !== undefined) next.item = settle(item.item)
      return [next]
    })
  return { ...response, item: settle(response.item) }
}

/** `item` with `children` as its items, or none. */
function withChildren(item: QuestionnaireResponseItem, children: QuestionnaireResponseItem[] | undefined): QuestionnaireResponseItem {
  const next = { ...item }
  if (children === undefined) delete next.item
  else next.item = children
  return next
}

/** Sets the answers of the response item with `id`. */
export function withAnswers(response: QuestionnaireResponse, id: string, answers: QuestionnaireResponseItemAnswer[]): QuestionnaireResponse {
  const update = (list: QuestionnaireResponseItem[] | undefined): QuestionnaireResponseItem[] | undefined =>
    list?.map((item) => {
      if (item.id === id) {
        const next = { ...item }
        if (answers.length === 0) delete next.answer
        else next.answer = answers
        return next
      }
      return item.item === undefined ? item : withChildren(item, update(item.item))
    })
  return { ...response, item: update(response.item) ?? [] }
}

/** Adds a repeat instance of the item `linkId` after the last one, with a new stable id. */
export function addRepeat(questionnaire: Questionnaire, response: QuestionnaireResponse, linkId: string): QuestionnaireResponse {
  const definition = itemsByLinkId(questionnaire).get(linkId)
  if (definition === undefined || definition.repeats !== true || definition.type !== 'group') return response
  const insert = (list: QuestionnaireResponseItem[] | undefined): QuestionnaireResponseItem[] | undefined => {
    if (list === undefined) return undefined
    const last = list.map((i) => i.linkId).lastIndexOf(linkId)
    const mapped = list.map((item) => (item.item === undefined ? item : withChildren(item, insert(item.item))))
    if (last === -1) return mapped
    return [...mapped.slice(0, last + 1), blank(definition), ...mapped.slice(last + 1)]
  }
  return { ...response, item: insert(response.item) ?? [] }
}

/** Removes the repeat instance with `id`, keeping at least one instance. */
export function removeRepeat(response: QuestionnaireResponse, id: string): QuestionnaireResponse {
  const remove = (list: QuestionnaireResponseItem[] | undefined): QuestionnaireResponseItem[] | undefined => {
    if (list === undefined) return undefined
    const target = list.find((item) => item.id === id)
    const siblings = target === undefined ? 0 : list.filter((item) => item.linkId === target.linkId).length
    return list
      .filter((item) => !(item.id === id && siblings > 1))
      .map((item) => (item.item === undefined ? item : withChildren(item, remove(item.item))))
  }
  return { ...response, item: remove(response.item) ?? [] }
}
