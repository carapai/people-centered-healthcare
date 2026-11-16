import { tmpdir } from 'node:os'
import { join } from 'node:path'

/** Where the e2e backend's development outbox puts patients' one-time codes, one
 * `{phone} {code}` line each (#111; there is no SMS provider yet, #235). */
export const OTP_OUTBOX = join(tmpdir(), 'health-e2e-otp-outbox')

// The e2e users, provisioned by backend.ts at KAM013.
export const USERS = {
  clerk: { username: 'e2e-clerk', password: 'tumbling otter quarry seven', roles: 'clerk' },
  reviewer: { username: 'e2e-reviewer', password: 'patient heron basket ninety', roles: 'reviewer' },
  storekeeper: { username: 'e2e-storekeeper', password: 'copper lantern meadow forty', roles: 'storekeeper' },
  cashier: { username: 'e2e-cashier', password: 'saffron kettle harbour nine', roles: 'cashier' },
  supervisor: { username: 'e2e-supervisor', password: 'granite willow pocket eleven', roles: 'supervisor' },
  nurse: { username: 'e2e-nurse', password: 'amber falcon ladder twelve', roles: 'nurse' },
  clinician: { username: 'e2e-clinician', password: 'velvet otter compass thirty', roles: 'clinician' },
  pharmacist: { username: 'e2e-pharmacist', password: 'linen badger orchard sixty', roles: 'pharmacist' },
  labtech: { username: 'e2e-labtech', password: 'quartz heron lantern eighty', roles: 'labtech' },
  verifier: { username: 'e2e-verifier', password: 'cobalt tapir meadow ninety', roles: 'labtech' },
  // Every right, for exploring a local backend by hand; no test signs in as it.
  admin: { username: 'admin', password: 'harbour lantern meadow quartz', roles: 'admin' },
} as const

/** The admin role: every FHIR interaction on every resource type, break-glass, and every named
 * permission the platform checks. A permission added to the code belongs here too. */
export const ADMIN_PERMISSIONS = [
  '*.cruds',
  'break-glass',
  'break-glass.alerts',
  'break-glass.review',
  'appointments.book',
  'caregiver.manage',
  'careplans.manage',
  'careplans.view',
  'consultation.document',
  'encounters.checkin',
  'encounters.manage',
  'laboratory.verify',
  'laboratory.work',
  'medications.prescribe',
  'orders.request',
  'payments.cash',
  'payments.refund',
  'payments.supervise',
  'pharmacy.dispense',
  'pharmacy.view',
  'pricing.approve',
  'pricing.manage',
  'pricing.quote',
  'programs.enrol',
  'programs.manage',
  'programs.view',
  'queues.view',
  'queues.work',
  'registry.review',
  'results.review',
  'stock.manage',
  'sync.resolve-conflicts',
  'workflow.supervise',
  'workflow.work',
]
