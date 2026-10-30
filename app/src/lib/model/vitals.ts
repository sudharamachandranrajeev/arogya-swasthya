// Vital signs: the measurements people take at home, each saved as a FHIR
// Observation following the R4 vital-signs profile (LOINC code, UCUM unit,
// category "vital-signs"). Blood pressure is one Observation with two parts.

import { getPath, OBS_CATEGORY, type Resource } from './fhir.ts'

export const LOINC = 'http://loinc.org'
export const UCUM = 'http://unitsofmeasure.org'

export interface Unit {
  /** As shown and stored in valueQuantity.unit. */
  label: string
  ucum: string
  /** Values outside these are refused as typing slips. */
  min: number
  max: number
  /** A LOINC code that differs for this unit (glucose). */
  loinc?: string
}

export interface Vital {
  key: string
  label: string
  loinc: string
  /** Extra codings FHIR's profile asks for alongside the main one. */
  also?: string[]
  units: Unit[]
  /** Blood pressure: the two values, each with its own code. */
  parts?: { key: string; label: string; display: string; loinc: string }[]
  explain: string
}

const mmHg: Unit = { label: 'mmHg', ucum: 'mm[Hg]', min: 20, max: 300 }

export const vitals: Vital[] = [
  {
    key: 'bp', label: 'Blood pressure', loinc: '85354-9', units: [mmHg],
    parts: [
      { key: 'systolic', label: 'Top number', display: 'Systolic', loinc: '8480-6' },
      { key: 'diastolic', label: 'Bottom number', display: 'Diastolic', loinc: '8462-4' },
    ],
    explain: 'Two numbers, such as 120 over 80. Sit quietly for five minutes first.',
  },
  { key: 'pulse', label: 'Pulse', loinc: '8867-4', units: [{ label: 'beats/min', ucum: '/min', min: 20, max: 250 }], explain: 'Heartbeats in one minute, at rest.' },
  {
    key: 'weight', label: 'Weight', loinc: '29463-7',
    units: [{ label: 'kg', ucum: 'kg', min: 0.5, max: 400 }, { label: 'lb', ucum: '[lb_av]', min: 1, max: 900 }],
    explain: 'Same scale, same time of day, makes the trend clearer.',
  },
  {
    key: 'temperature', label: 'Temperature', loinc: '8310-5',
    units: [{ label: '°C', ucum: 'Cel', min: 30, max: 45 }, { label: '°F', ucum: '[degF]', min: 86, max: 113 }],
    explain: 'Body temperature.',
  },
  { key: 'spo2', label: 'Oxygen level', loinc: '2708-6', also: ['59408-5'], units: [{ label: '%', ucum: '%', min: 50, max: 100 }], explain: 'SpO2 from a finger clip (pulse oximeter).' },
  {
    key: 'glucose', label: 'Blood sugar', loinc: '2339-0',
    units: [{ label: 'mmol/L', ucum: 'mmol/L', min: 1, max: 40, loinc: '15074-8' }, { label: 'mg/dL', ucum: 'mg/dL', min: 18, max: 720 }],
    explain: 'A finger-prick or sensor reading.',
  },
  { key: 'breathing', label: 'Breathing rate', loinc: '9279-1', units: [{ label: 'breaths/min', ucum: '/min', min: 4, max: 80 }], explain: 'Breaths in one minute, at rest.' },
  {
    key: 'height', label: 'Height', loinc: '8302-2',
    units: [{ label: 'cm', ucum: 'cm', min: 30, max: 250 }, { label: 'in', ucum: '[in_i]', min: 12, max: 100 }],
    explain: 'Standing, without shoes.',
  },
  {
    key: 'waist', label: 'Waist', loinc: '8280-0',
    units: [{ label: 'cm', ucum: 'cm', min: 30, max: 250 }, { label: 'in', ucum: '[in_i]', min: 12, max: 100 }],
    explain: 'Around the middle, halfway between the lowest rib and the top of the hip bone.',
  },
]

const codes = (r: unknown, path: string) =>
  ((getPath(r, path) as { system?: string; code?: string }[] | undefined) ?? []).filter((c) => c?.system === LOINC).map((c) => c.code)

export const isVital = (r: Resource) =>
  r.resourceType === 'Observation' &&
  ((r.category as { coding?: { code?: string }[] }[] | undefined) ?? []).some((c) => c.coding?.some((x) => x.code === 'vital-signs'))

/** Which vital an Observation holds, by its LOINC codes. */
export function vitalOf(r: Resource): Vital | undefined {
  const cs = codes(r, 'code.coding')
  return vitals.find((v) => cs.includes(v.loinc) || v.also?.some((a) => cs.includes(a)) || v.units.some((u) => u.loinc && cs.includes(u.loinc)))
}

/** What one vital looks like in the form: the value (or two, for blood
 *  pressure) and the unit chosen. */
export type Reading = { unit: string; values: Record<string, string> }

const quantity = (n: number, u: Unit) => ({ value: n, unit: u.label, system: UCUM, code: u.ucum })
const concept = (codes: string[], display: string) => ({
  coding: codes.map((code) => ({ system: LOINC, code })),
  text: display,
})

/** Problems in a reading, in plain words. */
export function checkReading(v: Vital, r: Reading): string[] {
  const u = v.units.find((x) => x.label === r.unit) ?? v.units[0]
  const keys = v.parts?.map((p) => p.key) ?? ['value']
  const filled = keys.filter((k) => (r.values[k] ?? '').trim())
  if (!filled.length) return []
  if (filled.length < keys.length) return [`${v.label}: fill in both numbers.`]
  const out: string[] = []
  for (const k of keys) {
    const n = Number(r.values[k].trim())
    if (!Number.isFinite(n)) out.push(`${v.label} must be a number.`)
    else if (n < u.min || n > u.max) out.push(`${v.label}: ${n} ${u.label} looks like a typing slip (expected ${u.min} to ${u.max}).`)
  }
  if (v.parts && !out.length && Number(r.values.systolic) <= Number(r.values.diastolic))
    out.push(`${v.label}: the top number is usually the larger one.`)
  return out
}

export const filled = (v: Vital, r: Reading | undefined) =>
  !!r && (v.parts?.map((p) => p.key) ?? ['value']).some((k) => (r.values[k] ?? '').trim())

/** The Observation for a reading. Starts from `old` when editing, so fields
 *  this app does not show are kept. */
export function toObservation(v: Vital, r: Reading, person: string, date: string, old?: Resource): Resource {
  const u = v.units.find((x) => x.label === r.unit) ?? v.units[0]
  const o: Resource = old
    ? JSON.parse(JSON.stringify(old))
    : {
        resourceType: 'Observation',
        status: 'final',
        category: [{ coding: [{ system: OBS_CATEGORY, code: 'vital-signs', display: 'Vital Signs' }], text: 'Vital signs' }],
      }
  o.code = concept([u.loinc ?? v.loinc, ...(v.also ?? [])], v.label)
  o.subject = { reference: `Patient/${person}` }
  o.effectiveDateTime = date
  delete o.valueQuantity
  delete o.component
  if (v.parts)
    o.component = v.parts.map((p) => ({ code: concept([p.loinc], p.display), valueQuantity: quantity(Number(r.values[p.key]), u) }))
  else o.valueQuantity = quantity(Number(r.values.value), u)
  return o
}

/** A stored Observation back into a reading. */
export function toReading(v: Vital, o: Resource): Reading {
  if (v.parts) {
    const comps = (o.component as Resource[] | undefined) ?? []
    const values: Record<string, string> = {}
    let unit = v.units[0].label
    for (const p of v.parts) {
      const c = comps.find((c) => codes(c, 'code.coding').includes(p.loinc))
      const q = c?.valueQuantity as { value?: number; unit?: string } | undefined
      values[p.key] = q?.value != null ? String(q.value) : ''
      if (q?.unit) unit = q.unit
    }
    return { unit, values }
  }
  const q = o.valueQuantity as { value?: number; unit?: string; code?: string } | undefined
  const u = v.units.find((x) => x.ucum === q?.code || x.label === q?.unit) ?? v.units[0]
  return { unit: u.label, values: { value: q?.value != null ? String(q.value) : '' } }
}

/** "128/82 mmHg", "71.5 kg". */
export function show(o: Resource): string {
  const v = vitalOf(o)
  if (v) {
    const r = toReading(v, o)
    const nums = (v.parts?.map((p) => p.key) ?? ['value']).map((k) => r.values[k]).filter(Boolean)
    if (nums.length) return `${nums.join('/')} ${r.unit}`
  }
  const q = o.valueQuantity as { value?: number; unit?: string } | undefined
  return q?.value != null ? `${q.value} ${q.unit ?? ''}`.trim() : ''
}
