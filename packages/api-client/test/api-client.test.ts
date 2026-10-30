import { LosslessNumber, parse } from 'lossless-json'
import { describe, expect, test } from 'vitest'
import { z } from 'zod'

import { ResponseError, decimal, integer, operations, parseResponse } from '../src'

// Bodies arrive as text and are parsed with lossless-json, as the client's transports do.
const body = (text: string): unknown => parse(text)

describe('generated operations', () => {
  test('a ready report parses, and so does a not-ready one on 503', () => {
    expect(parseResponse('getReady', 200, body('{"status":"ready","checks":{"database":"pass"}}'))).toEqual({
      status: 'ready',
      checks: { database: 'pass' },
    })
    expect(parseResponse('getReady', 503, body('{"status":"not-ready","checks":{"database":"fail"}}')).status).toBe('not-ready')
  })

  test('a body that does not match its schema is rejected with paths, not values', () => {
    try {
      parseResponse('getReady', 200, body('{"status":"maybe","checks":{"database":"pass"}}'))
      expect.unreachable()
    } catch (error) {
      expect(error).toBeInstanceOf(ResponseError)
      const issues = (error as ResponseError).issues
      expect(issues.map((i) => i.path)).toEqual([['status']])
      expect((error as Error).message).not.toContain('maybe')
    }
  })

  test('an undocumented status is rejected', () => {
    expect(() => parseResponse('getHealth', 500, body('{}'))).toThrow(ResponseError)
  })

  test('operations carry their method and path', () => {
    expect(operations.getHealth).toMatchObject({ method: 'GET', path: '/health' })
    expect(operations.getReady).toMatchObject({ method: 'GET', path: '/ready' })
  })
})

describe('numbers', () => {
  const Dto = z.object({ count: integer, amount: decimal })

  test('integers become numbers and decimals keep their exact text', () => {
    const parsed = Dto.parse(body('{"count":3,"amount":1.50}'))
    expect(parsed.count).toBe(3)
    expect(parsed.amount).toBeInstanceOf(LosslessNumber)
    expect(parsed.amount.toString()).toBe('1.50')
  })

  test('an integer beyond the safe range is rejected, not rounded', () => {
    expect(Dto.safeParse(body('{"count":9007199254740993,"amount":1}')).success).toBe(false)
  })

  test('plain numbers from code are accepted too', () => {
    const parsed = Dto.parse({ count: 2, amount: 0.1 })
    expect(parsed.amount.toString()).toBe('0.1')
  })
})
