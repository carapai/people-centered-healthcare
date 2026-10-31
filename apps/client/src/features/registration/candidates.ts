import type { Bundle, Patient } from '@health/fhir-types'

/** One "did you mean" candidate, as the list shows it. */
export interface Candidate {
  id: string
  name: string
  birthDate: string
  sex: string
  mrn: string
  /** The match grade (`certain`, `probable`, `possible`). */
  grade: string
  score: number
}

/** The candidates of a `$match` or search Bundle, in its order (best first). */
export function candidates(bundle: Bundle): Candidate[] {
  return (bundle.entry ?? []).flatMap((entry) => {
    const patient = entry.resource as Patient | undefined
    if (patient?.resourceType !== 'Patient' || patient.id === undefined) return []
    const name = patient.name?.[0]
    const mrn = patient.identifier?.find((i) => i.type?.coding?.some((c) => c.code === 'MR'))?.value ?? ''
    const grade = entry.search?.extension?.find((e) => e.url === 'http://hl7.org/fhir/StructureDefinition/match-grade')?.valueCode ?? ''
    return [
      {
        id: patient.id,
        name: [name?.given?.join(' '), name?.family].filter((part) => part !== undefined && part !== '').join(' '),
        birthDate: patient.birthDate ?? '',
        sex: patient.gender ?? '',
        mrn,
        grade,
        score: Number(entry.search?.score ?? 0),
      },
    ]
  })
}
