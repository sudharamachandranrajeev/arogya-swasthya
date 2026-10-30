// Body photo series: photos of one part of the body over time, kept as
// encrypted documents. Checks before saving help the next photo match the
// last one; they are hints, never a verdict on what the photo shows.

import { getPath, type Resource } from './fhir.ts'

export const BODY_PHOTO = 'Body photo'
/** R4 DocumentReference has no bodySite; this is the standard R5 cross-version extension. */
export const SITE_EXT = 'http://hl7.org/fhir/5.0/StructureDefinition/extension-DocumentReference.bodySite'

export const isBodyPhoto = (r: Resource) => getPath(r, 'type.text') === BODY_PHOTO

export function siteOf(r: Resource): string {
  const ext = (r.extension as { url?: string; valueCodeableConcept?: { text?: string } }[] | undefined)?.find((e) => e.url === SITE_EXT)
  return ext?.valueCodeableConcept?.text ?? ''
}

export const takenOf = (r: Resource) => String(getPath(r, 'context.period.start') ?? getPath(r, 'date') ?? '')

/** A new body photo record, before the file is attached. */
export function photoDoc(person: string, site: string, note: string, taken: Date): Resource {
  return {
    resourceType: 'DocumentReference',
    status: 'current',
    subject: { reference: `Patient/${person}` },
    type: { text: BODY_PHOTO },
    extension: [{ url: SITE_EXT, valueCodeableConcept: { text: site.trim() } }],
    description: note.trim() || `${site.trim()} photo`,
    context: { period: { start: taken.toISOString() } },
  }
}

/** Photos grouped by part of the body, oldest first, series with the newest photo first. */
export function series(rows: Resource[]): { site: string; photos: Resource[] }[] {
  const by = new Map<string, Resource[]>()
  for (const r of rows.filter(isBodyPhoto)) {
    const site = siteOf(r) || 'Other'
    const key = site.toLowerCase()
    if (!by.has(key)) by.set(key, [])
    by.get(key)!.push(r)
  }
  return [...by.values()]
    .map((photos) => ({ site: siteOf(photos[0]) || 'Other', photos: photos.sort((a, b) => takenOf(a).localeCompare(takenOf(b))) }))
    .sort((a, b) => takenOf(b.photos[b.photos.length - 1]).localeCompare(takenOf(a.photos[a.photos.length - 1])))
}

// ---- checks ----

export interface Pixels {
  data: Uint8ClampedArray | number[]
  width: number
  height: number
}

export interface Measure {
  /** Variance of a Laplacian over the grey image: higher is sharper. */
  sharpness: number
  /** Mean brightness, 0 to 255. */
  mean: number
  /** Shares of nearly black and nearly white pixels. */
  dark: number
  bright: number
  /** Width over height. */
  aspect: number
}

/** Measure an RGBA image (best a small copy, about 256 px across). */
export function measure(p: Pixels): Measure {
  const { width: w, height: h, data } = p
  const grey = new Float32Array(w * h)
  let sum = 0
  let dark = 0
  let bright = 0
  for (let i = 0; i < w * h; i++) {
    const y = 0.299 * data[i * 4] + 0.587 * data[i * 4 + 1] + 0.114 * data[i * 4 + 2]
    grey[i] = y
    sum += y
    if (y <= 8) dark++
    if (y >= 247) bright++
  }
  let n = 0
  let m = 0
  let m2 = 0
  for (let y = 1; y < h - 1; y++) {
    for (let x = 1; x < w - 1; x++) {
      const i = y * w + x
      const lap = grey[i - 1] + grey[i + 1] + grey[i - w] + grey[i + w] - 4 * grey[i]
      n++
      const d = lap - m
      m += d / n
      m2 += d * (lap - m)
    }
  }
  return { sharpness: n > 1 ? m2 / (n - 1) : 0, mean: sum / (w * h), dark: dark / (w * h), bright: bright / (w * h), aspect: w / h }
}

/** Skin has little fine detail, so the bar is low; a photo about as sharp as the last one passes too. */
export const SHARP_ENOUGH = 25

export interface Hint {
  tone: 'ok' | 'warn'
  text: string
}

/** Plain hints for a new photo, compared with the last one in its series when there is one. */
export function hints(now: Measure, last?: Measure): Hint[] {
  const out: Hint[] = []
  if (now.sharpness < SHARP_ENOUGH && !(last && now.sharpness >= last.sharpness * 0.7)) out.push({ tone: 'warn', text: 'It looks blurred. Hold still, or rest the phone on something, and try again.' })
  if (now.mean < 55) out.push({ tone: 'warn', text: 'It is dark. Use more light, ideally daylight from a window.' })
  else if (now.mean > 215) out.push({ tone: 'warn', text: 'It is very bright. Move out of direct light.' })
  if (now.bright > 0.08) out.push({ tone: 'warn', text: 'Part of it is washed out by light. Turn a little away from the light or the flash.' })
  if (last) {
    if (now.mean - last.mean > 30) out.push({ tone: 'warn', text: 'It is lighter than last time. Use the same light as before so the photos compare.' })
    else if (last.mean - now.mean > 30) out.push({ tone: 'warn', text: 'It is darker than last time. Use the same light as before so the photos compare.' })
    if (now.aspect > 1 !== last.aspect > 1) out.push({ tone: 'warn', text: 'The phone is turned differently from last time.' })
  }
  if (!out.length) out.push({ tone: 'ok', text: last ? 'Sharp, and close to the last photo in light.' : 'Sharp and well lit.' })
  return out
}
