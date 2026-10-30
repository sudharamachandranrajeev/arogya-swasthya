// The doctor report's content: what the person chose to share, as plain
// data for the PDF template. Charts arrive as positions from 0 to 100 so the
// template only draws them.

import { allergy, classify, condition, kindOf, medication, patient, procedure, result, resultValue, vaccine, type Kind, type Resource } from './fhir.ts'
import { age } from './calc.ts'
import { insights } from './insights.ts'
import { series, ticks, within, type Point, type Series, type Span } from './trends.ts'

export type Section = 'conditions' | 'allergies' | 'medicines' | 'vaccines' | 'procedures' | 'results' | 'charts' | 'patterns'

export const SECTIONS: { id: Section; label: string }[] = [
  { id: 'conditions', label: 'Conditions' },
  { id: 'allergies', label: 'Allergies' },
  { id: 'medicines', label: 'Medicines' },
  { id: 'vaccines', label: 'Vaccines' },
  { id: 'procedures', label: 'Procedures and operations' },
  { id: 'results', label: 'Test results' },
  { id: 'charts', label: 'Charts of readings and symptoms' },
  { id: 'patterns', label: 'Patterns the app noticed' },
]

const LISTS: [Section, Kind][] = [
  ['conditions', condition],
  ['allergies', allergy],
  ['medicines', medication],
  ['vaccines', vaccine],
  ['procedures', procedure],
]

export interface Chart {
  title: string
  paired: boolean
  /** [x, y, y of the bottom number or null], 0 to 100, y from the top. */
  points: [number, number, number | null][]
  band: [number, number] | null
  y: [number, string][]
  x: [number, string][]
  last: string
}

export interface ReportData {
  title: string
  person: { name: string; born?: string; age?: number; sex?: string }
  made: string
  span: string
  note: string
  footer: string
  about: string
  lists: { heading: string; rows: { title: string; sub: string; date: string }[] }[]
  results: { name: string; value: string; flag: string; range: string; date: string }[]
  charts: Chart[]
  patterns: { title: string; text: string }[]
}

const SPAN_WORDS: Record<Span, string> = { '3m': 'The last 3 months', '1y': 'The last year', all: 'All records' }
const r1 = (n: number) => Math.round(n * 10) / 10
const dateOf = (r: Resource) => String(r.effectiveDateTime ?? (r.effectivePeriod as { start?: string } | undefined)?.start ?? '')
const day = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })

/** Positions for one series over the points in the period. */
export function chartOf(s: Series, points: Point[]): Chart {
  const values = points.flatMap((p) => (p.lo != null ? [p.v, p.lo] : [p.v]))
  const ys = s.domain
    ? ticks(s.domain[0], s.domain[1], 5)
    : ticks(Math.min(...values, s.range?.low ?? Infinity), Math.max(...values, s.range?.high ?? -Infinity))
  const [y0, y1] = [ys[0], ys[ys.length - 1]]
  const [t0, t1] = [points[0].t, points[points.length - 1].t]
  const fx = (t: number) => (t1 === t0 ? 50 : r1(((t - t0) / (t1 - t0)) * 100))
  const fy = (v: number) => r1((1 - (v - y0) / (y1 - y0)) * 100)
  const days = (t1 - t0) / 86_400_000
  const fmt = new Intl.DateTimeFormat('en-GB', days > 730 ? { year: 'numeric' } : days > 120 ? { month: 'short', year: 'numeric' } : { day: 'numeric', month: 'short' })
  const xs = (points.length > 2 ? [points[0], points[Math.floor(points.length / 2)], points[points.length - 1]] : points)
    .map((p): [number, string] => [fx(p.t), fmt.format(new Date(p.t))])
    .filter((x, i, all) => i === 0 || x[1] !== all[i - 1][1])
  const band: [number, number] | null =
    s.range && (s.range.low != null || s.range.high != null)
      ? [fy(Math.min(s.range.high ?? y1, y1)), fy(Math.max(s.range.low ?? y0, y0))]
      : null
  const last = points[points.length - 1]
  const unit = s.group === 'symptom' ? '0 to 10' : s.unit
  return {
    title: unit ? `${s.title} (${unit})` : s.title,
    paired: !!s.paired,
    points: points.map((p) => [fx(p.t), fy(p.v), p.lo != null ? fy(p.lo) : null]),
    band,
    y: ys.map((v) => [fy(v), String(v)]),
    x: xs,
    last: p2s(last),
  }
}
const p2s = (p: Point) => (p.lo != null ? `${p.v}/${p.lo}` : String(p.v))

export interface Gathered {
  person: Resource
  /** Every record of the person, any kind. */
  records: Resource[]
}

export function buildReport(g: Gathered, chosen: Set<Section>, span: Span, note: string, now = new Date()): ReportData {
  const name = patient.summary(g.person).title
  const born = typeof g.person.birthDate === 'string' ? g.person.birthDate : undefined
  const sexes: Record<string, string> = { female: 'Female', male: 'Male', other: 'Other' }
  const from = span === 'all' ? '' : new Date(now.getTime() - (span === '3m' ? 92 : 366) * 86_400_000).toISOString().slice(0, 10)
  const obs = g.records.filter((r) => r.resourceType === 'Observation')

  const lists = LISTS.filter(([id]) => chosen.has(id)).map(([, k]) => ({
    heading: k.title,
    rows: g.records
      .filter((r) => kindOf(r) === k)
      .map((r) => {
        const s = k.summary(r)
        return { title: s.title, sub: s.sub, date: s.date ?? '' }
      })
      .sort((a, b) => b.date.localeCompare(a.date)),
  }))
  const results = chosen.has('results')
    ? obs
        .filter((r) => kindOf(r) === result && dateOf(r) >= from)
        .sort((a, b) => dateOf(b).localeCompare(dateOf(a)))
        .map((r) => {
          const v = resultValue(r)
          const band = classify(v.value, v.low, v.high)
          return {
            name: String((r.code as { text?: string } | undefined)?.text ?? ''),
            value: [v.value, v.unit].filter(Boolean).join(' '),
            flag: band === 'low' ? 'below range' : band === 'high' ? 'above range' : '',
            range: v.low || v.high ? `${v.low || '…'} to ${v.high || '…'}` : '',
            date: dateOf(r).slice(0, 10),
          }
        })
    : []
  const all = series(obs)
  const charts = chosen.has('charts')
    ? all.map((s) => ({ s, points: within(s, span, now.getTime()) })).filter((x) => x.points.length >= 2).map((x) => chartOf(x.s, x.points))
    : []
  const meds = g.records.filter((r) => kindOf(r) === medication)
  const patterns = chosen.has('patterns') ? insights(all, meds).filter((i) => i.enough).map(({ title, text }) => ({ title, text })) : []

  return {
    title: 'Health summary',
    person: { name, born, age: born ? age(born, now) : undefined, sex: sexes[String(g.person.gender)] },
    made: day.format(now),
    span: span === 'all' ? SPAN_WORDS.all : `${SPAN_WORDS[span]} (${day.format(new Date(from))} to ${day.format(now)}), for results and charts`,
    note: note.trim(),
    footer: `${name} · ${day.format(now)}`,
    about:
      'Made with Arogya Swasthya from records the patient keeps themselves, typed in or imported. Please check anything important against the original documents.',
    lists,
    results,
    charts,
    patterns,
  }
}
