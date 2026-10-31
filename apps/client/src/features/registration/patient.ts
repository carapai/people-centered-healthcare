import type { Patient } from '@health/fhir-types'

/** What the clerk enters at reception. */
export interface RegistrationForm {
  family: string
  given: string
  sex: '' | 'female' | 'male' | 'other' | 'unknown'
  /** `YYYY-MM-DD`, or empty when only an age is known. */
  birthDate: string
  /** Years, when the birth date is not known: the birth year is estimated from it. */
  age: string
  phone: string
  nin: string
  district: string
  village: string
}

export const emptyForm: RegistrationForm = {
  family: '',
  given: '',
  sex: '',
  birthDate: '',
  age: '',
  phone: '',
  nin: '',
  district: '',
  village: '',
}

/**
 * The Patient the clerk entered. An age gives an estimated birth year (`birthDate` of four
 * digits), which matching widens by ±5 years (ADR 0055). Values are staff-entered; a NIN is
 * NIRA-verified only after verification.
 */
export function toPatient(form: RegistrationForm, ninSystem: string, thisYear: number): Patient {
  const patient: Patient = {
    resourceType: 'Patient',
    meta: { tag: [{ system: 'urn:health:registry:trust', code: 'staff-entered' }] },
  }
  const family = form.family.trim()
  const given = form.given.trim()
  if (family !== '' || given !== '') {
    patient.name = [{ ...(family === '' ? {} : { family }), ...(given === '' ? {} : { given: given.split(/\s+/) }) }]
  }
  if (form.sex !== '') patient.gender = form.sex
  const age = Number.parseInt(form.age, 10)
  if (form.birthDate !== '') patient.birthDate = form.birthDate
  else if (Number.isInteger(age) && age >= 0 && age < 130) patient.birthDate = String(thisYear - age)
  const nin = form.nin.trim().toUpperCase()
  if (nin !== '') patient.identifier = [{ system: ninSystem, value: nin }]
  const phone = form.phone.trim()
  if (phone !== '') patient.telecom = [{ system: 'phone', value: phone }]
  const district = form.district.trim()
  const village = form.village.trim()
  if (district !== '' || village !== '') {
    patient.address = [{ ...(village === '' ? {} : { city: village }), ...(district === '' ? {} : { district }) }]
  }
  return patient
}

/** Whether the form has the minimum to register: a name, and sex. */
export function canRegister(form: RegistrationForm): boolean {
  return form.family.trim() !== '' && form.given.trim() !== '' && form.sex !== ''
}
