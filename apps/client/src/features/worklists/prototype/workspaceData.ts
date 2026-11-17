// PROTOTYPE (#563), throwaway: made-up OPD data for the health worker workspace layouts.
// The real data comes from the `worklists` and `shared-record` read models and /api/inbox.

export type Role = 'clinician' | 'nurse' | 'receptionist'
export type Density = 'comfortable' | 'compact'
export type Priority = 'emergency' | 'urgent' | 'routine'

export const ROLES: { id: Role; label: string; density: Density; modules: { id: string; label: string }[] }[] = [
  {
    id: 'clinician',
    label: 'Clinician (Dr. Okot)',
    density: 'compact',
    modules: [
      { id: 'queue', label: 'My queue' },
      { id: 'inbox', label: 'Inbox' },
      { id: 'patients', label: 'Find a patient' },
      { id: 'notes', label: 'Unsigned notes' },
    ],
  },
  {
    id: 'nurse',
    label: 'Nurse (Sr. Apio)',
    density: 'comfortable',
    modules: [
      { id: 'queue', label: 'Triage queue' },
      { id: 'inbox', label: 'Inbox' },
      { id: 'patients', label: 'Find a patient' },
      { id: 'tasks', label: 'Care tasks' },
    ],
  },
  {
    id: 'receptionist',
    label: 'Receptionist (Mr. Wasswa)',
    density: 'comfortable',
    modules: [
      { id: 'queue', label: 'Arrivals' },
      { id: 'patients', label: 'Register or find' },
      { id: 'queues', label: 'All queues' },
    ],
  },
]

export type Vitals = { temp: number; bp: string; pulse: number; spo2: number; weight: number }
export type QueueItem = {
  id: string
  name: string
  age: string
  sex: 'F' | 'M'
  reason: string
  priority: Priority
  waited: number
  status: 'waiting' | 'mine' | 'taken'
  takenBy?: string
  payment: 'free' | 'cleared' | 'awaiting'
  vitals?: Vitals
  flags: string[]
  step: Record<Role, string>
}

export const QUEUE: QueueItem[] = [
  { id: 'q1', name: 'Achieng Grace', age: '2 y', sex: 'F', reason: 'Fever 3 days, convulsed once at home', priority: 'emergency', waited: 4, status: 'waiting', payment: 'awaiting', vitals: { temp: 39.6, bp: '-', pulse: 168, spo2: 91, weight: 11 }, flags: [], step: { clinician: 'Consultation', nurse: 'Triage done', receptionist: 'Sent to OPD' } },
  { id: 'q2', name: 'Mugisha Peter', age: '54 y', sex: 'M', reason: 'Headache, review of BP', priority: 'urgent', waited: 22, status: 'mine', payment: 'cleared', vitals: { temp: 36.8, bp: '182/110', pulse: 96, spo2: 97, weight: 81 }, flags: ['Hypertension', 'Allergy: penicillin'], step: { clinician: 'Consultation', nurse: 'Triage done', receptionist: 'Sent to OPD' } },
  { id: 'q3', name: 'Nakato Sarah', age: '27 y', sex: 'F', reason: 'ANC follow-up, 24 weeks', priority: 'routine', waited: 35, status: 'waiting', payment: 'free', vitals: { temp: 36.6, bp: '118/76', pulse: 82, spo2: 98, weight: 64 }, flags: ['ANC', 'HIV positive: on ART'], step: { clinician: 'Consultation', nurse: 'Triage done', receptionist: 'Sent to ANC' } },
  { id: 'q4', name: 'Ochieng Daniel', age: '8 y', sex: 'M', reason: 'Cough 2 weeks', priority: 'routine', waited: 41, status: 'taken', takenBy: 'Dr. Namusoke', payment: 'free', vitals: { temp: 37.4, bp: '-', pulse: 104, spo2: 96, weight: 24 }, flags: ['TB screen positive'], step: { clinician: 'Consultation', nurse: 'Triage done', receptionist: 'Sent to OPD' } },
  { id: 'q5', name: 'Namatovu Ruth', age: '63 y', sex: 'F', reason: 'Diabetes refill', priority: 'routine', waited: 52, status: 'waiting', payment: 'cleared', vitals: { temp: 36.5, bp: '138/84', pulse: 78, spo2: 97, weight: 72 }, flags: ['Diabetes', 'Last HbA1c 9.1%'], step: { clinician: 'Consultation', nurse: 'Triage done', receptionist: 'Sent to OPD' } },
  { id: 'q6', name: 'Kato Brian', age: '31 y', sex: 'M', reason: 'Painful urination', priority: 'routine', waited: 8, status: 'waiting', payment: 'awaiting', flags: [], step: { clinician: 'Not triaged yet', nurse: 'Triage', receptionist: 'At cashier' } },
]

export type InboxItem = { id: string; kind: 'result' | 'referral' | 'approval' | 'request' | 'documentation'; title: string; patient: string; critical: boolean; since: string }
export const INBOX: InboxItem[] = [
  { id: 'i1', kind: 'result', title: 'Potassium 6.4 mmol/L (critical)', patient: 'Namatovu Ruth', critical: true, since: '10 min' },
  { id: 'i2', kind: 'result', title: 'Malaria RDT positive', patient: 'Achieng Grace', critical: false, since: '2 min' },
  { id: 'i3', kind: 'referral', title: 'Referral in from Kawaala HC III: suspected pre-eclampsia', patient: 'Babirye Joan', critical: false, since: '25 min' },
  { id: 'i4', kind: 'approval', title: 'Prescription needs approval: ceftriaxone 1 g', patient: 'Mugisha Peter', critical: false, since: '1 h' },
  { id: 'i5', kind: 'documentation', title: 'Unsigned note: consultation yesterday 16:40', patient: 'Okello Moses', critical: false, since: '18 h' },
  { id: 'i6', kind: 'request', title: 'Patient asks to move appointment to Friday', patient: 'Nakato Sarah', critical: false, since: '3 h' },
]

export type Chart = { allergies: string[]; conditions: string[]; medications: string[]; programs: string[]; encounters: { date: string; what: string }[]; results: { name: string; value: string; when: string; flag?: 'high' | 'low' | 'critical' }[] }
const base: Chart = { allergies: [], conditions: [], medications: [], programs: [], encounters: [], results: [] }
export const CHARTS: Record<string, Chart> = {
  q1: { ...base, results: [{ name: 'Malaria RDT', value: 'positive', when: 'today 10:12', flag: 'high' }, { name: 'Hb', value: '8.9 g/dL', when: 'today 10:15', flag: 'low' }], encounters: [{ date: '2026-09-03', what: 'OPD: diarrhoea, ORS' }] },
  q2: { allergies: ['Penicillin (rash)'], conditions: ['Essential hypertension (2019)'], medications: ['Amlodipine 10 mg daily', 'Hydrochlorothiazide 25 mg daily'], programs: ['NCD clinic'], encounters: [{ date: '2026-10-14', what: 'NCD review: BP 164/98, adherence counselled' }, { date: '2026-08-12', what: 'NCD review' }], results: [{ name: 'Creatinine', value: '118 µmol/L', when: '2026-10-14' }] },
  q3: { ...base, conditions: ['HIV (2021), virally suppressed'], medications: ['TLD daily', 'Ferrous + folic acid'], programs: ['ANC (G2P1, 24 weeks)', 'HIV care'], encounters: [{ date: '2026-10-20', what: 'ANC 2nd visit' }, { date: '2026-08-25', what: 'ANC 1st visit at 16 weeks' }], results: [{ name: 'Viral load', value: '< 50 copies/mL', when: '2026-08-25' }] },
  q4: { ...base, results: [{ name: 'GeneXpert MTB/RIF', value: 'pending', when: 'sent today' }] },
  q5: { ...base, conditions: ['Type 2 diabetes (2015)', 'CKD stage 3'], medications: ['Metformin 1 g twice daily', 'Enalapril 10 mg daily'], programs: ['NCD clinic'], results: [{ name: 'Potassium', value: '6.4 mmol/L', when: 'today 09:40', flag: 'critical' }, { name: 'HbA1c', value: '9.1 %', when: '2026-09-02', flag: 'high' }] },
  q6: base,
}

export type Sync = { mode: 'Facility Edge'; central: boolean; pending: number; lastSync: string }
export const priorityLabel: Record<Priority, string> = { emergency: 'Emergency', urgent: 'Urgent', routine: 'Routine' }
