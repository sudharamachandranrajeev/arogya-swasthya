// Readings over time, grouped for charts: one series per vital sign, per
// numeric test (by name and unit) and per symptom.

import { classify, kindOf, result, symptom, vital, type Band, type Resource } from './fhir.ts'
import { toReading, vitalOf } from './vitals.ts'

export interface Point {
  /** As stored: a day, a month or a year. */
  date: string
  /** Milliseconds, for placing the point. */
  t: number
  /** The value; for blood pressure, the top number. */
  v: number
  /** Blood pressure: the bottom number. */
  lo?: number
  band?: Band
}

export interface Series {
  id: string
  title: string
  group: 'vital' | 'result' | 'symptom'
  unit: string
  points: Point[]
  /** The lab's normal range (results), shaded behind the line. */
  range?: { low?: number; high?: number }
  /** Fixed scale (symptoms: 0 to 10). */
  domain?: [number, number]
  /** Two values per reading (blood pressure). */
  paired?: boolean
  /** Readings left out because they use another unit. */
  otherUnits: number
}

const num = (x: unknown) => (typeof x === 'number' && Number.isFinite(x) ? x : undefined)
const at = (date: string) => Date.parse(date.length === 4 ? `${date}-07-01` : date.length === 7 ? `${date}-15` : date.slice(0, 10))
const dateOf = (r: Resource) => String(r.effectiveDateTime ?? (r.effectivePeriod as { start?: string } | undefined)?.start ?? '')

export function series(rows: Resource[]): Series[] {
  const map = new Map<string, Series & { byUnit: Map<string, Point[]> }>()
  const ranges = new Map<string, { t: number; low?: number; high?: number }>()
  const add = (id: string, base: Omit<Series, 'points' | 'unit' | 'otherUnits'>, unit: string, p: Point) => {
    let s = map.get(id)
    if (!s) map.set(id, (s = { ...base, unit: '', points: [], otherUnits: 0, byUnit: new Map() }))
    s.byUnit.set(unit, [...(s.byUnit.get(unit) ?? []), p])
  }
  for (const r of rows) {
    const date = dateOf(r)
    const t = at(date)
    if (!Number.isFinite(t)) continue
    const k = kindOf(r)
    if (k === vital) {
      const v = vitalOf(r)
      if (!v) continue
      const reading = toReading(v, r)
      const top = num(Number(reading.values[v.parts ? 'systolic' : 'value'] || NaN))
      if (top === undefined) continue
      const lo = v.parts ? num(Number(reading.values.diastolic || NaN)) : undefined
      if (v.parts && lo === undefined) continue
      add(`vital:${v.key}`, { id: `vital:${v.key}`, title: v.label, group: 'vital', paired: !!v.parts }, reading.unit, { date, t, v: top, lo })
    } else if (k === symptom) {
      const v = num(r.valueInteger)
      const name = String((r.code as { text?: string } | undefined)?.text ?? '').trim()
      if (v === undefined || !name) continue
      const id = `symptom:${name.toLowerCase()}`
      add(id, { id, title: name, group: 'symptom', domain: [0, 10] }, 'out of 10', { date, t, v })
    } else if (k === result) {
      const q = r.valueQuantity as { value?: unknown; unit?: string } | undefined
      const v = num(q?.value)
      const name = String((r.code as { text?: string } | undefined)?.text ?? '').trim()
      if (v === undefined || !name) continue
      const low = num((r.referenceRange as { low?: { value?: unknown } }[] | undefined)?.[0]?.low?.value)
      const high = num((r.referenceRange as { high?: { value?: unknown } }[] | undefined)?.[0]?.high?.value)
      const unit = q?.unit ?? ''
      const id = `result:${name.toLowerCase()}`
      add(id, { id, title: name, group: 'result' }, unit, { date, t, v, band: classify(String(v), low?.toString(), high?.toString()) })
      // The newest reading's range is the one shown.
      if ((low !== undefined || high !== undefined) && t >= (ranges.get(id)?.t ?? -Infinity)) ranges.set(id, { t, low, high })
    }
  }
  const out: Series[] = []
  for (const { byUnit, ...s } of map.values()) {
    // Keep the unit used most recently; count the others.
    let unit = ''
    let latest = -Infinity
    let total = 0
    for (const [u, ps] of byUnit) {
      total += ps.length
      const t = Math.max(...ps.map((p) => p.t))
      if (t > latest) [latest, unit] = [t, u]
    }
    const points = byUnit.get(unit)!.sort((a, b) => a.t - b.t)
    const r = ranges.get(s.id)
    if (r) s.range = { low: r.low, high: r.high }
    out.push({ ...s, unit, points, otherUnits: total - points.length })
  }
  const order = { vital: 0, result: 1, symptom: 2 }
  return out.sort((a, b) => order[a.group] - order[b.group] || a.title.localeCompare(b.title))
}

export type Span = '3m' | '1y' | 'all'

export function within(s: Series, span: Span, now = Date.now()): Point[] {
  if (span === 'all') return s.points
  const from = now - (span === '3m' ? 92 : 366) * 86_400_000
  return s.points.filter((p) => p.t >= from)
}

/** A clean scale: round tick values covering [min, max], about `count` of them. */
export function ticks(min: number, max: number, count = 4): number[] {
  if (min === max) [min, max] = [min - 1, max + 1]
  const raw = (max - min) / count
  const mag = 10 ** Math.floor(Math.log10(raw))
  const step = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => s >= raw)!
  const out: number[] = []
  for (let v = Math.floor(min / step) * step; v <= max + step * 1e-9; v += step) out.push(Number(v.toFixed(10)))
  if (out[out.length - 1] < max) out.push(Number((out[out.length - 1] + step).toFixed(10)))
  return out
}
