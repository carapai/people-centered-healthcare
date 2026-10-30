import { LosslessNumber, isLosslessNumber } from 'lossless-json'
import { z } from 'zod'

// JSON numbers as the client parses them: LosslessNumber (ADR 0052, ADR 0054), or plain numbers
// from code that built the value itself.
const jsonNumber = z.union([z.number(), z.custom<LosslessNumber>(isLosslessNumber, 'expected a number')])

/** A JSON integer, as a JS number; one beyond the safe-integer range is rejected, not rounded. */
export const integer = jsonNumber
  .refine((n) => Number.isSafeInteger(typeof n === 'number' ? n : Number(n.value)), 'expected a safe integer')
  .transform((n) => (typeof n === 'number' ? n : Number(n.value)))

/** A JSON number that is not an integer: kept as LosslessNumber, so its exact text survives. */
export const decimal = jsonNumber.transform((n) => (typeof n === 'number' ? new LosslessNumber(String(n)) : n))
