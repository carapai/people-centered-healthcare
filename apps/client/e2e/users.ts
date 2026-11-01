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
} as const
