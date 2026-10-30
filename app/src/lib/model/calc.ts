// Public health formulas, each with the band it falls in and where it comes
// from. No licensed scores. Inputs are filled from the newest records and can
// be changed before working anything out.

import { getPath, kindOf, result, vital, type Resource } from './fhir.ts'
import { toReading, vitalOf } from './vitals.ts'

export type Tone = 'ok' | 'warn' | 'danger' | 'accent'
export interface Band {
  label: string
  tone: Tone
}

const round = (n: number, places = 1) => Math.round(n * 10 ** places) / 10 ** places

// ---- body size ----

/** Body mass index, kg/m². */
export const bmi = (kg: number, cm: number) => kg / (cm / 100) ** 2

/** WHO adult bands; `lower` uses the lower cut-offs NICE advises for people
 *  of South Asian, Chinese, other Asian, Middle Eastern, Black African or
 *  African-Caribbean family background (23 and 27.5). */
export function bmiBand(b: number, lower = false): Band {
  const [over, obese] = lower ? [23, 27.5] : [25, 30]
  if (b < 18.5) return { label: 'Below the healthy range', tone: 'warn' }
  if (b < over) return { label: 'In the healthy range', tone: 'ok' }
  if (b < obese) return { label: 'Above the healthy range', tone: 'warn' }
  return { label: 'In the obesity range', tone: 'danger' }
}

export const whtr = (waistCm: number, heightCm: number) => waistCm / heightCm

/** NICE (2022): keep your waist to less than half your height. */
export function whtrBand(r: number): Band {
  if (r < 0.4) return { label: 'Lower than usual', tone: 'accent' }
  if (r < 0.5) return { label: 'No increased health risk', tone: 'ok' }
  if (r < 0.6) return { label: 'Increased health risk', tone: 'warn' }
  return { label: 'High health risk', tone: 'danger' }
}

// ---- kidneys ----

/** Creatinine µmol/L per mg/dL. */
export const CREATININE_UMOL = 88.4

/** eGFR by CKD-EPI 2021 (no race term), mL/min/1.73 m², adults. */
export function egfr(scrMgDl: number, age: number, sex: 'female' | 'male'): number {
  const k = sex === 'female' ? 0.7 : 0.9
  const a = sex === 'female' ? -0.241 : -0.302
  const s = scrMgDl / k
  return 142 * Math.min(s, 1) ** a * Math.max(s, 1) ** -1.2 * 0.9938 ** age * (sex === 'female' ? 1.012 : 1)
}

/** KDIGO GFR categories. */
export function egfrBand(g: number): Band & { stage: string } {
  if (g >= 90) return { stage: 'G1', label: 'Normal or high', tone: 'ok' }
  if (g >= 60) return { stage: 'G2', label: 'Mildly decreased', tone: 'ok' }
  if (g >= 45) return { stage: 'G3a', label: 'Mildly to moderately decreased', tone: 'warn' }
  if (g >= 30) return { stage: 'G3b', label: 'Moderately to severely decreased', tone: 'warn' }
  if (g >= 15) return { stage: 'G4', label: 'Severely decreased', tone: 'danger' }
  return { stage: 'G5', label: 'Kidney failure range', tone: 'danger' }
}

// ---- smoking and alcohol ----

/** Packs of 20 a day times years smoked. */
export const packYears = (perDay: number, years: number) => (perDay / 20) * years

/** One drink: UK units (10 mL of pure alcohol each) and grams of alcohol. */
export function drink(ml: number, abv: number) {
  const pure = (ml * abv) / 100
  return { units: pure / 10, grams: pure * 0.789 }
}

/** UK Chief Medical Officers: no more than 14 units a week. */
export const UNITS_A_WEEK = 14

// ---- inputs from records ----

export interface Known {
  kg?: number
  cm?: number
  waistCm?: number
  /** The newest creatinine result, in mg/dL. */
  creatinine?: { value: number; unit: 'mg/dL' | 'µmol/L'; mgdl: number; date: string; shown: string }
  /** When each was measured. */
  dates: Partial<Record<'kg' | 'cm' | 'waistCm', string>>
}

const TO_CM: Record<string, number> = { cm: 1, in: 2.54 }
const TO_KG: Record<string, number> = { kg: 1, lb: 0.45359237 }
const when = (r: Resource) => String(r.effectiveDateTime ?? (r.effectivePeriod as { start?: string } | undefined)?.start ?? '')

/** The newest weight, height, waist and creatinine among a person's
 *  Observations. */
export function known(rows: Resource[]): Known {
  const out: Known = { dates: {} }
  const sorted = [...rows].sort((a, b) => when(b).localeCompare(when(a)))
  for (const r of sorted) {
    const k = kindOf(r)
    if (k === vital) {
      const v = vitalOf(r)
      if (!v) continue
      const { unit, values } = toReading(v, r)
      const n = Number(values.value)
      if (!values.value || !Number.isFinite(n)) continue
      const field = ({ weight: 'kg', height: 'cm', waist: 'waistCm' } as const)[v.key as 'weight' | 'height' | 'waist']
      const factor = field === 'kg' ? TO_KG[unit] : TO_CM[unit]
      if (field && factor && out[field] === undefined) {
        out[field] = round(n * factor, 1)
        out.dates[field] = when(r)
      }
    } else if (k === result && !out.creatinine) {
      const name = String(getPath(r, 'code.text') ?? '')
      const q = r.valueQuantity as { value?: number; unit?: string } | undefined
      if (!/creatinine/i.test(name) || /urine|ratio|clearance|kinase/i.test(name) || typeof q?.value !== 'number') continue
      const unit = (q.unit ?? '').replace('μ', 'µ').toLowerCase()
      const mg = unit === 'mg/dl'
      if (!mg && !/^(µ|u|micro)mol\/l$/.test(unit)) continue
      out.creatinine = {
        value: q.value,
        unit: mg ? 'mg/dL' : 'µmol/L',
        mgdl: round(mg ? q.value : q.value / CREATININE_UMOL, 2),
        date: when(r),
        shown: `${q.value} ${q.unit}`,
      }
    }
  }
  return out
}

/** Whole years from a birth date ("1980", "1980-04" or "1980-04-02") to `on`. */
export function age(birth: string, on: Date = new Date()): number | undefined {
  const m = /^(\d{4})(?:-(\d{2})(?:-(\d{2}))?)?$/.exec(birth)
  if (!m) return undefined
  const [y, mo, d] = [Number(m[1]), Number(m[2] ?? 7), Number(m[3] ?? 1)]
  let a = on.getFullYear() - y
  if (on.getMonth() + 1 < mo || (on.getMonth() + 1 === mo && on.getDate() < d)) a--
  return a
}
