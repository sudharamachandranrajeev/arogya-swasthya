// Calculators: published formulas checked against worked examples.
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { age, bmi, bmiBand, drink, egfr, egfrBand, known, packYears, whtr, whtrBand } from '../src/lib/model/calc.ts'
import { result, toFhir } from '../src/lib/model/fhir.ts'
import { toObservation, vitals } from '../src/lib/model/vitals.ts'

test('BMI and its bands, with the lower cut-offs when chosen', () => {
  assert.equal(Math.round(bmi(70, 175) * 10) / 10, 22.9)
  assert.equal(bmiBand(22.9).label, 'In the healthy range')
  assert.equal(bmiBand(24).label, 'In the healthy range')
  assert.equal(bmiBand(24, true).label, 'Above the healthy range')
  assert.equal(bmiBand(28, true).label, 'In the obesity range')
  assert.equal(bmiBand(17).tone, 'warn')
})

test('waist to height bands', () => {
  assert.equal(whtr(85, 170), 0.5)
  assert.equal(whtrBand(0.49).label, 'No increased health risk')
  assert.equal(whtrBand(0.5).label, 'Increased health risk')
  assert.equal(whtrBand(0.6).tone, 'danger')
})

test('eGFR CKD-EPI 2021 matches worked examples', () => {
  assert.equal(Math.round(egfr(1.0, 60, 'male')), 86)
  assert.equal(Math.round(egfr(0.8, 50, 'female')), 90)
  assert.equal(Math.round(egfr(0.6, 40, 'female')), 116)
  assert.equal(egfrBand(86).stage, 'G2')
  assert.equal(egfrBand(44.9).stage, 'G3b')
  assert.equal(egfrBand(10).stage, 'G5')
})

test('pack-years and alcohol units', () => {
  assert.equal(packYears(10, 30), 15)
  const pint = drink(568, 4)
  assert.equal(Math.round(pint.units * 10) / 10, 2.3)
  assert.equal(Math.round(pint.grams * 10) / 10, 17.9)
})

test('age counts whole years, even from a year alone', () => {
  const on = new Date(2026, 9, 30)
  assert.equal(age('1980-04-02', on), 46)
  assert.equal(age('1980-11-02', on), 45)
  assert.equal(age('1980', on), 46)
  assert.equal(age('', on), undefined)
})

test('the newest weight, height, waist and creatinine fill the inputs', () => {
  const v = (key: string) => vitals.find((x) => x.key === key)!
  const rows = [
    toObservation(v('weight'), { unit: 'kg', values: { value: '80' } }, 'p1', '2026-01-01'),
    toObservation(v('weight'), { unit: 'lb', values: { value: '154' } }, 'p1', '2026-10-01'),
    toObservation(v('height'), { unit: 'in', values: { value: '69' } }, 'p1', '2025-05-01'),
    toFhir(result, { name: 'Creatinine', value: '88', unit: 'µmol/L', date: '2026-09-01' }, 'p1'),
    toFhir(result, { name: 'Urine creatinine', value: '9', unit: 'mmol/L', date: '2026-10-01' }, 'p1'),
  ]
  const k = known(rows)
  assert.equal(k.kg, 69.9)
  assert.equal(k.dates.kg, '2026-10-01')
  assert.equal(k.cm, 175.3)
  assert.equal(k.waistCm, undefined)
  assert.equal(k.creatinine?.mgdl, 1)
})
