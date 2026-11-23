import type { Transport } from '../../platform/types'
import { type Answer, type Explorer, ExplorerError, type MissingFacility, type Question, type Tone, toneOf } from './api'

/** A `[longitude, latitude]` position. */
export type Position = [number, number]

/** A district's or region's shape (GeoJSON): one polygon or several, each its outer ring then any holes. */
export type Geometry = { type: 'Polygon'; coordinates: Position[][] } | { type: 'MultiPolygon'; coordinates: Position[][][] }

/** A district or region in the session's area with its boundary, from the national core package. */
export interface MapArea {
  code: string
  name: string
  level: 'district' | 'region'
  geometry: Geometry
}

/** An open facility in the session's area at its NHFR coordinates. */
export interface MapFacility {
  code: string
  name: string
  parent: string | null
  latitude: number
  longitude: number
}

/** What a map of the session's area can draw (`GET /api/analytics/map`). */
export interface MapPlaces {
  areas: MapArea[]
  facilities: MapFacility[]
}

/** Exact JSON numbers (lossless-json), deep in arrays, as plain numbers. */
const plain = (value: unknown): unknown => (Array.isArray(value) ? value.map(plain) : Number(value))

/** The boundaries and facility points of the session's area: the same for every map, so asked once. */
export function mapQuery(transport: Transport) {
  return {
    queryKey: ['analytics', 'map'],
    staleTime: Infinity,
    queryFn: async ({ signal }: { signal?: AbortSignal }): Promise<MapPlaces> => {
      const response = await transport.request({ method: 'GET', path: '/analytics/map', ...(signal === undefined ? {} : { signal }) })
      if (response.status !== 200 || typeof response.body !== 'object' || response.body === null) throw new ExplorerError(response.status, undefined)
      const body = response.body as MapPlaces
      return {
        areas: body.areas.map((a) => ({ ...a, geometry: { ...a.geometry, coordinates: plain(a.geometry.coordinates) } as Geometry })),
        facilities: body.facilities.map((f) => ({ ...f, latitude: Number(f.latitude), longitude: Number(f.longitude) })),
      }
    },
  }
}

/** A district or region as drawn: its SVG path, and its figure's band when it was asked. */
export interface DrawnArea {
  code: string
  name: string
  path: string
  /** False for the backdrop: the viewer's own place, or one around a place asked. */
  picked: boolean
  value: number | null
  tone: Tone | null
}

/** A facility as drawn: a point, hollow when it has not synced for the period. */
export interface DrawnPoint {
  code: string
  name: string
  x: number
  y: number
  /** Whether the facility itself was asked, so its figure colours it. */
  picked: boolean
  value: number | null
  tone: Tone | null
  /** Set when it is missing for the period: never counted, with its last sync. */
  missing: MissingFacility | null
}

/** One indicator's map for the range's last period. */
export interface DrawnMap {
  indicator: string
  period: string
  width: number
  height: number
  areas: DrawnArea[]
  points: DrawnPoint[]
}

const polygonsOf = (geometry: Geometry): Position[][][] => (geometry.type === 'Polygon' ? [geometry.coordinates] : geometry.coordinates)

/**
 * `indicator`'s map of `answer` for its last period (#604), `width` across: the districts and
 * regions asked, shaded by the band of their figure, over the viewer's own place and those around
 * the places asked; and each facility asked or inside a place asked at its NHFR coordinates,
 * coloured by its own figure when it was asked, hollow when it has not synced for the period.
 * Drawn with an equirectangular projection, offline, from the boundaries in the national core
 * package. None when nothing asked has a boundary or coordinates.
 */
export function drawnMap(answer: Answer, explorer: Explorer, question: Question, places: MapPlaces, indicator: string, width = 480): DrawnMap | null {
  const period = answer.periods[answer.periods.length - 1]?.id
  if (period === undefined) return null
  const parents = new Map([explorer.place, ...explorer.places].filter((p) => p !== null).map((p) => [p.code, p.parent]))
  /** `code` and every place above it in the session's area. */
  const lineage = (code: string): string[] => {
    const up: string[] = []
    for (let at: string | null | undefined = code; at != null && !up.includes(at); at = parents.get(at)) up.push(at)
    return up
  }
  const asked = new Set(question.places)
  const around = new Set([...question.places.flatMap(lineage), ...(explorer.place === null ? [] : [explorer.place.code])])
  const cells = answer.cells.filter((c) => c.indicator === indicator && c.period === period)
  const bands = explorer.indicators.find((i) => i.measure === indicator)?.thresholds ?? []
  const figure = (code: string) => {
    const value = cells.find((c) => c.place === code)?.value
    return value == null ? null : Number(value)
  }
  const missing = new Map(cells.flatMap((c) => c.missing ?? []).map((f) => [f.facility, f]))

  const areas = places.areas.filter((a) => asked.has(a.code) || around.has(a.code))
  const facilities = places.facilities.filter((f) => lineage(f.code).some((code) => asked.has(code)) || (f.parent !== null && lineage(f.parent).some((code) => asked.has(code))))
  const positions: Position[] = [...areas.flatMap((a) => polygonsOf(a.geometry).flat(2)), ...facilities.map((f): Position => [f.longitude, f.latitude])]
  if (positions.length === 0) return null

  const longitudes = positions.map(([x]) => x)
  const latitudes = positions.map(([, y]) => y)
  const [west, east, south, north] = [Math.min(...longitudes), Math.max(...longitudes), Math.min(...latitudes), Math.max(...latitudes)]
  // Equirectangular, a degree of longitude shortened by the cosine of the middle latitude.
  const across = Math.cos((((south + north) / 2) * Math.PI) / 180)
  const pad = 12
  const scale = (width - 2 * pad) / Math.max((east - west) * across, north - south, 1e-6)
  const height = Math.round(Math.max(120, (north - south) * scale + 2 * pad))
  const offset = (width - 2 * pad - (east - west) * across * scale) / 2
  const x = (longitude: number) => round(pad + offset + (longitude - west) * across * scale)
  const y = (latitude: number) => round(pad + (north - latitude) * scale)
  const path = (geometry: Geometry) =>
    polygonsOf(geometry)
      .flatMap((rings) => rings.map((ring) => `M${ring.map(([lon, lat]) => `${x(lon)},${y(lat)}`).join('L')}Z`))
      .join('')

  // The backdrop first, so the places asked are drawn over it.
  const drawn = areas.map((a): DrawnArea => {
    const picked = asked.has(a.code)
    const value = picked ? figure(a.code) : null
    return { code: a.code, name: a.name, path: path(a.geometry), picked, value, tone: toneOf(bands, value) }
  })
  drawn.sort((a, b) => Number(a.picked) - Number(b.picked))
  return {
    indicator,
    period,
    width,
    height,
    areas: drawn,
    points: facilities.map((f) => {
      const picked = asked.has(f.code)
      const value = picked ? figure(f.code) : null
      return { code: f.code, name: f.name, x: x(f.longitude), y: y(f.latitude), picked, value, tone: toneOf(bands, value), missing: missing.get(f.code) ?? null }
    }),
  }
}

const round = (n: number) => Math.round(n * 10) / 10
