import type { QuestionnaireResponse } from '@health/fhir-types'

import { FhirError } from './fhir'
import type { Transport } from './types'

/**
 * Forms' commands (client-application.md § Forms; #92): a QuestionnaireResponse saved on the
 * server with its client-generated UUID, `in-progress` until it is submitted `completed`. Each
 * save after the first names the version it read (`If-Match`), so two devices never overwrite
 * each other; a stale save fails with 412. The server validates every write.
 */
export interface FormsCommands {
  /** Saves `response` (its `id` set), as the version after `versionId` if it was saved before. */
  saveResponse(response: QuestionnaireResponse, versionId: string | undefined, signal?: AbortSignal): Promise<QuestionnaireResponse>
}

/** `base` is the FHIR base relative to the transport's base. */
export function formsCommands(transport: Transport, base = '/fhir'): FormsCommands {
  return {
    async saveResponse(response, versionId, signal) {
      const result = await transport.request({
        method: 'PUT',
        path: `${base}/QuestionnaireResponse/${encodeURIComponent(response.id ?? '')}`,
        headers: {
          accept: 'application/fhir+json',
          'content-type': 'application/fhir+json',
          ...(versionId === undefined ? {} : { 'if-match': `W/"${versionId}"` }),
        },
        body: response,
        ...(signal === undefined ? {} : { signal }),
      })
      if (result.status !== 200 && result.status !== 201) throw new FhirError(result.status, undefined)
      return result.body as QuestionnaireResponse
    },
  }
}
