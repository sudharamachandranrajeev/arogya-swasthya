// The doctor report's data: chosen sections only, the period for results
// and charts, chart positions from 0 to 100.
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { writeFileSync } from 'node:fs'
import { allergy, condition, medication, result, toFhir } from '../src/lib/model/fhir.ts'
import { buildReport, SECTIONS } from '../src/lib/model/report.ts'
import { toObservation, vitals } from '../src/lib/model/vitals.ts'

const now = new Date('2026-10-30T12:00:00Z')
const person = { resourceType: 'Patient', id: 'p1', name: [{ text: 'Alex Demo' }], birthDate: '1980-04-02', gender: 'male' }
const bp = vitals.find((v) => v.key === 'bp')!
const weight = vitals.find((v) => v.key === 'weight')!
const records = [
  toFhir(condition, { name: 'Asthma', status: 'active', onset: '2015' }, 'p1'),
  toFhir(allergy, { name: 'Penicillin', reaction: 'Rash' }, 'p1'),
  toFhir(medication, { name: 'Metformin 500 mg', dose: 'One tablet twice a day', times: '08:00, 20:00', status: 'active', start: '2021-03-15' }, 'p1'),
  toFhir(result, { name: 'HbA1c', value: '52', unit: 'mmol/mol', low: '20', high: '41', date: '2026-06-01' }, 'p1'),
  toFhir(result, { name: 'HbA1c', value: '47', unit: 'mmol/mol', low: '20', high: '41', date: '2026-10-02' }, 'p1'),
  toFhir(result, { name: 'Ferritin', value: '30', unit: 'ug/L', date: '2023-01-01' }, 'p1'),
  ...[['2026-08-02', '138', '88', '73.2'], ['2026-09-05', '133', '84', '72.4'], ['2026-10-01', '126', '80', '71.9']].flatMap(([d, s, di, w]) => [
    toObservation(bp, { unit: 'mmHg', values: { systolic: s, diastolic: di } }, 'p1', d),
    toObservation(weight, { unit: 'kg', values: { value: w } }, 'p1', d),
  ]),
]

test('a report holds the chosen sections, with results and charts in the period', () => {
  const all = new Set(SECTIONS.map((s) => s.id))
  const d = buildReport({ person, records }, all, '1y', ' Headaches since August. ', now)
  assert.equal(d.person.name, 'Alex Demo')
  assert.equal(d.person.age, 46)
  assert.equal(d.person.sex, 'Male')
  assert.equal(d.note, 'Headaches since August.')
  assert.deepEqual(d.lists.map((l) => l.heading), ['Conditions', 'Allergies', 'Medicines', 'Vaccines', 'Procedures'])
  assert.match(d.lists[2].rows[0].sub, /at 08:00, 20:00/)
  assert.deepEqual(d.results.map((r) => [r.name, r.value, r.flag, r.range]), [
    ['HbA1c', '47 mmol/mol', 'above range', '20 to 41'],
    ['HbA1c', '52 mmol/mol', 'above range', '20 to 41'],
  ])
  assert.deepEqual(d.charts.map((c) => c.title), ['Blood pressure (mmHg)', 'Weight (kg)', 'HbA1c (mmol/mol)'])
  const w = d.charts[1]
  assert.deepEqual(w.points.map((p) => p[0]), [0, 56.7, 100])
  assert.ok(w.points.every(([x, y]) => x >= 0 && x <= 100 && y >= 0 && y <= 100))
  assert.equal(d.charts[0].paired, true)
  assert.equal(d.charts[0].last, '126/80')
  assert.ok(d.charts[2].band, 'the lab range is shaded')
  if (process.env.REPORT_DATA) writeFileSync(process.env.REPORT_DATA, JSON.stringify(d))
})

test('sections left out are left out', () => {
  const d = buildReport({ person, records }, new Set(['allergies']), 'all', '', now)
  assert.deepEqual(d.lists.map((l) => l.heading), ['Allergies'])
  assert.equal(d.results.length, 0)
  assert.equal(d.charts.length, 0)
  assert.equal(d.patterns.length, 0)
})
