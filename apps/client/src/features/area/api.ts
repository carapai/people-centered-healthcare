import type { Transport } from '../../platform/types'

/** Where a place sits in the facility registry's hierarchy (ADR 0094). */
export type PlaceLevel = 'facility' | 'district' | 'region' | 'national'

/** One place of the facility registry (`health_facility_registry::Place`). */
export interface Place {
  code: string
  name: string
  level: PlaceLevel
  parent: string | null
  /** False once the NHFR closes it; it is kept. */
  active: boolean
  care_level?: string
  ownership?: string
  coordinates?: { latitude: string; longitude: string }
}

/** The session's place and its area (`GET /api/places/area`). */
export interface Area {
  code: string
  level: PlaceLevel
  /** The place, when the registry holds it. */
  place: Place | null
  /** Every place below it, by code. */
  places: Place[]
  /** The codes of the facilities in the area: what its figures roll up over. */
  facilities: string[]
}

/** The area could not be read: the status, for the page to explain. */
export class AreaError extends Error {
  constructor(readonly status: number) {
    super(`the area could not be read: ${status}`)
    this.name = 'AreaError'
  }
}

/** The signed-in session's place and its area. */
export function areaQuery(transport: Transport) {
  return {
    queryKey: ['places', 'area'],
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<Area> => {
      const response = await transport.request({ method: 'GET', path: '/places/area', ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || typeof response.body !== 'object' || response.body === null) throw new AreaError(response.status)
      return response.body as Area
    },
  }
}

/** The facilities of an area, by district: each district (or the place itself) with its facilities in code order. */
export function facilitiesByDistrict(area: Area): { district: Place | null; facilities: Place[] }[] {
  const facilities = area.places.filter((p) => p.level === 'facility')
  const districts = area.level === 'district' ? [area.place] : area.places.filter((p) => p.level === 'district')
  const groups = districts.map((district) => ({ district, facilities: facilities.filter((f) => f.parent === district?.code) }))
  return groups.filter((g) => g.facilities.length > 0)
}
