// Placing records on the body: sites first, names only as a fallback.
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { byRegion, firstYear, MARKERS, place, REGIONS, regionsOfSite } from '../src/lib/model/body.ts'
import type { Resource } from '../src/lib/model/fhir.ts'

const cond = (name: string, status: string, extra: Record<string, unknown> = {}): Resource => ({
  resourceType: 'Condition',
  code: { text: name },
  clinicalStatus: { coding: [{ code: status }] },
  ...extra,
})

test('every pick-list part of the body lands somewhere real', () => {
  const sites = ['Head', 'Neck', 'Chest', 'Heart', 'Lungs', 'Abdomen', 'Lower back', 'Upper back', 'Left shoulder',
    'Right shoulder', 'Left arm', 'Right arm', 'Left hand', 'Right hand', 'Left hip', 'Right hip', 'Left knee',
    'Right knee', 'Left foot', 'Right foot', 'Skin']
  for (const s of sites) {
    const ids = regionsOfSite(s)
    assert.ok(ids.length > 0, s)
    for (const id of ids) assert.ok(REGIONS[id] || MARKERS[id], `${s} -> ${id}`)
  }
})

test('a side word picks one side; none lights both', () => {
  assert.deepEqual(regionsOfSite('Left knee'), ['patella-l'])
  assert.deepEqual(regionsOfSite('right foot'), ['foot-r'])
  assert.deepEqual(regionsOfSite('Knees'), ['patella-r', 'patella-l'])
  assert.deepEqual(regionsOfSite('Lower back'), ['spine-lumbar', 'sacrum'])
  assert.deepEqual(regionsOfSite('Somewhere odd'), [])
})

test('a site wins over the name, and a name alone is marked as such', () => {
  const [a, b, c] = place([
    cond('Asthma', 'active'),
    cond('Asthma', 'active', { bodySite: [{ text: 'Chest' }] }),
    cond('Mystery rash', 'active'),
  ])
  assert.deepEqual(a.regions, ['lung-r', 'lung-l'])
  assert.equal(a.byName, true)
  assert.deepEqual(b.regions, ['rib-cage'])
  assert.equal(b.byName, false)
  assert.equal(c, undefined)
})

test('tones follow status, and the strongest wins per region', () => {
  const placed = place([
    cond('Knee pain', 'resolved', { bodySite: [{ text: 'Left knee' }] }),
    cond('Knee arthritis', 'recurrence', { bodySite: [{ text: 'Left knee' }] }),
    { resourceType: 'Procedure', code: { text: 'Knee scope' }, bodySite: [{ text: 'Right knee' }], performedDateTime: '2020-03-01' },
  ])
  const map = byRegion(placed)
  assert.equal(map.get('patella-l')?.tone, 'warn')
  assert.equal(map.get('patella-l')?.items.length, 2)
  assert.equal(map.get('patella-r')?.tone, 'accent')
})

test('as of a year: later records are left out, ended ones were still active then', () => {
  const rs = [
    cond('Eczema', 'resolved', { onsetDateTime: '2015-06-01', abatementDateTime: '2019-02-01' }),
    cond('Migraine', 'active', { onsetDateTime: '2021-01-10' }),
  ]
  assert.equal(place(rs, '2014').length, 0)
  const then = place(rs, '2018')
  assert.equal(then.length, 1)
  assert.equal(then[0].tone, 'danger')
  assert.equal(place(rs, '2020')[0].tone, 'ok')
  assert.equal(place(rs).length, 2)
  assert.equal(firstYear(rs), 2015)
})

test('markers anchor to regions that exist in the model', () => {
  for (const [id, m] of Object.entries(MARKERS)) assert.ok(REGIONS[m.at[0]], id)
})
