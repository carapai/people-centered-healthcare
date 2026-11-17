// PROTOTYPE (#482), throwaway: an ART program package as the Program Designer would assemble it
// (program-designer.md): its identity, eligibility, enrolment, stages, forms, regimen templates,
// investigations, follow-up schedule, outcomes, indicators and where it applies. Rules carry their
// FHIRPath text and, for the walk-through only, a JavaScript stand-in for the FHIRPath engine.

export type Where = 'draft' | 'published' | 'missing'
export type Ref = { kind: 'Questionnaire' | 'Workflow Definition' | 'ActivityDefinition' | 'Measure' | 'ValueSet' | 'Extraction Mapping'; title: string; url: string; version: string; where: Where }

export type Patient = { age: number; weightKg: number; sex: 'female' | 'male'; pregnant: boolean; hivPositive: boolean; onTbTreatment: boolean }

export type Rule = { fhirpath: string; test: (p: Patient) => boolean }
export type Regimen = { id: string; line: 'first' | 'second'; title: string; when: Rule; activity: Ref; source: string }
export type Investigation = { id: string; title: string; activity: Ref; /** Months after starting ART. */ at: number[]; later?: string | undefined }
export type Visit = { label: string; /** Days after enrolment. */ day: number }
export type Stage = { id: string; label: string; form?: Ref | undefined; does: string[]; leaves: string }
export type Outcome = { code: string; title: string; how: string }
export type Override = { name: string; value: string; allowed: string }

export type Program = {
  name: string
  version: string
  title: string
  episodeType: string
  approvers: string
  eligibility: Rule
  enrolment: { form: Ref; workflow: Ref }
  workflow: Ref
  stages: Stage[]
  regimens: Regimen[]
  investigations: Investigation[]
  schedule: Visit[]
  scheduleNote: string
  outcomes: Outcome[]
  indicators: Ref[]
  mappings: Ref[]
  scopes: string[]
  overrides: Override[]
}

const q = (title: string, id: string, version: string, where: Where = 'draft'): Ref => ({ kind: 'Questionnaire', title, url: `https://health.go.ug/fhir/Questionnaire/${id}`, version, where })
const ad = (title: string, id: string, where: Where = 'published'): Ref => ({ kind: 'ActivityDefinition', title, url: `https://health.go.ug/fhir/ActivityDefinition/${id}`, version: '1.0.0', where })
const m = (title: string, id: string, where: Where = 'draft'): Ref => ({ kind: 'Measure', title, url: `https://health.go.ug/fhir/Measure/${id}`, version: '1.0.0', where })

export const ART: Program = {
  name: 'ug.moh.art',
  version: '3.0.0',
  title: 'HIV care and antiretroviral therapy (ART)',
  episodeType: 'hiv-art (urn:health:episode-type)',
  approvers: 'Two clinical approvers (ACP lead, MoH standards)',
  eligibility: { fhirpath: "%patient.hivStatus = 'positive' and %patient.activeEpisode('hiv-art').empty()", test: (p) => p.hivPositive },
  enrolment: { form: q('HIV enrolment', 'hiv-enrolment', '3.0.0'), workflow: { kind: 'Workflow Definition', title: 'ART enrolment', url: 'https://health.go.ug/fhir/workflow/art-enrolment', version: '3.0.0', where: 'draft' } },
  workflow: { kind: 'Workflow Definition', title: 'ART care', url: 'https://health.go.ug/fhir/workflow/art-care', version: '3.0.0', where: 'draft' },
  stages: [
    { id: 'enrolment', label: 'Enrolment', form: q('HIV enrolment', 'hiv-enrolment', '3.0.0'), does: ['Open the EpisodeOfCare', 'Assign the unique ART number'], leaves: 'enrolled' },
    { id: 'baseline', label: 'Baseline assessment', form: q('ART baseline assessment', 'art-baseline', '3.0.0'), does: ['CD4 count', 'TB screening', 'Cryptococcal antigen if CD4 < 200'], leaves: 'assessment complete' },
    { id: 'initiation', label: 'ART initiation', form: q('ART initiation', 'art-initiation', '3.0.0'), does: ['Choose the first-line regimen', 'Prescribe for 14 days', 'Book the 2-week review'], leaves: 'regimen started' },
    { id: 'follow-up', label: 'Follow-up', form: q('ART follow-up', 'art-followup', '2.4.0', 'published'), does: ['Adherence check', 'Refill', 'Viral load when due', 'Move to 3- or 6-monthly visits when stable'], leaves: 'transfer, death, stop, or 28 days after a missed visit' },
    { id: 'outcome', label: 'Outcome', form: q('ART exit', 'art-exit', '1.0.0', 'missing'), does: ['Record the outcome', 'Close the EpisodeOfCare'], leaves: 'final' },
  ],
  regimens: [
    { id: 'tld', line: 'first', title: 'TDF/3TC/DTG 300/300/50 mg, one tablet daily', when: { fhirpath: '%patient.age() >= 10 and %patient.weight() >= 30', test: (p) => p.age >= 10 && p.weightKg >= 30 }, activity: ad('Prescribe TLD', 'rx-tld'), source: 'Uganda Consolidated HIV Guidelines 2024, §4.3' },
    { id: 'abc-3tc-dtg', line: 'first', title: 'ABC/3TC 120/60 mg + DTG 10 mg, by weight band', when: { fhirpath: '%patient.weight() >= 3 and %patient.weight() < 30', test: (p) => p.weightKg >= 3 && p.weightKg < 30 }, activity: ad('Prescribe paediatric ABC/3TC + DTG', 'rx-abc-3tc-dtg'), source: 'Uganda Consolidated HIV Guidelines 2024, §4.4' },
    { id: 'tld-rifampicin', line: 'first', title: 'TLD plus an extra DTG 50 mg twelve hours later', when: { fhirpath: "%patient.age() >= 10 and %patient.weight() >= 30 and %patient.activeEpisode('tb').exists()", test: (p) => p.age >= 10 && p.weightKg >= 30 && p.onTbTreatment }, activity: ad('Prescribe TLD with extra DTG', 'rx-tld-extra-dtg', 'missing'), source: 'Uganda Consolidated HIV Guidelines 2024, §6.2' },
    { id: 'second-line', line: 'second', title: 'AZT/3TC + ATV/r', when: { fhirpath: "%vl.last(2).all(value > 1000) and %adherence.counselling = 'completed'", test: () => false }, activity: ad('Prescribe AZT/3TC + ATV/r', 'rx-azt-3tc-atvr'), source: 'Uganda Consolidated HIV Guidelines 2024, §4.7' },
  ],
  investigations: [
    { id: 'cd4', title: 'CD4 count', activity: ad('Order CD4', 'order-cd4'), at: [0] },
    { id: 'vl', title: 'HIV viral load', activity: ad('Order viral load', 'order-hiv-vl'), at: [6, 12], later: 'every 12 months; 3 months after a result over 1000' },
    { id: 'crag', title: 'Cryptococcal antigen', activity: ad('Order CrAg', 'order-crag'), at: [0], later: 'only when CD4 < 200' },
  ],
  schedule: [
    { label: 'Enrolment and initiation', day: 0 },
    { label: '2-week review', day: 14 },
    { label: 'Month 1', day: 30 },
    { label: 'Month 2', day: 60 },
    { label: 'Month 3', day: 90 },
    { label: 'Month 6', day: 180 },
    { label: 'Month 9', day: 270 },
    { label: 'Month 12', day: 360 },
  ],
  scheduleNote: 'Monthly until month 3, then every 3 months. Stable clients (suppressed viral load) may move to 6-monthly community refills where the facility offers them.',
  outcomes: [
    { code: 'transferred-out', title: 'Transferred out', how: 'A transfer request that the receiving facility accepts' },
    { code: 'died', title: 'Died', how: 'A death recorded on the patient' },
    { code: 'stopped', title: 'Stopped treatment', how: 'Recorded by a clinician on the exit form' },
    { code: 'ltfu', title: 'Lost to follow-up', how: 'A timer: 28 days after a missed appointment, a tracing task; then the outcome' },
  ],
  indicators: [m('TX_NEW: newly started on ART', 'tx-new'), m('TX_CURR: currently on ART', 'tx-curr'), m('TX_PVLS: viral load suppressed', 'tx-pvls'), m('Missed appointments traced', 'art-tracing', 'missing')],
  mappings: [{ kind: 'Extraction Mapping', title: 'ART follow-up → Observations, MedicationRequest', url: 'https://health.go.ug/fhir/mapping/art-followup', version: '2.4.0', where: 'published' }],
  scopes: ['All accredited ART sites (Organization tag art-accredited)'],
  overrides: [
    { name: 'Clinic days', value: 'Tuesday, Thursday', allowed: 'Any weekdays' },
    { name: 'Six-monthly community refills', value: 'Off', allowed: 'On or off' },
    { name: 'Tracing after a missed visit', value: '28 days', allowed: '14 to 28 days' },
  ],
}

/** Everything the package links, with where it stands. */
export function parts(p: Program): { section: string; ref: Ref }[] {
  return [
    { section: 'Enrolment', ref: p.enrolment.form },
    { section: 'Enrolment', ref: p.enrolment.workflow },
    { section: 'Stages', ref: p.workflow },
    ...p.stages.flatMap((s) => (s.form && s.id !== 'enrolment' ? [{ section: 'Stages', ref: s.form }] : [])),
    ...p.regimens.map((r) => ({ section: 'Regimens', ref: r.activity })),
    ...p.investigations.map((i) => ({ section: 'Investigations', ref: i.activity })),
    ...p.mappings.map((ref) => ({ section: 'Forms', ref })),
    ...p.indicators.map((ref) => ({ section: 'Indicators', ref })),
  ]
}

export function problems(p: Program): string[] {
  const out = parts(p)
    .filter((x) => x.ref.where === 'missing')
    .map((x) => `${x.section}: ${x.ref.title} (${x.ref.kind}) does not exist yet`)
  for (const r of p.regimens) if (!r.source) out.push(`Regimens: ${r.title} cites no approved protocol`)
  if (p.scopes.length === 0) out.push('Applies to: no scope, so it can never be activated')
  return out
}

export type Walk = { eligible: boolean; regimen?: Regimen | undefined; visits: { date: string; label: string; tests: string[] }[] }

/** A sample patient through eligibility, regimen choice and the first year's visits. */
export function walk(p: Program, patient: Patient, start: Date): Walk {
  const eligible = p.eligibility.test(patient)
  const regimen = p.regimens.filter((r) => r.line === 'first').find((r) => r.when.test(patient))
  const date = (day: number) => new Date(start.getTime() + day * 86_400_000).toISOString().slice(0, 10)
  const visits = eligible
    ? p.schedule.map((v) => ({
        date: date(v.day),
        label: v.label,
        tests: p.investigations.filter((i) => i.at.some((month) => Math.abs(month * 30 - v.day) < 8)).map((i) => i.title),
      }))
    : []
  return { eligible, regimen, visits }
}
