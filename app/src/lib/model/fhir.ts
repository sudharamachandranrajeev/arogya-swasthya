// The record kinds people can type in, and how each maps to FHIR R4.
// Forms work on flat string values; editing starts from the stored resource so
// fields this app does not show (from an import, say) are kept as they were.

import { lists } from './lists.ts'
import { isVital, show, vitalOf } from './vitals.ts'

export type Resource = { resourceType: string; id?: string; [k: string]: unknown }
export type Values = Record<string, string>

export interface Field {
  key: string
  label: string
  type: 'text' | 'date' | 'select' | 'number' | 'note'
  /** JSON path inside the resource ("a.0.b"). */
  path: string
  options?: [value: string, label: string][]
  /** Suggestions shown while typing (free text is still allowed). */
  list?: readonly string[]
  required?: boolean
  /** Numbers: the smallest and largest allowed. */
  min?: number
  max?: number
  /** Dates: a new record starts at today. */
  today?: boolean
  explain?: string
}

export interface Kind {
  type: string
  /** Page title, plural. */
  title: string
  /** One record, lower case: "a condition". */
  one: string
  explain: string
  /** The page that lists this kind. */
  page: string
  /** Kinds sharing a FHIR type (Observation) tell their records apart. */
  match?: (r: Resource) => boolean
  /** Field that points at the person, or '' for Patient. */
  personField: string
  fields: Field[]
  /** Fixed values every new record of this kind starts with. */
  base: Record<string, unknown>
  summary: (r: Resource) => { title: string; sub: string; date?: string }
}

/** Deep copy. JSON is enough for FHIR, and unlike structuredClone it also
 *  copies Svelte state proxies. */
export const clone = <T,>(v: T): T => JSON.parse(JSON.stringify(v))

// ---- paths ----

const parts = (path: string) => path.split('.').map((p) => (/^\d+$/.test(p) ? Number(p) : p))

export function getPath(r: unknown, path: string): unknown {
  let v: unknown = r
  for (const p of parts(path)) {
    if (v == null || typeof v !== 'object') return undefined
    v = (v as Record<string | number, unknown>)[p]
  }
  return v
}

/** Set a value, making objects and arrays on the way; `undefined` removes it
 *  and any container left empty by the removal. */
export function setPath(r: Record<string, unknown>, path: string, value: unknown) {
  const ps = parts(path)
  const chain: [Record<string | number, unknown>, string | number][] = []
  let o = r as Record<string | number, unknown>
  for (let i = 0; i < ps.length - 1; i++) {
    const p = ps[i]
    if (o[p] == null || typeof o[p] !== 'object') {
      if (value === undefined) return
      o[p] = typeof ps[i + 1] === 'number' ? [] : {}
    }
    chain.push([o, p])
    o = o[p] as Record<string | number, unknown>
  }
  const last = ps[ps.length - 1]
  if (value === undefined) {
    if (Array.isArray(o) && typeof last === 'number') o.splice(last, 1)
    else delete o[last]
    for (let i = chain.length - 1; i >= 0; i--) {
      const [parent, key] = chain[i]
      const child = parent[key] as object
      if (Object.keys(child).length) break
      if (Array.isArray(parent) && typeof key === 'number') parent.splice(key, 1)
      else delete parent[key]
    }
  } else o[last] = value
}

// ---- codings ----

const coded = (system: string, code: string) => ({ coding: [{ system, code }] })
const CLINICAL = 'http://terminology.hl7.org/CodeSystem/condition-clinical'
const ALLERGY_CLINICAL = 'http://terminology.hl7.org/CodeSystem/allergyintolerance-clinical'
export const OBS_CATEGORY = 'http://terminology.hl7.org/CodeSystem/observation-category'

/** Fields whose form value is a code inside a CodeableConcept. */
const CODED: Record<string, string> = {
  'clinicalStatus.coding.0.code': CLINICAL,
}

const text = (r: Resource, path: string) => {
  const v = getPath(r, path)
  return typeof v === 'string' || typeof v === 'number' ? String(v) : ''
}
const label = (f: Field | undefined, v: string) => f?.options?.find(([k]) => k === v)?.[1] ?? v
const join = (...xs: string[]) => xs.filter(Boolean).join(' · ')
const note: Field = { key: 'note', label: 'Notes', type: 'note', path: 'note.0.text' }

// ---- kinds ----

export const patient: Kind = {
  type: 'Patient',
  page: 'settings',
  title: 'People',
  one: 'a person',
  explain: 'Everyone whose record you keep here: you, and anyone you look after.',
  personField: '',
  base: {},
  fields: [
    { key: 'name', label: 'Name', type: 'text', path: 'name.0.text', required: true },
    { key: 'birthDate', label: 'Date of birth', type: 'date', path: 'birthDate', explain: 'Used for age-based checks and plans later. A year alone is fine.' },
    {
      key: 'gender', label: 'Sex', type: 'select', path: 'gender',
      options: [['female', 'Female'], ['male', 'Male'], ['other', 'Other'], ['unknown', 'Prefer not to say']],
      explain: 'Some screening and reference ranges differ by sex. FHIR calls this field "gender".',
    },
  ],
  summary: (r) => ({ title: text(r, 'name.0.text') || 'Unnamed', sub: join(text(r, 'birthDate'), label(patient.fields[2], text(r, 'gender'))) }),
}

export const condition: Kind = {
  type: 'Condition',
  page: 'records',
  title: 'Conditions',
  one: 'a condition',
  explain: 'Illnesses, injuries and long-term conditions, past and present.',
  personField: 'subject',
  base: {},
  fields: [
    { key: 'name', label: 'Condition', type: 'text', path: 'code.text', list: lists.conditions, required: true },
    {
      key: 'status', label: 'Status', type: 'select', path: 'clinicalStatus.coding.0.code', required: true,
      options: [['active', 'Active'], ['recurrence', 'Comes and goes'], ['remission', 'In remission'], ['resolved', 'Resolved']],
      explain: '"In remission" means no signs now but it may return; "resolved" means it is over.',
    },
    { key: 'onset', label: 'Started', type: 'date', path: 'onsetDateTime' },
    { key: 'site', label: 'Part of the body', type: 'text', path: 'bodySite.0.text', list: lists.bodySites },
    note,
  ],
  summary: (r) => ({
    title: text(r, 'code.text'),
    sub: join(label(condition.fields[1], text(r, 'clinicalStatus.coding.0.code')), text(r, 'bodySite.0.text')),
    date: text(r, 'onsetDateTime'),
  }),
}

export const medication: Kind = {
  type: 'MedicationStatement',
  page: 'medicines',
  title: 'Medicines',
  one: 'a medicine',
  explain: 'Medicines and supplements you take or have taken, with the dose as written on the label.',
  personField: 'subject',
  base: {},
  fields: [
    { key: 'name', label: 'Medicine', type: 'text', path: 'medicationCodeableConcept.text', list: lists.medicines, required: true, explain: 'Name and strength as on the box, e.g. "Metformin 500 mg".' },
    { key: 'dose', label: 'How you take it', type: 'text', path: 'dosage.0.text', explain: 'In your own words, e.g. "one tablet twice a day with food".' },
    {
      key: 'status', label: 'Status', type: 'select', path: 'status', required: true,
      options: [['active', 'Taking now'], ['on-hold', 'Paused'], ['completed', 'Finished the course'], ['stopped', 'Stopped']],
    },
    { key: 'start', label: 'Started', type: 'date', path: 'effectivePeriod.start' },
    { key: 'end', label: 'Ended', type: 'date', path: 'effectivePeriod.end' },
    { key: 'reason', label: 'What it is for', type: 'text', path: 'reasonCode.0.text', list: lists.conditions },
    note,
  ],
  summary: (r) => ({
    title: text(r, 'medicationCodeableConcept.text'),
    sub: join(text(r, 'dosage.0.text'), label(medication.fields[2], text(r, 'status'))),
    date: text(r, 'effectivePeriod.start'),
  }),
}

export const allergy: Kind = {
  type: 'AllergyIntolerance',
  page: 'records',
  title: 'Allergies',
  one: 'an allergy',
  explain: 'Allergies and intolerances to medicines, foods and other things, with what happens.',
  personField: 'patient',
  base: { clinicalStatus: coded(ALLERGY_CLINICAL, 'active') },
  fields: [
    { key: 'name', label: 'Allergic to', type: 'text', path: 'code.text', list: lists.allergens, required: true },
    {
      key: 'category', label: 'Kind', type: 'select', path: 'category.0',
      options: [['medication', 'Medicine'], ['food', 'Food'], ['environment', 'Environment (pollen, dust, animals)'], ['biologic', 'Biologic (blood products, vaccines)']],
    },
    { key: 'reaction', label: 'What happens', type: 'text', path: 'reaction.0.manifestation.0.text', list: lists.reactions },
    {
      key: 'criticality', label: 'How serious', type: 'select', path: 'criticality',
      options: [['low', 'Mild'], ['high', 'Serious'], ['unable-to-assess', 'Not sure']],
      explain: '"Serious" means a reaction could be life-threatening, such as swelling of the throat or anaphylaxis.',
    },
    { key: 'onset', label: 'First noticed', type: 'date', path: 'onsetDateTime' },
    note,
  ],
  summary: (r) => ({
    title: text(r, 'code.text'),
    sub: join(text(r, 'reaction.0.manifestation.0.text'), text(r, 'criticality') === 'high' ? 'Serious' : ''),
    date: text(r, 'onsetDateTime'),
  }),
}

export const procedure: Kind = {
  type: 'Procedure',
  page: 'records',
  title: 'Procedures',
  one: 'a procedure',
  explain: 'Operations and other procedures done to you, such as surgery, scans with injections or endoscopies.',
  personField: 'subject',
  base: { status: 'completed' },
  fields: [
    { key: 'name', label: 'Procedure', type: 'text', path: 'code.text', list: lists.procedures, required: true },
    { key: 'date', label: 'Date', type: 'date', path: 'performedDateTime' },
    { key: 'site', label: 'Part of the body', type: 'text', path: 'bodySite.0.text', list: lists.bodySites },
    { key: 'place', label: 'Where', type: 'text', path: 'location.display', explain: 'Hospital or clinic, if you know it.' },
    note,
  ],
  summary: (r) => ({ title: text(r, 'code.text'), sub: join(text(r, 'bodySite.0.text'), text(r, 'location.display')), date: text(r, 'performedDateTime') }),
}

export const vaccine: Kind = {
  type: 'Immunization',
  page: 'records',
  title: 'Vaccines',
  one: 'a vaccine',
  explain: 'Vaccines you have had, with the date and dose number where you know them.',
  personField: 'patient',
  base: { status: 'completed' },
  fields: [
    { key: 'name', label: 'Vaccine', type: 'text', path: 'vaccineCode.text', list: lists.vaccines, required: true },
    { key: 'date', label: 'Date', type: 'date', path: 'occurrenceDateTime', required: true },
    { key: 'dose', label: 'Dose', type: 'text', path: 'protocolApplied.0.doseNumberString', explain: 'Which dose this was, e.g. "1", "2" or "booster".' },
    { key: 'lot', label: 'Batch number', type: 'text', path: 'lotNumber', explain: 'Printed on your vaccination card. Useful if there is ever a recall.' },
    note,
  ],
  summary: (r) => ({ title: text(r, 'vaccineCode.text'), sub: join(text(r, 'protocolApplied.0.doseNumberString') && `Dose ${text(r, 'protocolApplied.0.doseNumberString')}`), date: text(r, 'occurrenceDateTime') }),
}

export const symptom: Kind = {
  type: 'Observation',
  page: 'track',
  match: (r) => ((r.category as { text?: string }[] | undefined) ?? []).some((c) => c.text === 'Symptom'),
  title: 'Symptoms',
  one: 'a symptom',
  explain: 'How you felt: what it was and how bad, from 0 (none) to 10 (the worst you can imagine), the scale most doctors use.',
  base: { status: 'final', category: [{ coding: [{ system: OBS_CATEGORY, code: 'survey' }], text: 'Symptom' }] },
  personField: 'subject',
  fields: [
    { key: 'name', label: 'Symptom', type: 'text', path: 'code.text', list: lists.symptoms, required: true },
    { key: 'severity', label: 'How bad (0 to 10)', type: 'number', path: 'valueInteger', required: true, min: 0, max: 10, explain: '0 is none, 10 is the worst you can imagine.' },
    { key: 'date', label: 'Date', type: 'date', path: 'effectiveDateTime', required: true, today: true },
    note,
  ],
  summary: (r) => ({
    title: text(r, 'code.text'),
    sub: text(r, 'valueInteger') ? `${text(r, 'valueInteger')} out of 10` : '',
    date: text(r, 'effectiveDateTime'),
  }),
}

export const vital: Kind = {
  type: 'Observation',
  page: 'track',
  match: (r) => isVital(r),
  title: 'Vital signs',
  one: 'a vital sign',
  explain: 'Blood pressure, pulse, weight, temperature and other measurements taken at home or in clinic.',
  base: {},
  personField: 'subject',
  // Entered on the Track page's own form, one or many at a time.
  fields: [],
  summary: (r) => ({
    title: vitalOf(r)?.label ?? text(r, 'code.text'),
    sub: show(r),
    date: text(r, 'effectiveDateTime'),
  }),
}

export const result: Kind = {
  type: 'Observation',
  page: 'results',
  title: 'Results',
  one: 'a result',
  explain: 'Test results such as blood tests, with the range the lab calls normal. Charts arrive in a later version.',
  personField: 'subject',
  base: { status: 'final', category: [coded(OBS_CATEGORY, 'laboratory')] },
  fields: [
    { key: 'name', label: 'Test', type: 'text', path: 'code.text', list: lists.tests, required: true },
    { key: 'value', label: 'Result', type: 'text', path: 'value', required: true, explain: 'The number, or words such as "negative".' },
    { key: 'unit', label: 'Unit', type: 'text', path: 'valueQuantity.unit', list: lists.units },
    { key: 'low', label: 'Normal from', type: 'number', path: 'referenceRange.0.low.value', explain: 'The lowest normal value printed by the lab. Labs differ, so copy theirs.' },
    { key: 'high', label: 'Normal up to', type: 'number', path: 'referenceRange.0.high.value', explain: 'The highest normal value printed by the lab.' },
    { key: 'date', label: 'Date taken', type: 'date', path: 'effectiveDateTime', required: true },
    note,
  ],
  summary: (r) => {
    const v = resultValue(r)
    const band = classify(v.value, v.low, v.high)
    return {
      title: text(r, 'code.text'),
      sub: join(join(v.value, v.unit).replace(' · ', ' '), band === 'low' ? 'Below range' : band === 'high' ? 'Above range' : band === 'in' ? 'In range' : ''),
      date: text(r, 'effectiveDateTime'),
    }
  },
}

export const document: Kind = {
  type: 'DocumentReference',
  page: 'documents',
  title: 'Documents',
  one: 'a document',
  explain: 'Letters, reports, scans, photos and recordings. Each file is encrypted and kept on this device with the records.',
  personField: 'subject',
  base: { status: 'current' },
  fields: [
    { key: 'title', label: 'What it is', type: 'text', path: 'description', required: true, explain: 'A few words you will recognise later, such as "Knee MRI report".' },
    { key: 'kind', label: 'Kind of document', type: 'text', path: 'type.text', list: lists.documents },
    { key: 'date', label: 'Date on the document', type: 'date', path: 'context.period.start' },
    { key: 'from', label: 'From', type: 'text', path: 'author.0.display', explain: 'Who wrote or made it: a doctor, clinic or lab.' },
  ],
  summary: (r) => ({
    title: text(r, 'description') || text(r, 'content.0.attachment.title'),
    sub: join(text(r, 'type.text'), text(r, 'author.0.display'), text(r, 'content.0.attachment.title')),
    date: text(r, 'context.period.start'),
  }),
}

/** The vault file a document points at, if any. */
export function attachment(r: Resource) {
  const url = text(r, 'content.0.attachment.url')
  if (!url.startsWith('swasthya-file:')) return null
  return {
    id: url.slice('swasthya-file:'.length),
    name: text(r, 'content.0.attachment.title') || 'document',
    type: text(r, 'content.0.attachment.contentType'),
    size: Number(getPath(r, 'content.0.attachment.size') ?? 0),
  }
}

export const kinds = [condition, medication, allergy, procedure, vaccine, vital, symptom, result, document]
/** The kind a record belongs to. Results take any Observation that is not
 *  a vital sign or a symptom. */
export const kindOf = (r: Resource) => [patient, ...kinds].find((k) => k.type === r.resourceType && (!k.match || k.match(r)))

// ---- results ----

export type Band = 'low' | 'in' | 'high' | null

/** Where a numeric result sits against its range. Needs a number and at least one bound. */
export function classify(value: string, low?: string, high?: string): Band {
  const v = Number(value)
  if (value.trim() === '' || !Number.isFinite(v)) return null
  const lo = low?.trim() ? Number(low) : NaN
  const hi = high?.trim() ? Number(high) : NaN
  if (Number.isFinite(lo) && v < lo) return 'low'
  if (Number.isFinite(hi) && v > hi) return 'high'
  return Number.isFinite(lo) || Number.isFinite(hi) ? 'in' : null
}

function resultValue(r: Resource) {
  const value = r.valueQuantity != null ? text(r, 'valueQuantity.value') : text(r, 'valueString')
  return { value, unit: text(r, 'valueQuantity.unit'), low: text(r, 'referenceRange.0.low.value'), high: text(r, 'referenceRange.0.high.value') }
}

// ---- form <-> resource ----

export function fromFhir(kind: Kind, r: Resource): Values {
  const v: Values = {}
  for (const f of kind.fields) {
    if (f.path === 'value') v[f.key] = resultValue(r).value
    else v[f.key] = text(r, f.path)
  }
  return v
}

/** Build the resource to save. Starts from `old` (when editing) so unknown fields survive. */
export function toFhir(kind: Kind, values: Values, person: string, old?: Resource): Resource {
  const r: Resource = old ? clone(old) : { resourceType: kind.type, ...clone(kind.base) }
  if (kind.personField) r[kind.personField] = { reference: `Patient/${person}` }
  for (const f of kind.fields) {
    const raw = (values[f.key] ?? '').trim()
    if (f.path === 'value') continue
    if (f.path in CODED) {
      setPath(r, f.path.replace(/\.coding\.0\.code$/, ''), raw ? coded(CODED[f.path], raw) : undefined)
      continue
    }
    if (f.type === 'number') {
      const n = Number(raw)
      setPath(r, f.path, raw && Number.isFinite(n) ? n : undefined)
    } else setPath(r, f.path, raw || undefined)
  }
  if (kind === result) {
    const value = (values.value ?? '').trim()
    const unit = (values.unit ?? '').trim()
    const n = Number(value)
    delete r.valueString
    delete r.valueQuantity
    if (value && Number.isFinite(n)) r.valueQuantity = unit ? { value: n, unit } : { value: n }
    else if (value) r.valueString = value
    // Range bounds carry the result's unit, as FHIR expects.
    for (const end of ['low', 'high'])
      if (getPath(r, `referenceRange.0.${end}.value`) != null) setPath(r, `referenceRange.0.${end}.unit`, unit || undefined)
  }
  if (kind === condition && !r.recordedDate) r.recordedDate = new Date().toISOString().slice(0, 10)
  return r
}

/** Problems that stop a save, in plain words. Empty when the values are fine. */
export function check(kind: Kind, values: Values): string[] {
  const out: string[] = []
  for (const f of kind.fields) {
    const v = (values[f.key] ?? '').trim()
    if (f.required && !v) out.push(`${f.label} is needed.`)
    if (v && f.type === 'date' && !/^\d{4}(-\d{2}(-\d{2})?)?$/.test(v)) out.push(`${f.label}: use a date like 2024-03-15, 2024-03 or 2024.`)
    if (v && f.type === 'number') {
      const n = Number(v)
      if (!Number.isFinite(n)) out.push(`${f.label} must be a number.`)
      else if ((f.min != null && n < f.min) || (f.max != null && n > f.max)) out.push(`${f.label} must be from ${f.min} to ${f.max}.`)
      else if (f.path.endsWith('Integer') && !Number.isInteger(n)) out.push(`${f.label} must be a whole number.`)
    }
  }
  const today = new Date().toISOString().slice(0, 10)
  for (const f of kind.fields)
    if (f.type === 'date' && values[f.key] && values[f.key] > today) out.push(`${f.label} is in the future.`)
  return out
}
