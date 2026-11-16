// Activity for a seeded local backend (`HEALTH_SEED=n bun e2e/backend.ts`), at KAM013 where
// the admin signs in: what the screens show once a facility has been working for a while. It goes
// through the same command APIs the screens use, so every rule and side effect applies (queue
// entries, vital-sign extraction, the stock ledger), and none of it is written around them.
//
// - **Stock:** the product master, and goods received into `Location/pharmacy` (the Stock
//   screen's default store) and `Location/main-store`, with a batch near expiry, and a ward's
//   requisition waiting for the pharmacy.
// - **Visits and vital signs:** patients checked in. Some wait at triage; others are triaged with
//   the OPD triage form, so their vital signs are extracted as Observations, and wait for
//   consultation; the rest are triaged and their visit completed.
// - **Appointments:** booked appointments over the next two weeks, and patients' appointment
//   requests (as the portal makes them) in the clinician's inbox.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

const FACILITY = 'KAM013'
const TRIAGE_FORM = 'https://health.go.ug/fhir/Questionnaire/opd-triage|1.0.0'
const TRIAGE_CATEGORY = 'https://health.go.ug/fhir/CodeSystem/triage-category'
const FACILITY_TAG = 'urn:health:facility'
const ORIGIN_TAG = 'urn:health:origin'

/** Products: code (lowercase letters, digits and dashes), name, base unit, pack size, unit cost (UGX). */
const PRODUCTS: [string, string, string, number, number][] = [
  ['amox-500', 'Amoxicillin 500 mg capsule', 'capsule', 100, 120],
  ['al-20-120', 'Artemether/Lumefantrine 20/120 mg tablet', 'tablet', 24, 260],
  ['pcm-500', 'Paracetamol 500 mg tablet', 'tablet', 1000, 25],
  ['ors', 'Oral rehydration salts sachet', 'sachet', 50, 350],
  ['zinc-20', 'Zinc sulfate 20 mg dispersible tablet', 'tablet', 100, 60],
  ['ctx-960', 'Cotrimoxazole 960 mg tablet', 'tablet', 100, 90],
  ['tld', 'Tenofovir/Lamivudine/Dolutegravir 300/300/50 mg tablet', 'tablet', 30, 400],
  ['met-500', 'Metformin 500 mg tablet', 'tablet', 100, 70],
  ['aml-5', 'Amlodipine 5 mg tablet', 'tablet', 100, 55],
  ['fefo', 'Ferrous sulfate/Folic acid 200/0.4 mg tablet', 'tablet', 1000, 20],
  ['oxy-10', 'Oxytocin 10 IU/mL injection', 'ampoule', 10, 1800],
  ['cro-1g', 'Ceftriaxone 1 g powder for injection', 'vial', 10, 2500],
  ['mtz-400', 'Metronidazole 400 mg tablet', 'tablet', 100, 45],
  ['alb-400', 'Albendazole 400 mg chewable tablet', 'tablet', 100, 150],
  ['mrdt', 'Malaria rapid diagnostic test', 'test', 25, 1200],
  ['ns-500', 'Sodium chloride 0.9% 500 mL infusion', 'bottle', 20, 3000],
  ['ibu-200', 'Ibuprofen 200 mg tablet', 'tablet', 100, 40],
  ['salb-100', 'Salbutamol 100 mcg/dose inhaler', 'inhaler', 1, 9500],
]

/** Reasons people come and ask to be seen. */
const REASONS = ['Review of blood pressure', 'Diabetes review', 'Antenatal visit', 'HIV clinic refill visit', 'Follow-up after malaria treatment', 'Child immunisation', 'Family planning', 'Cough for two weeks']

type Patient = { id: string; name?: { given?: string[]; family?: string }[]; identifier?: { system?: string }[]; address?: { district?: string }[] }

/** The patients to give activity to: those registered at the facility first, then Kampala's. */
function choose(seedDir: string, count: number): Patient[] {
  const all = readFileSync(join(seedDir, 'Patient.ndjson'), 'utf8')
    .split('\n')
    .filter((line) => line !== '')
    .map((line) => JSON.parse(line) as Patient)
  const here = (p: Patient) => p.identifier?.some((i) => i.system === `urn:health:mrn/${FACILITY}`) === true
  const near = (p: Patient) => !here(p) && p.address?.[0]?.district === 'Kampala'
  return [...all.filter(here), ...all.filter(near), ...all.filter((p) => !here(p) && !near(p))].slice(0, count)
}

/** One answered item of the triage form. */
const answer = (linkId: string, key: 'valueDecimal' | 'valueInteger', value: number) => ({ linkId, answer: [{ [key]: value }] })

/** A small deterministic sequence, so each start gives the same readings. */
function numbers(seed: number) {
  let state = seed
  return (low: number, high: number, decimals = 0) => {
    state = (state * 1_103_515_245 + 12_345) % 2_147_483_648
    const value = low + (state / 2_147_483_648) * (high - low)
    const scale = 10 ** decimals
    return Math.round(value * scale) / scale
  }
}

const day = (offset: number, hour: number) => {
  const d = new Date()
  d.setUTCDate(d.getUTCDate() + offset)
  d.setUTCHours(hour - 3, 0, 0, 0) // East Africa Time, UTC+3
  return d.toISOString()
}

/** Seeds the activity into `central` as the holder of `token` (the admin, at KAM013). */
export async function activity(central: string, token: string, seedDir: string): Promise<string> {
  const call = async (method: 'GET' | 'POST' | 'PUT', path: string, body?: unknown): Promise<unknown> => {
    const response = await fetch(`${central}${path}`, {
      method,
      headers: { authorization: `Bearer ${token}`, 'content-type': path.startsWith('/fhir/') ? 'application/fhir+json' : 'application/json', accept: 'application/json, application/fhir+json' },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    })
    const text = await response.text()
    if (!response.ok) throw new Error(`${method} ${path}: ${response.status} ${text.slice(0, 500)}`)
    return text === '' ? undefined : (JSON.parse(text) as unknown)
  }
  const random = numbers(7)

  // Stock.
  for (const [code, name, baseUnit, pack] of PRODUCTS) {
    await call('POST', '/api/stock/products', { code, name, baseUnit, packSizes: [pack] })
  }
  const receive = (store: string, delivery: string, near: boolean) =>
    call('POST', '/api/stock/goods-received', {
      store,
      supplier: 'National Medical Stores',
      delivery,
      lines: PRODUCTS.map(([code, , , pack, cost], i) => ({
        product: code,
        batch: `${code}-${delivery.slice(-2)}${i}`,
        // One batch in eight expires within three months, so the screen flags it.
        expiry: near && i % 8 === 0 ? day(75, 12).slice(0, 10) : `${2027 + (i % 3)}-${String(1 + (i % 12)).padStart(2, '0')}-28`,
        quantity: pack * (2 + (i % 5)) * (store.endsWith('main-store') ? 5 : 1),
        pack_size: pack,
        unit_cost: String(cost),
      })),
    })
  await receive('Location/main-store', 'NMS-2026-11-01', false)
  await receive('Location/pharmacy', 'NMS-2026-11-02', true)
  await call('POST', '/api/stock/requisitions', {
    requesting_store: 'Location/ward',
    supplying_store: 'Location/pharmacy',
    lines: [
      { product: 'pcm-500', quantity: 500 },
      { product: 'cro-1g', quantity: 20 },
      { product: 'ns-500', quantity: 12 },
    ],
  })

  // Visits and vital signs.
  const people = choose(seedDir, 34)
  const visits = people.slice(0, 18)
  const triage = (patient: string, encounter: string) => {
    const id = Bun.randomUUIDv7()
    const fever = random(0, 1) < 0.3
    const category = fever ? 'urgent' : 'routine'
    return call('PUT', `/fhir/QuestionnaireResponse/${id}`, {
      resourceType: 'QuestionnaireResponse',
      id,
      status: 'completed',
      questionnaire: TRIAGE_FORM,
      subject: { reference: patient },
      encounter: { reference: `Encounter/${encounter}` },
      authored: new Date().toISOString(),
      item: [
        {
          linkId: 'vitals',
          item: [
            answer('temperature', 'valueDecimal', fever ? random(38, 39.8, 1) : random(36.2, 37.4, 1)),
            answer('pulse', 'valueInteger', fever ? random(98, 124) : random(64, 92)),
            answer('respiratory-rate', 'valueInteger', random(14, 24)),
            answer('spo2', 'valueInteger', random(93, 100)),
            answer('weight', 'valueDecimal', random(48, 92, 1)),
            answer('height', 'valueDecimal', random(150, 185)),
            { linkId: 'blood-pressure', item: [answer('systolic', 'valueInteger', random(104, 158)), answer('diastolic', 'valueInteger', random(64, 98))] },
          ],
        },
        { linkId: 'triage-category', answer: [{ valueCoding: { system: TRIAGE_CATEGORY, code: category } }] },
      ],
    })
  }
  for (const [index, person] of visits.entries()) {
    const patient = `Patient/${person.id}`
    const encounter = Bun.randomUUIDv7()
    await call('POST', '/api/reception/check-in', { id: encounter, patient, servicePoint: 'triage', class: 'ambulatory' })
    // A third wait at triage; a third are triaged and wait for consultation; a third are done.
    if (index % 3 === 0) continue
    await triage(patient, encounter)
    await call('POST', `/api/reception/encounters/${encounter}/move`, { servicePoint: 'consultation' })
    if (index % 3 === 2) await call('POST', `/api/reception/encounters/${encounter}/status`, { status: 'completed' })
  }

  // Appointments.
  const appointments = people.slice(18)
  for (const [index, person] of appointments.entries()) {
    const id = Bun.randomUUIDv7()
    const patient = `Patient/${person.id}`
    const reason = REASONS[index % REASONS.length] ?? 'Review'
    if (index < 5) {
      // A patient's request from the portal, waiting in the inbox to be booked or declined.
      await call('PUT', `/fhir/Appointment/${id}`, {
        resourceType: 'Appointment',
        id,
        meta: { tag: [{ system: FACILITY_TAG, code: FACILITY }, { system: ORIGIN_TAG, code: 'patient-reported' }] },
        status: 'proposed',
        description: reason,
        subject: { reference: patient },
        participant: [{ actor: { reference: patient }, status: 'accepted' }],
        created: new Date().toISOString(),
        requestedPeriod: [{ start: day(3 + index, 9).slice(0, 10) }],
      })
    } else {
      const start = day(1 + (index % 14), 8 + (index % 8))
      await call('PUT', `/fhir/Appointment/${id}`, {
        resourceType: 'Appointment',
        id,
        meta: { tag: [{ system: FACILITY_TAG, code: FACILITY }] },
        status: 'booked',
        description: reason,
        subject: { reference: patient },
        start,
        end: new Date(Date.parse(start) + 20 * 60_000).toISOString(),
        participant: [{ actor: { reference: patient }, status: 'accepted' }],
        created: new Date().toISOString(),
      })
    }
  }
  const waiting = visits.filter((_, i) => i % 3 === 0).length
  const consult = visits.filter((_, i) => i % 3 === 1).length
  return `${PRODUCTS.length} products received into Location/pharmacy and Location/main-store; ${visits.length} visits (${waiting} at triage, ${consult} triaged and waiting for consultation, ${visits.length - waiting - consult} completed); ${appointments.length} appointments (5 requests in the inbox)`
}
