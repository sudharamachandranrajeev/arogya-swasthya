// Where records belong on the body. A record's own "Part of the body" wins;
// a condition without one is placed by a short table of names, and says so.
// Nothing is placed by guesswork beyond these tables.

import anatomy from './anatomy.json' with { type: 'json' }
import { condition, getPath, kindOf, procedure, type Resource } from './fhir.ts'

export type Tone = 'danger' | 'warn' | 'ok' | 'accent'
export type Kind = 'skin' | 'bone' | 'organ'

/** Every region of the 3D model, from the same table the model is built from. */
export const REGIONS: Record<string, { label: string; kind: Kind }> = anatomy as Record<string, { fma: string; kind: Kind; label: string }>

/** Parts the model has no mesh for, shown as a marker: [anchor region, x, y, z] as fractions
 *  of the anchor's box (x towards the body's left, y up, z towards the front). */
export const MARKERS: Record<string, { label: string; at: [string, number, number, number] }> = {
  thyroid: { label: 'Thyroid', at: ['trachea', 0.5, 0.8, 1.2] },
  spleen: { label: 'Spleen', at: ['stomach', 1.15, 0.55, 0.15] },
  'breast-r': { label: 'Right breast', at: ['rib-cage', 0.27, 0.72, 1.05] },
  'breast-l': { label: 'Left breast', at: ['rib-cage', 0.73, 0.72, 1.05] },
  uterus: { label: 'Womb', at: ['bladder', 0.5, 1.6, 0.3] },
  'ovary-r': { label: 'Right ovary', at: ['hip-bone-r', 0.75, 0.75, 0.5] },
  'ovary-l': { label: 'Left ovary', at: ['hip-bone-l', 0.25, 0.75, 0.5] },
}

export const labelOf = (id: string) => REGIONS[id]?.label ?? MARKERS[id]?.label ?? id

// Words in "Part of the body" (the pick list and common synonyms). A side word
// picks the -l or -r region; without one, both sides light.
const SITES: [RegExp, string[]][] = [
  [/\bhead\b/, ['skull', 'brain']],
  [/\bbrain\b/, ['brain']],
  [/\bskull\b/, ['skull']],
  [/\b(jaw|teeth|tooth)\b/, ['mandible']],
  [/\beyes?\b/, ['eye-*']],
  [/\b(mouth|tongue)\b/, ['tongue']],
  [/\bthroat\b/, ['trachea', 'esophagus']],
  [/\bneck\b/, ['spine-cervical']],
  [/\bthyroid\b/, ['thyroid']],
  [/\b(chest|ribs?|breastbone|sternum)\b/, ['rib-cage']],
  [/\bbreasts?\b/, ['breast-*']],
  [/\bheart\b/, ['heart']],
  [/\blungs?\b/, ['lung-*']],
  [/\b(abdomen|belly|tummy)\b/, ['stomach', 'small-intestine', 'large-intestine']],
  [/\bstomach\b/, ['stomach']],
  [/\b(bowel|intestines?|gut|colon)\b/, ['small-intestine', 'large-intestine']],
  [/\brectum\b/, ['rectum']],
  [/\bliver\b/, ['liver']],
  [/\bgall ?bladder\b/, ['gallbladder']],
  [/\bpancreas\b/, ['pancreas']],
  [/\bspleen\b/, ['spleen']],
  [/\bkidneys?\b/, ['kidney-*']],
  [/\bbladder\b/, ['bladder']],
  [/\bprostate\b/, ['prostate']],
  [/\b(womb|uterus)\b/, ['uterus']],
  [/\bovar(y|ies)\b/, ['ovary-*']],
  [/\bupper back\b/, ['spine-thoracic']],
  [/\blower back\b/, ['spine-lumbar', 'sacrum']],
  [/^back$|\bspine\b/, ['spine-cervical', 'spine-thoracic', 'spine-lumbar']],
  [/\bshoulders?\b/, ['scapula-*', 'clavicle-*']],
  [/\bcollar ?bones?\b/, ['clavicle-*']],
  [/\b(upper arm|arms?)\b/, ['humerus-*', 'radius-*', 'ulna-*']],
  [/\belbows?\b/, ['ulna-*']],
  [/\bforearms?\b/, ['radius-*', 'ulna-*']],
  [/\b(hands?|wrists?|fingers?|thumbs?)\b/, ['hand-*']],
  [/\b(hips?|pelvis)\b/, ['hip-bone-*']],
  [/\b(thighs?)\b/, ['femur-*']],
  [/\bknees?\b/, ['patella-*']],
  [/\blegs?\b/, ['femur-*', 'tibia-*', 'fibula-*']],
  [/\b(shins?|calf|calves)\b/, ['tibia-*', 'fibula-*']],
  [/\b(feet|foot|ankles?|toes?|heels?)\b/, ['foot-*']],
  [/\bskin\b/, ['skin']],
]

// Condition names that belong to one place, for conditions without a site.
const NAMES: [RegExp, string[]][] = [
  [/\b(asthma|copd|pneumonia|bronchitis|emphysema|tuberculosis)\b/, ['lung-*']],
  [/\bdiabetes\b/, ['pancreas']],
  [/\b(hypertension|high blood pressure|heart|angina|atrial fibrillation|coronary|arrhythmia)\b/, ['heart']],
  [/\b(migraine|epilepsy|stroke|dementia|alzheimer|parkinson|multiple sclerosis)\b/, ['brain']],
  [/\b(kidney|renal|ckd)\b/, ['kidney-*']],
  [/\b(eczema|psoriasis|dermatitis|acne|rosacea|melanoma|urticaria)\b/, ['skin']],
  [/\b(reflux|gerd|gord|heartburn)\b/, ['esophagus', 'stomach']],
  [/\b(gastritis|stomach ulcer|peptic ulcer)\b/, ['stomach']],
  [/\b(ibs|irritable bowel|crohn|colitis|coeliac|celiac|diverticul)\w*/, ['small-intestine', 'large-intestine']],
  [/\b(hepatitis|fatty liver|cirrhosis)\b/, ['liver']],
  [/\bgallstones?\b/, ['gallbladder']],
  [/\b(hypothyroid|hyperthyroid|thyroid)\w*/, ['thyroid']],
  [/\b(glaucoma|cataracts?|macular)\b/, ['eye-*']],
  [/\b(cystitis|urinary tract infection|uti)\b/, ['bladder']],
  [/\b(back pain|sciatica)\b/, ['spine-lumbar']],
  [/\b(endometriosis|fibroids?)\b/, ['uterus']],
  [/\b(polycystic ovar)\w*/, ['ovary-*']],
]

function expand(ids: string[], text: string): string[] {
  const left = /\bleft\b/.test(text)
  const right = /\bright\b/.test(text)
  return ids.flatMap((id) => {
    if (!id.endsWith('-*')) return [id]
    const base = id.slice(0, -1)
    if (left && !right) return [`${base}l`]
    if (right && !left) return [`${base}r`]
    return [`${base}r`, `${base}l`]
  })
}

function match(table: [RegExp, string[]][], raw: string): string[] {
  const text = raw.toLowerCase().trim()
  const out = new Set<string>()
  for (const [re, ids] of table) if (re.test(text)) for (const id of expand(ids, text)) out.add(id)
  return [...out]
}

/** Regions for a "Part of the body" text. */
export const regionsOfSite = (site: string) => match(SITES, site)

export interface Placed {
  record: Resource
  regions: string[]
  tone: Tone
  /** Placed from the condition's name, not a part of the body the person chose. */
  byName: boolean
}

const str = (r: Resource, path: string) => {
  const v = getPath(r, path)
  return typeof v === 'string' ? v : ''
}

function toneOf(r: Resource, asOf?: string): Tone {
  if (r.resourceType === 'Procedure') return 'accent'
  const ended = str(r, 'abatementDateTime')
  if (asOf && ended && ended.slice(0, asOf.length) > asOf) return 'danger'
  const status = str(r, 'clinicalStatus.coding.0.code')
  if (status === 'active') return 'danger'
  if (status === 'recurrence' || status === 'relapse') return 'warn'
  return 'ok'
}

const startOf = (r: Resource) => str(r, 'onsetDateTime') || str(r, 'performedDateTime') || str(r, 'onsetPeriod.start') || str(r, 'performedPeriod.start')

/** Conditions and procedures placed on the body. `asOf` ("2019" or a full date)
 *  leaves out what started later. */
export function place(records: Resource[], asOf?: string): Placed[] {
  const out: Placed[] = []
  for (const r of records) {
    const k = kindOf(r)
    if (k !== condition && k !== procedure) continue
    const start = startOf(r)
    if (asOf && start && start.slice(0, asOf.length) > asOf) continue
    const site = str(r, 'bodySite.0.text')
    let regions = site ? regionsOfSite(site) : []
    let byName = false
    if (!regions.length && k === condition) {
      regions = match(NAMES, `${str(r, 'code.text')} ${site}`)
      byName = regions.length > 0
    }
    if (regions.length) out.push({ record: r, regions, tone: toneOf(r, asOf), byName })
  }
  return out
}

const RANK: Record<Tone, number> = { danger: 3, warn: 2, accent: 1, ok: 0 }

/** Each lit region with its strongest tone and its records. */
export function byRegion(placed: Placed[]): Map<string, { tone: Tone; items: Placed[] }> {
  const out = new Map<string, { tone: Tone; items: Placed[] }>()
  for (const p of placed) {
    for (const id of p.regions) {
      const cur = out.get(id)
      if (!cur) out.set(id, { tone: p.tone, items: [p] })
      else {
        cur.items.push(p)
        if (RANK[p.tone] > RANK[cur.tone]) cur.tone = p.tone
      }
    }
  }
  return out
}

/** The first year anything placed started, for the year slider. */
export function firstYear(records: Resource[]): number | null {
  const years = records.map(startOf).filter(Boolean).map((d) => Number(d.slice(0, 4))).filter((y) => y > 1900)
  return years.length ? Math.min(...years) : null
}
