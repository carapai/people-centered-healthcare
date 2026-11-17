// PROTOTYPE (#512), throwaway: a few code systems, ValueSets and the forms that bind them, a tiny
// stand-in for the terminology service's $expand, and the semver bump terminology.md defines
// (add a code: minor; change a display: patch; remove or redefine a code: major).
// ICD-11 here is a small hand-picked sample for the screens, not the real classification.

export type Concept = { code: string; display: string; definition?: string | undefined; parent?: string | undefined; retired?: boolean | undefined }
export type CodeSystem = { url: string; title: string; version: string; local: boolean; where: 'published' | 'draft'; total: number; concepts: Concept[] }
export type Filter = { op: 'is-a' | 'descendent-of'; value: string }
export type Rule = { system: string; codes: string[]; filter?: Filter | undefined }
export type ValueSet = { url: string; title: string; version: string; where: 'published' | 'draft'; include: Rule[]; exclude: Rule[] }
export type Form = { title: string; item: string; valueSet: string }

const ICD11 = 'http://id.who.int/icd/release/11/mms'
const TRIAGE = 'https://health.go.ug/fhir/CodeSystem/triage-category'
const DANGER = 'https://health.go.ug/fhir/CodeSystem/danger-signs'

export const SYSTEMS: CodeSystem[] = [
  {
    url: ICD11,
    title: 'ICD-11 for Mortality and Morbidity Statistics',
    version: '2025-01',
    local: false,
    where: 'published',
    total: 17_036,
    concepts: [
      { code: '01', display: 'Certain infectious or parasitic diseases' },
      { code: '1A0', display: 'Bacterial intestinal infections', parent: '01' },
      { code: '1A00', display: 'Cholera', parent: '1A0' },
      { code: '1A07', display: 'Typhoid fever', parent: '1A0' },
      { code: '1B1', display: 'Tuberculosis', parent: '01' },
      { code: '1B10', display: 'Respiratory tuberculosis, bacteriologically confirmed', parent: '1B1' },
      { code: '1B11', display: 'Respiratory tuberculosis, not bacteriologically confirmed', parent: '1B1' },
      { code: '1B12', display: 'Tuberculosis of the nervous system', parent: '1B1' },
      { code: '1C6', display: 'Human immunodeficiency virus disease', parent: '01' },
      { code: '1C60', display: 'HIV disease clinical stage 1', parent: '1C6' },
      { code: '1C62', display: 'HIV disease clinical stage 3', parent: '1C6' },
      { code: '1F4', display: 'Malaria', parent: '01' },
      { code: '1F40', display: 'Malaria due to Plasmodium falciparum', parent: '1F4' },
      { code: '1F40.0', display: 'Plasmodium falciparum malaria with cerebral complications', parent: '1F40' },
      { code: '1F41', display: 'Malaria due to Plasmodium vivax', parent: '1F4' },
      { code: '1F42', display: 'Malaria due to Plasmodium malariae', parent: '1F4' },
      { code: '1F4Z', display: 'Malaria, unspecified', parent: '1F4' },
      { code: '12', display: 'Diseases of the respiratory system' },
      { code: 'CA40', display: 'Pneumonia', parent: '12' },
      { code: 'CA40.0', display: 'Bacterial pneumonia', parent: 'CA40' },
      { code: 'CA23', display: 'Asthma', parent: '12' },
      { code: '11', display: 'Diseases of the circulatory system' },
      { code: 'BA00', display: 'Essential hypertension', parent: '11' },
    ],
  },
  {
    url: TRIAGE,
    title: 'Triage category',
    version: '1.1.0',
    local: true,
    where: 'published',
    total: 3,
    concepts: [
      { code: 'emergency', display: 'Emergency', definition: 'Needs immediate care; bypasses payment and the queue' },
      { code: 'urgent', display: 'Urgent', definition: 'Seen before routine patients' },
      { code: 'routine', display: 'Routine', definition: 'Seen in order of arrival' },
    ],
  },
  {
    url: DANGER,
    title: 'Danger signs (IMCI)',
    version: '1.0.0',
    local: true,
    where: 'draft',
    total: 5,
    concepts: [
      { code: 'unable-to-drink', display: 'Unable to drink or breastfeed' },
      { code: 'vomits-everything', display: 'Vomits everything' },
      { code: 'convulsions', display: 'Convulsions' },
      { code: 'lethargic', display: 'Lethargic or unconscious' },
      { code: 'chest-indrawing', display: 'Chest indrawing' },
    ],
  },
]

export const VALUE_SETS: ValueSet[] = [
  {
    url: 'https://health.go.ug/fhir/ValueSet/opd-diagnoses',
    title: 'OPD diagnoses (HMIS 105)',
    version: '3.2.0',
    where: 'draft',
    include: [
      { system: ICD11, codes: [], filter: { op: 'is-a', value: '1F4' } },
      { system: ICD11, codes: [], filter: { op: 'descendent-of', value: '1B1' } },
      { system: ICD11, codes: ['1A00', '1A07', 'CA40', 'CA23', 'BA00'] },
    ],
    exclude: [{ system: ICD11, codes: ['1F4Z'] }],
  },
  { url: 'https://health.go.ug/fhir/ValueSet/triage-category', title: 'Triage category', version: '1.1.0', where: 'published', include: [{ system: TRIAGE, codes: [] }], exclude: [] },
  { url: 'https://health.go.ug/fhir/ValueSet/danger-signs', title: 'Danger signs', version: '1.0.0', where: 'draft', include: [{ system: DANGER, codes: [] }], exclude: [] },
]

export const FORMS: Form[] = [
  { title: 'OPD consultation', item: 'Diagnosis', valueSet: 'https://health.go.ug/fhir/ValueSet/opd-diagnoses' },
  { title: 'OPD triage', item: 'Triage category', valueSet: 'https://health.go.ug/fhir/ValueSet/triage-category' },
  { title: 'Sick child (IMCI)', item: 'Danger signs', valueSet: 'https://health.go.ug/fhir/ValueSet/danger-signs' },
  { title: 'HMIS 105 monthly', item: 'Diagnosis line', valueSet: 'https://health.go.ug/fhir/ValueSet/opd-diagnoses' },
]

export const systemOf = (systems: CodeSystem[], url: string) => systems.find((s) => s.url === url)

function descendants(cs: CodeSystem, code: string): string[] {
  const kids = cs.concepts.filter((c) => c.parent === code).map((c) => c.code)
  return kids.flatMap((k) => [k, ...descendants(cs, k)])
}

export function ruleCodes(systems: CodeSystem[], rule: Rule): string[] {
  const cs = systemOf(systems, rule.system)
  if (!cs) return []
  if (rule.filter) return rule.filter.op === 'is-a' ? [rule.filter.value, ...descendants(cs, rule.filter.value)] : descendants(cs, rule.filter.value)
  if (rule.codes.length) return rule.codes.filter((c) => cs.concepts.some((x) => x.code === c))
  return cs.concepts.filter((c) => !c.retired).map((c) => c.code)
}

export type Expanded = { system: string; code: string; display: string }

/** A stand-in for `$expand`: includes, minus excludes, in order, without duplicates. */
export function expand(systems: CodeSystem[], vs: ValueSet): Expanded[] {
  const out: Expanded[] = []
  const gone = new Set(vs.exclude.flatMap((r) => ruleCodes(systems, r).map((c) => `${r.system}|${c}`)))
  for (const r of vs.include) {
    const cs = systemOf(systems, r.system)
    for (const code of ruleCodes(systems, r)) {
      const key = `${r.system}|${code}`
      if (gone.has(key) || out.some((x) => `${x.system}|${x.code}` === key)) continue
      out.push({ system: r.system, code, display: cs?.concepts.find((c) => c.code === code)?.display ?? code })
    }
  }
  return out
}

/** The semver bump a change to a local CodeSystem needs (terminology.md § Lifecycle). */
export function bump(before: Concept[], after: Concept[]): { level: 'none' | 'patch' | 'minor' | 'major'; why: string[] } {
  const why: string[] = []
  let level: 'none' | 'patch' | 'minor' | 'major' = 'none'
  const rank = { none: 0, patch: 1, minor: 2, major: 3 }
  const raise = (l: typeof level, reason: string) => {
    why.push(reason)
    if (rank[l] > rank[level]) level = l
  }
  for (const b of before) {
    const a = after.find((x) => x.code === b.code)
    if (!a) raise('major', `${b.code} removed`)
    else if (a.retired && !b.retired) raise('minor', `${b.code} retired (kept, no longer offered)`)
    else if ((a.definition ?? '') !== (b.definition ?? '')) raise('major', `${b.code} redefined`)
    else if (a.display !== b.display) raise('patch', `${b.code} display changed`)
  }
  for (const a of after) if (!before.some((b) => b.code === a.code)) raise('minor', `${a.code || '(blank)'} added`)
  return { level, why }
}

export const nextVersion = (v: string, level: 'none' | 'patch' | 'minor' | 'major') => {
  const [ma = 0, mi = 0, pa = 0] = v.split('.').map(Number)
  return level === 'major' ? `${ma + 1}.0.0` : level === 'minor' ? `${ma}.${mi + 1}.0` : level === 'patch' ? `${ma}.${mi}.${pa + 1}` : v
}
