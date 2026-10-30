// Form <-> FHIR mapping, result ranges, places and terms. Synthetic data only.
//   node --test 'app/test/*.test.ts'
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { allergy, check, classify, condition, fromFhir, kindOf, kinds, medication, result, setPath, symptom, toFhir, vaccine, vital } from '../src/lib/model/fhir.ts'
import { checkReading, show, toObservation, toReading, vitalOf, vitals } from '../src/lib/model/vitals.ts'
import { addHome, country, emergencyLine, emptyPlaces, removeHome } from '../src/lib/model/places.ts'
import { terms } from '../src/lib/model/terms.ts'

test('a condition round-trips through the form and points at its person', () => {
  const r = toFhir(condition, { name: 'Asthma', status: 'active', onset: '2015', site: 'Lungs', note: '' }, 'p1')
  assert.equal(r.resourceType, 'Condition')
  assert.deepEqual(r.subject, { reference: 'Patient/p1' })
  assert.equal((r.clinicalStatus as any).coding[0].system, 'http://terminology.hl7.org/CodeSystem/condition-clinical')
  assert.equal(r.note, undefined)
  assert.deepEqual(fromFhir(condition, r), { name: 'Asthma', status: 'active', onset: '2015', site: 'Lungs', note: '' })
})

test('editing keeps fields the form does not show, and clears emptied ones', () => {
  const old = { resourceType: 'MedicationStatement', id: 'm1', subject: { reference: 'Patient/p1' }, status: 'active',
    medicationCodeableConcept: { text: 'Metformin 500 mg', coding: [{ system: 'x', code: '1' }] },
    dosage: [{ text: 'twice a day' }], informationSource: { display: 'GP letter' } }
  const r = toFhir(medication, { ...fromFhir(medication, old), dose: '' }, 'p1', old)
  assert.equal(r.id, 'm1')
  assert.deepEqual(r.informationSource, { display: 'GP letter' })
  assert.deepEqual((r.medicationCodeableConcept as any).coding, [{ system: 'x', code: '1' }])
  assert.equal(r.dosage, undefined)
  assert.equal(old.dosage[0].text, 'twice a day', 'the stored copy is not changed')
})

test('allergies and vaccines carry their fixed fields and patient reference', () => {
  const a = toFhir(allergy, { name: 'Penicillin', category: 'medication', criticality: 'high' }, 'p1')
  assert.deepEqual(a.patient, { reference: 'Patient/p1' })
  assert.deepEqual(a.category, ['medication'])
  assert.equal((a.clinicalStatus as any).coding[0].code, 'active')
  assert.equal(allergy.summary(a).sub, 'Serious')
  const v = toFhir(vaccine, { name: 'Influenza', date: '2025-10-02', dose: '1' }, 'p1')
  assert.equal(v.status, 'completed')
  assert.equal(vaccine.summary(v).sub, 'Dose 1')
})

test('results: numbers become quantities, words stay words, ranges take the unit', () => {
  const n = toFhir(result, { name: 'HbA1c', value: '52', unit: 'mmol/mol', low: '20', high: '41', date: '2025-06-01' }, 'p1')
  assert.deepEqual(n.valueQuantity, { value: 52, unit: 'mmol/mol' })
  assert.deepEqual((n.referenceRange as any)[0], { low: { value: 20, unit: 'mmol/mol' }, high: { value: 41, unit: 'mmol/mol' } })
  assert.equal(result.summary(n).sub, '52 mmol/mol · Above range')
  assert.equal(fromFhir(result, n).value, '52')
  const w = toFhir(result, { name: 'COVID-19 test', value: 'negative', date: '2025-06-01' }, 'p1', n)
  assert.equal(w.valueString, 'negative')
  assert.equal(w.valueQuantity, undefined)
})

test('classify needs a number and a bound', () => {
  assert.equal(classify('3', '4', '6'), 'low')
  assert.equal(classify('5', '4', '6'), 'in')
  assert.equal(classify('7', '', '6'), 'high')
  assert.equal(classify('5'), null)
  assert.equal(classify('positive', '1', '2'), null)
  assert.equal(classify('', '1', '2'), null)
})

test('check explains missing, malformed and future values', () => {
  assert.deepEqual(check(vaccine, { name: 'MMR', date: '2020-01-05' }), [])
  assert.match(check(vaccine, { name: '' }).join(' '), /Vaccine is needed.*Date is needed/)
  assert.match(check(vaccine, { name: 'MMR', date: '5/1/2020' }).join(), /2024-03-15/)
  assert.match(check(vaccine, { name: 'MMR', date: '2999' }).join(), /future/)
  assert.match(check(result, { name: 'x', value: '1', date: '2020', low: 'abc' }).join(), /number/)
})

test('every kind has a summary and a required first field', () => {
  // Vital signs have their own form (Vitals.svelte).
  for (const k of kinds.filter((k) => k !== vital)) {
    assert.ok(k.fields[0].required, k.type)
    assert.equal(typeof k.summary(toFhir(k, { name: 'x' }, 'p')).title, 'string')
  }
})

test('setPath removes containers it empties', () => {
  const r: Record<string, unknown> = { a: [{ b: 1 }], keep: 1 }
  setPath(r, 'a.0.b', undefined)
  assert.deepEqual(r, { keep: 1 })
})

test('places: several homes, travel, own numbers', () => {
  let p = addHome(addHome(emptyPlaces(), 'GB'), 'IN')
  assert.deepEqual(p.homes, ['GB', 'IN'])
  assert.equal(p.current, 'GB')
  assert.equal(emergencyLine(p), 'United Kingdom: emergency 999 · urgent advice NHS 111')
  p = { ...p, current: 'JP' }
  assert.match(emergencyLine(p), /Japan: emergency 119/)
  p = { ...p, current: 'LK' }
  assert.equal(country(p, 'LK'), undefined)
  assert.match(emergencyLine(p), /add your local emergency number/)
  p = { ...p, custom: { LK: { name: 'Sri Lanka', emergency: '1990' } } }
  assert.equal(emergencyLine(p), 'Sri Lanka: emergency 1990')
  p = removeHome({ ...p, current: 'GB' }, 'GB')
  assert.deepEqual([p.homes, p.current], [['IN'], 'IN'])
})

test('terms name the emergency number of where the person is', () => {
  const t = terms(addHome(emptyPlaces(), 'US'))
  assert.ok(t.points.some((x) => x.includes('emergency 911')))
  assert.ok(t.points.some((x) => /not a doctor/.test(x)))
})

const v = (key: string) => vitals.find((x) => x.key === key)!

test('blood pressure is one Observation with two LOINC-coded parts in UCUM', () => {
  const o = toObservation(v('bp'), { unit: 'mmHg', values: { systolic: '128', diastolic: '82' } }, 'p1', '2026-10-01')
  assert.equal((o.code as any).coding[0].code, '85354-9')
  assert.equal((o.category as any)[0].coding[0].code, 'vital-signs')
  assert.deepEqual((o.component as any)[0].valueQuantity, { value: 128, unit: 'mmHg', system: 'http://unitsofmeasure.org', code: 'mm[Hg]' })
  assert.equal((o.component as any)[1].code.coding[0].code, '8462-4')
  assert.equal(o.valueQuantity, undefined)
  assert.equal(show(o), '128/82 mmHg')
  assert.equal(kindOf(o), vital)
  assert.deepEqual(toReading(v('bp'), o), { unit: 'mmHg', values: { systolic: '128', diastolic: '82' } })
})

test('the unit chooses the code where it matters, and edits keep other fields', () => {
  const o = toObservation(v('glucose'), { unit: 'mmol/L', values: { value: '5.4' } }, 'p1', '2026-10-01')
  assert.equal((o.code as any).coding[0].code, '15074-8')
  assert.equal(vitalOf(o), v('glucose'))
  const old = { ...o, id: 'g1', note: [{ text: 'after breakfast' }] }
  const e = toObservation(v('glucose'), { unit: 'mg/dL', values: { value: '97' } }, 'p1', '2026-10-02', old)
  assert.equal(e.id, 'g1')
  assert.deepEqual(e.note, [{ text: 'after breakfast' }])
  assert.equal((e.code as any).coding[0].code, '2339-0')
  assert.equal(show(e), '97 mg/dL')
  const spo2 = toObservation(v('spo2'), { unit: '%', values: { value: '97' } }, 'p1', '2026-10-01')
  assert.deepEqual((spo2.code as any).coding.map((c: any) => c.code), ['2708-6', '59408-5'])
})

test('readings refuse typing slips and half-filled blood pressure', () => {
  assert.deepEqual(checkReading(v('weight'), { unit: 'kg', values: {} }), [])
  assert.match(checkReading(v('weight'), { unit: 'kg', values: { value: '7150' } }).join(), /typing slip/)
  assert.deepEqual(checkReading(v('weight'), { unit: 'lb', values: { value: '160' } }), [])
  assert.match(checkReading(v('bp'), { unit: 'mmHg', values: { systolic: '120' } }).join(), /both/)
  assert.match(checkReading(v('bp'), { unit: 'mmHg', values: { systolic: '80', diastolic: '120' } }).join(), /top number/)
  assert.match(checkReading(v('temperature'), { unit: '°C', values: { value: 'warm' } }).join(), /number/)
})

test('symptoms are 0 to 10, and Observations sort into their kinds', () => {
  const s = toFhir(symptom, { name: 'Headache', severity: '6', date: '2026-10-01' }, 'p1')
  assert.equal(s.valueInteger, 6)
  assert.equal(kindOf(s), symptom)
  assert.equal(symptom.summary(s).sub, '6 out of 10')
  assert.match(check(symptom, { name: 'x', severity: '11', date: '2026-10-01' }).join(), /from 0 to 10/)
  assert.match(check(symptom, { name: 'x', severity: '2.5', date: '2026-10-01' }).join(), /whole number/)
  const lab = toFhir(result, { name: 'Ferritin', value: '45', date: '2026-10-01' }, 'p1')
  assert.equal(kindOf(lab), result)
  assert.equal(kindOf({ resourceType: 'Observation' }), result, 'an imported Observation with no category is a result')
})

test('trends group readings into series, keep the newest unit and the newest lab range', async () => {
  const { series, ticks, within } = await import('../src/lib/model/trends.ts')
  const bp = (d: string, a: string, b: string) => toObservation(v('bp'), { unit: 'mmHg', values: { systolic: a, diastolic: b } }, 'p1', d)
  const kg = (d: string, n: string, unit = 'kg') => toObservation(v('weight'), { unit, values: { value: n } }, 'p1', d)
  const lab = (d: string, n: string, high: string) => toFhir(result, { name: 'HbA1c', value: n, unit: 'mmol/mol', low: '20', high, date: d }, 'p1')
  const sym = (d: string, n: string) => toFhir(symptom, { name: 'Headache', severity: n, date: d }, 'p1')
  const all = series([bp('2026-01-02', '130', '85'), bp('2026-03-02', '124', '80'), kg('2026-01-01', '72'), kg('2026-02-01', '160', 'lb'), kg('2026-03-01', '71'),
    lab('2025-06', '52', '41'), lab('2026-01-15', '45', '42'), sym('2026-02-01', '4'), sym('2026-02-03', '2'), { resourceType: 'Observation', valueString: 'negative', code: { text: 'Covid test' } }])
  assert.deepEqual(all.map((s) => s.id), ['vital:bp', 'vital:weight', 'result:hba1c', 'symptom:headache'])
  const [b, w, h, s] = all
  assert.ok(b.paired)
  assert.deepEqual(b.points.map((p) => [p.v, p.lo]), [[130, 85], [124, 80]])
  assert.equal(w.unit, 'kg')
  assert.equal(w.otherUnits, 1)
  assert.deepEqual(h.range, { low: 20, high: 42 })
  assert.equal(h.points[0].date, '2025-06')
  assert.equal(h.points[1].band, 'high')
  assert.deepEqual(s.domain, [0, 10])
  assert.equal(within(b, '3m', Date.parse('2026-03-10')).length, 2)
  assert.equal(within(h, '3m', Date.parse('2026-03-10')).length, 1)
  assert.deepEqual(ticks(0, 10, 5), [0, 2, 4, 6, 8, 10])
  assert.deepEqual(ticks(71, 72.4), [71, 71.5, 72, 72.5])
})
