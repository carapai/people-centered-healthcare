import { describe, expect, test } from 'vitest'

import { type Area, AreaError, areaQuery, facilitiesByDistrict } from '../src/features/area/api'
import { fakeTransport } from './fake'

const place = (code: string, level: Area['level'], parent: string | null, active = true) => ({ code, name: code, level, parent, active })

describe('area', () => {
  test("the session's area is read from central", async () => {
    const area: Area = {
      code: 'district-kampala',
      level: 'district',
      place: place('district-kampala', 'district', 'region-central'),
      places: [place('KAM013', 'facility', 'district-kampala'), place('KAM020', 'facility', 'district-kampala', false)],
      facilities: ['KAM013', 'KAM020'],
    }
    const transport = fakeTransport(() => ({ body: area }))
    const got = await areaQuery(transport).queryFn({})
    expect(transport.sent.map((r) => `${r.method} ${r.path}`)).toEqual(['GET /places/area'])
    expect(got.facilities).toEqual(['KAM013', 'KAM020'])
    expect(facilitiesByDistrict(got)).toEqual([{ district: area.place, facilities: area.places }])
  })

  test('a region groups its facilities by district', () => {
    const area: Area = {
      code: 'region-central',
      level: 'region',
      place: place('region-central', 'region', 'central'),
      places: [place('KAM013', 'facility', 'district-kampala'), place('WAK001', 'facility', 'district-wakiso'), place('district-kampala', 'district', 'region-central'), place('district-mukono', 'district', 'region-central'), place('district-wakiso', 'district', 'region-central')],
      facilities: ['KAM013', 'WAK001'],
    }
    const groups = facilitiesByDistrict(area)
    expect(groups.map((g) => [g.district?.code, g.facilities.map((f) => f.code)])).toEqual([
      ['district-kampala', ['KAM013']],
      ['district-wakiso', ['WAK001']],
    ])
  })

  test('a refusal carries its status', async () => {
    const transport = fakeTransport(() => ({ status: 404, body: undefined }))
    await expect(areaQuery(transport).queryFn({})).rejects.toEqual(new AreaError(404))
  })
})
