// The e2e users, provisioned by backend.ts at KAM013.
export const USERS = {
  clerk: { username: 'e2e-clerk', password: 'tumbling otter quarry seven', roles: 'clerk' },
  reviewer: { username: 'e2e-reviewer', password: 'patient heron basket ninety', roles: 'reviewer' },
} as const
