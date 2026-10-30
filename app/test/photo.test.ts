// Body photos: series grouping and the capture checks, on made-up pixels.
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { hints, isBodyPhoto, measure, photoDoc, series, siteOf, SHARP_ENOUGH, type Pixels } from '../src/lib/model/photo.ts'

const image = (w: number, h: number, f: (x: number, y: number) => number): Pixels => {
  const data = new Uint8ClampedArray(w * h * 4)
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      const v = f(x, y)
      data.set([v, v, v, 255], (y * w + x) * 4)
    }
  return { data, width: w, height: h }
}

test('a sharp, even photo passes; a flat one reads as blurred', () => {
  const sharp = measure(image(64, 48, (x, y) => ((x >> 2) + (y >> 2)) % 2 ? 180 : 70))
  assert.ok(sharp.sharpness > SHARP_ENOUGH * 10)
  assert.deepEqual(hints(sharp).map((h) => h.tone), ['ok'])
  const flat = measure(image(64, 48, () => 128))
  assert.equal(flat.sharpness, 0)
  assert.match(hints(flat)[0].text, /blurred/)
})

test('dark, bright and washed-out photos say so', () => {
  const edges = (lo: number, hi: number) => image(64, 48, (x) => (x % 2 ? hi : lo))
  assert.ok(hints(measure(edges(5, 60))).some((h) => /dark/.test(h.text)))
  assert.ok(hints(measure(edges(200, 255))).some((h) => /bright/.test(h.text)))
  assert.ok(hints(measure(edges(100, 255))).some((h) => /washed out/.test(h.text)))
})

test('a photo lighter than the last, or turned, is pointed out', () => {
  const at = (lo: number, hi: number, w = 64, h = 48) => measure(image(w, h, (x) => (x % 2 ? hi : lo)))
  const last = at(60, 120)
  const now = hints(at(110, 170), last).map((h) => h.text)
  assert.ok(now.some((t) => /lighter than last time/.test(t)))
  assert.ok(hints(at(60, 120, 48, 64), last).some((h) => /turned differently/.test(h.text)))
  assert.deepEqual(hints(at(62, 122), last).map((h) => h.tone), ['ok'])
})

test('photos group by part of the body, oldest first in each series', () => {
  const p = (site: string, iso: string) => ({ ...photoDoc('me', site, '', new Date(iso)), id: iso })
  const rows = [p('Left forearm', '2026-03-01'), p('Back', '2026-01-01'), p('left forearm ', '2026-01-15'), { resourceType: 'DocumentReference', type: { text: 'Lab report' } }]
  assert.equal(isBodyPhoto(rows[0]), true)
  assert.equal(siteOf(rows[2]), 'left forearm')
  const s = series(rows)
  assert.deepEqual(s.map((x) => x.photos.length), [2, 1])
  assert.deepEqual(s[0].photos.map((r) => r.id), ['2026-01-15', '2026-03-01'])
})
