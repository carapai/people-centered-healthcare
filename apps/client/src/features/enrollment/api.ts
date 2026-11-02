import { isLosslessNumber } from '../../platform/json'
import type { ApiResponse, Transport } from '../../platform/types'

/**
 * The edge's device enrollment (lan-security.md § Device enrollment; ADR 0009; health-auth's
 * `DeviceGate`). The device's own calls carry a DPoP proof by its key; the registry's admin
 * routes answer the edge's own screen only.
 */

/** A device in the edge's registry. */
export interface Device {
  id: string
  name: string
  location: string
  /** What the device said it was when it asked. */
  label: string
  enrolled_by: string
  enrolled_at: string
  status: 'active' | 'revoked'
  revoked_by: string | null
  revoked_at: string | null
}

/** Where this device stands. */
export type Enrollment =
  | { status: 'none' }
  | { status: 'pending'; code: string; expires: string }
  | { status: 'enrolled'; device: Device }
  | { status: 'revoked' }

/** A request waiting for an admin; its code is not listed, the admin types the one the device shows. */
export interface PendingRequest {
  id: string
  label: string
  requested_at: string
  expires: string
}

/** A refusal, by the edge's error code (`no-such-code`, `expired`, `device-revoked`, …). */
export class EnrollmentError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
  ) {
    super(`enrollment: ${status} ${code}`)
    this.name = 'EnrollmentError'
  }
}

function body(response: ApiResponse): Record<string, unknown> {
  const value = response.body
  return typeof value === 'object' && value !== null && !Array.isArray(value) && !isLosslessNumber(value) ? (value as Record<string, unknown>) : {}
}

function refused(response: ApiResponse): EnrollmentError {
  const error = body(response)['error']
  return new EnrollmentError(response.status, typeof error === 'string' ? error : String(response.status))
}

/** Asks the edge to enroll this device; the pairing code to show. */
export async function requestEnrollment(transport: Transport, label: string): Promise<{ code: string; expires: string }> {
  const response = await transport.request({ method: 'POST', path: '/enrollment', body: { label } })
  if (response.status !== 201) throw refused(response)
  const { code, expires } = body(response)
  return { code: String(code), expires: String(expires) }
}

/** Where this device stands, by its key. */
export async function enrollmentStatus(transport: Transport, signal?: AbortSignal): Promise<Enrollment> {
  const response = await transport.request({ method: 'GET', path: '/enrollment', ...(signal === undefined ? {} : { signal }) })
  if (response.status === 404) return { status: 'none' }
  if (response.status !== 200) throw refused(response)
  return body(response) as unknown as Enrollment
}

/** The device as the gate sees it: answers only an enrolled, active device. */
export async function thisDevice(transport: Transport, signal?: AbortSignal): Promise<Device> {
  const response = await transport.request({ method: 'GET', path: '/device', ...(signal === undefined ? {} : { signal }) })
  if (response.status !== 200) throw refused(response)
  return body(response) as unknown as Device
}

/** The requests waiting for an admin. */
export async function pendingRequests(transport: Transport, signal?: AbortSignal): Promise<PendingRequest[]> {
  const response = await transport.request({ method: 'GET', path: '/admin/enrollments', ...(signal === undefined ? {} : { signal }) })
  if (response.status !== 200 || !Array.isArray(response.body)) throw refused(response)
  return response.body as PendingRequest[]
}

/** Approves the request showing `code`, naming the device and placing it. */
export async function approve(transport: Transport, approval: { code: string; name: string; location: string }): Promise<Device> {
  const response = await transport.request({ method: 'POST', path: '/admin/enrollments/approve', body: approval })
  if (response.status !== 201) throw refused(response)
  return body(response) as unknown as Device
}

/** Every device the edge has enrolled, revoked ones too. */
export async function devices(transport: Transport, signal?: AbortSignal): Promise<Device[]> {
  const response = await transport.request({ method: 'GET', path: '/admin/devices', ...(signal === undefined ? {} : { signal }) })
  if (response.status !== 200 || !Array.isArray(response.body)) throw refused(response)
  return response.body as Device[]
}

/** Revokes a device: its next request is refused. */
export async function revoke(transport: Transport, id: string): Promise<Device> {
  const response = await transport.request({ method: 'POST', path: `/admin/devices/${encodeURIComponent(id)}/revoke` })
  if (response.status !== 200) throw refused(response)
  return body(response) as unknown as Device
}

/** A short label for this device from its user agent, e.g. `Chrome on Windows`, for the admin to recognise it. */
export function deviceLabel(userAgent: string): string {
  const browser = /Edg\//.test(userAgent)
    ? 'Edge'
    : /Firefox\//.test(userAgent)
      ? 'Firefox'
      : /Chrome\//.test(userAgent)
        ? 'Chrome'
        : /Safari\//.test(userAgent)
          ? 'Safari'
          : 'Browser'
  const system = /Android/.test(userAgent)
    ? 'Android'
    : /iPhone|iPad/.test(userAgent)
      ? 'iOS'
      : /Windows/.test(userAgent)
        ? 'Windows'
        : /Mac OS X/.test(userAgent)
          ? 'macOS'
          : /Linux/.test(userAgent)
            ? 'Linux'
            : 'an unknown system'
  return `${browser} on ${system}`
}
