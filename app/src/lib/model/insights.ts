// Patterns worth a look, only between things that plausibly go together:
// weight and blood pressure, and a symptom before and after a medicine was
// started. Each says how many days it rests on and how big the change is,
// and says "not enough data yet" below 14 days. A pattern is not a cause.

import { getPath, type Resource } from './fhir.ts'
import type { Point, Series } from './trends.ts'

/** Days of paired data needed before a pattern is shown. */
export const MIN_DAYS = 14
/** Days looked at before and after a medicine starts. */
export const WINDOW_DAYS = 28
const DAY = 86_400_000

export interface Insight {
  id: string
  title: string
  /** What was found, or what is still needed. */
  text: string
  /** Days the pattern rests on. */
  n: number
  enough: boolean
  /** "small", "moderate" … when there is enough data. */
  size?: string
}

/** One value per day (the mean of that day's readings), by day number. */
function daily(points: Point[], pick: (p: Point) => number = (p) => p.v): Map<number, number> {
  const sums = new Map<number, [number, number]>()
  for (const p of points) {
    if (p.date.length < 10) continue // a month or year alone cannot be paired
    const d = Math.floor(p.t / DAY)
    const [s, n] = sums.get(d) ?? [0, 0]
    sums.set(d, [s + pick(p), n + 1])
  }
  return new Map([...sums].map(([d, [s, n]]) => [d, s / n]))
}

const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / xs.length

/** Pearson's correlation; NaN when either side does not vary. */
export function pearson(xs: number[], ys: number[]): number {
  const mx = mean(xs)
  const my = mean(ys)
  let sxy = 0
  let sxx = 0
  let syy = 0
  for (let i = 0; i < xs.length; i++) {
    sxy += (xs[i] - mx) * (ys[i] - my)
    sxx += (xs[i] - mx) ** 2
    syy += (ys[i] - my) ** 2
  }
  return sxy / Math.sqrt(sxx * syy)
}

/** Cohen's d with the pooled standard deviation; NaN when nothing varies. */
export function cohensD(a: number[], b: number[]): number {
  const v = (xs: number[]) => xs.reduce((s, x) => s + (x - mean(xs)) ** 2, 0) / (xs.length - 1)
  const pooled = Math.sqrt(((a.length - 1) * v(a) + (b.length - 1) * v(b)) / (a.length + b.length - 2))
  return (mean(b) - mean(a)) / pooled
}

/** Cohen's conventional words for |r|. */
export const rSize = (r: number) => (Math.abs(r) < 0.1 ? 'very weak' : Math.abs(r) < 0.3 ? 'weak' : Math.abs(r) < 0.5 ? 'moderate' : 'strong')
/** Cohen's conventional words for |d|. */
export const dSize = (d: number) => (Math.abs(d) < 0.2 ? 'negligible' : Math.abs(d) < 0.5 ? 'small' : Math.abs(d) < 0.8 ? 'medium' : 'large')

const fmt = (n: number, p = 1) => n.toLocaleString(undefined, { maximumFractionDigits: p, minimumFractionDigits: p })
const days = (n: number) => `${n} day${n === 1 ? '' : 's'}`

function weightAndPressure(weight: Series, bp: Series): Insight {
  const w = daily(weight.points)
  const sys = daily(bp.points)
  const both = [...w.keys()].filter((d) => sys.has(d))
  const id = 'weight-bp'
  const title = 'Weight and blood pressure'
  if (both.length < MIN_DAYS)
    return { id, title, n: both.length, enough: false, text: `Not enough data yet: ${days(both.length)} with both a weight and a blood pressure reading. Patterns show from ${MIN_DAYS}.` }
  const r = pearson(both.map((d) => w.get(d)!), both.map((d) => sys.get(d)!))
  if (!Number.isFinite(r))
    return { id, title, n: both.length, enough: true, text: `Over ${days(both.length)}, one of them did not change, so there is no pattern to see.` }
  const way = r > 0 ? 'higher weight went with a higher top number' : 'higher weight went with a lower top number'
  return {
    id, title, n: both.length, enough: true, size: rSize(r),
    text: `On ${days(both.length)} with both readings, ${way}: a ${rSize(r)} link (r = ${fmt(r, 2)}). Weight affects blood pressure for many people, but other things change both.`,
  }
}

function symptomAroundMedicine(med: Resource, s: Series): Insight | null {
  const start = String(getPath(med, 'effectivePeriod.start') ?? '')
  const name = String(getPath(med, 'medicationCodeableConcept.text') ?? '').trim()
  if (!/^\d{4}-\d{2}-\d{2}/.test(start) || !name) return null
  const day0 = Math.floor(Date.parse(start.slice(0, 10)) / DAY)
  const scores = daily(s.points)
  const before: number[] = []
  const after: number[] = []
  for (const [d, v] of scores) {
    if (d >= day0 - WINDOW_DAYS && d < day0) before.push(v)
    else if (d >= day0 && d < day0 + WINDOW_DAYS) after.push(v)
  }
  if (!before.length && !after.length) return null
  const id = `med:${med.id}:${s.id}`
  const title = `${s.title} around starting ${name}`
  const n = before.length + after.length
  const half = MIN_DAYS / 2
  if (before.length < half || after.length < half)
    return {
      id, title, n, enough: false,
      text: `Not enough data yet: ${s.title.toLowerCase()} was recorded on ${days(before.length)} in the ${WINDOW_DAYS} days before ${start.slice(0, 10)} and ${days(after.length)} after. Patterns show from ${half} on each side.`,
    }
  const d = cohensD(before, after)
  const change = mean(after) - mean(before)
  // Every score the same on each side: d cannot be worked out.
  const size = Number.isFinite(d) ? dSize(d) : change === 0 ? 'negligible' : 'large'
  return {
    id, title, n, enough: true, size,
    text:
      `${s.title} averaged ${fmt(mean(before))} out of 10 over ${days(before.length)} in the ${WINDOW_DAYS} days before ${start.slice(0, 10)}, ` +
      `and ${fmt(mean(after))} over ${days(after.length)} in the ${WINDOW_DAYS} days after: ${change === 0 ? 'no change' : `${change > 0 ? 'up' : 'down'} ${fmt(Math.abs(change))} points`}, ` +
      `a ${size} difference${Number.isFinite(d) ? ` (d = ${fmt(d, 2)})` : ''}. This shows when the change happened, not that the medicine caused it.`,
  }
}

/** Patterns for the plausible pairs that have any data, with enough data first. */
export function insights(all: Series[], meds: Resource[]): Insight[] {
  const out: Insight[] = []
  const weight = all.find((s) => s.id === 'vital:weight')
  const bp = all.find((s) => s.id === 'vital:bp')
  if (weight && bp) out.push(weightAndPressure(weight, bp))
  for (const m of meds)
    for (const s of all.filter((x) => x.group === 'symptom')) {
      const i = symptomAroundMedicine(m, s)
      if (i) out.push(i)
    }
  return out.sort((a, b) => Number(b.enough) - Number(a.enough) || b.n - a.n)
}
