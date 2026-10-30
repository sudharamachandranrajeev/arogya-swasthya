// Patterns: only plausible pairs, with sample size and effect size, and
// nothing below 14 days.
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { cohensD, insights, pearson } from '../src/lib/model/insights.ts'
import type { Series } from '../src/lib/model/trends.ts'

const day = (i: number) => new Date(Date.UTC(2026, 5, 1 + i)).toISOString().slice(0, 10)
const pts = (vals: number[], from = 0) => vals.map((v, i) => ({ date: day(from + i), t: Date.parse(day(from + i)), v }))
const s = (id: string, title: string, group: Series['group'], points: Series['points']): Series => ({ id, title, group, unit: '', points, otherUnits: 0 })

test('pearson and Cohen d match hand-worked values', () => {
  assert.equal(pearson([1, 2, 3], [2, 4, 6]), 1)
  assert.equal(Math.round(pearson([1, 2, 3, 4], [1, 3, 2, 4]) * 100) / 100, 0.8)
  assert.equal(Math.round(cohensD([1, 2, 3], [3, 4, 5]) * 100) / 100, 2)
})

test('weight and blood pressure: not enough below 14 paired days', () => {
  const w = s('vital:weight', 'Weight', 'vital', pts([70, 71, 72]))
  const bp = s('vital:bp', 'Blood pressure', 'vital', pts([120, 122, 125]))
  const [i] = insights([w, bp], [])
  assert.equal(i.enough, false)
  assert.match(i.text, /3 days with both/)
})

test('weight and blood pressure: a strong link over 14 days', () => {
  const ws = Array.from({ length: 14 }, (_, i) => 70 + i * 0.2)
  const w = s('vital:weight', 'Weight', 'vital', pts(ws))
  const bp = s('vital:bp', 'Blood pressure', 'vital', pts(ws.map((x, i) => 50 + x + (i % 3))))
  const [i] = insights([w, bp], [])
  assert.equal(i.enough, true)
  assert.equal(i.n, 14)
  assert.equal(i.size, 'strong')
  assert.match(i.text, /higher weight went with a higher top number/)
})

test('a symptom before and after starting a medicine', () => {
  const head = s('symptom:headache', 'Headache', 'symptom', [...pts([6, 7, 6, 5, 7, 6, 7]), ...pts([3, 2, 3, 4, 2, 3, 3], 10)])
  const med = { resourceType: 'MedicationStatement', id: 'm1', medicationCodeableConcept: { text: 'Propranolol' }, effectivePeriod: { start: day(10) } }
  const [i] = insights([head], [med])
  assert.equal(i.enough, true)
  assert.equal(i.size, 'large')
  assert.match(i.text, /averaged 6.3 out of 10 over 7 days.*and 2.9 over 7 days.*down 3.4 points/)
  assert.match(i.text, /not that the medicine caused it/)
  // A medicine with no symptom data around it says nothing.
  const far = { ...med, id: 'm2', effectivePeriod: { start: '2020-01-01' } }
  assert.equal(insights([head], [far]).length, 0)
  // Too few days on one side.
  const few = s('symptom:headache', 'Headache', 'symptom', [...pts([6, 7]), ...pts([3, 2, 3, 4, 2, 3, 3], 10)])
  assert.equal(insights([few], [med])[0].enough, false)
})
