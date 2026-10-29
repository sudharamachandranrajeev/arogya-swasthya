// Countries and their emergency numbers. The person picks the countries they
// live in and switches "where I am now" when travelling; safety notes use the
// current one. Numbers here are the main public ones; each can be changed in
// Settings, and a country not listed asks for its number.

export interface Country {
  code: string
  name: string
  /** Ambulance or general emergency number. */
  emergency: string
  /** Urgent but not emergency medical advice, where one national number exists. */
  urgent?: string
}

const eu112 = (code: string, name: string): Country => ({ code, name, emergency: '112' })

export const COUNTRIES: Country[] = [
  { code: 'AU', name: 'Australia', emergency: '000' },
  eu112('AT', 'Austria'),
  eu112('BE', 'Belgium'),
  { code: 'CA', name: 'Canada', emergency: '911' },
  eu112('DK', 'Denmark'),
  eu112('FI', 'Finland'),
  eu112('FR', 'France'),
  eu112('DE', 'Germany'),
  eu112('GR', 'Greece'),
  { code: 'IN', name: 'India', emergency: '112 (ambulance 108)' },
  { code: 'IE', name: 'Ireland', emergency: '112 or 999' },
  eu112('IT', 'Italy'),
  { code: 'JP', name: 'Japan', emergency: '119' },
  eu112('NL', 'Netherlands'),
  { code: 'NZ', name: 'New Zealand', emergency: '111' },
  eu112('PL', 'Poland'),
  eu112('PT', 'Portugal'),
  { code: 'SG', name: 'Singapore', emergency: '995' },
  eu112('ES', 'Spain'),
  eu112('SE', 'Sweden'),
  { code: 'AE', name: 'United Arab Emirates', emergency: '998' },
  { code: 'GB', name: 'United Kingdom', emergency: '999', urgent: 'NHS 111' },
  { code: 'US', name: 'United States', emergency: '911', urgent: '988 (mental health crisis)' },
]

export interface Places {
  /** Countries the person lives in, in their order. */
  homes: string[]
  /** Where they are now: a home or a country they are visiting. */
  current: string
  /** Numbers the person typed: for countries not listed, or to correct ours. */
  custom: Record<string, { name?: string; emergency: string; urgent?: string }>
}

export const emptyPlaces = (): Places => ({ homes: [], current: '', custom: {} })

/** The country's details with the person's own numbers on top. Undefined when
 *  the country is unknown and they have not typed a number for it. */
export function country(places: Places, code: string): Country | undefined {
  const known = COUNTRIES.find((c) => c.code === code)
  const own = places.custom[code]
  if (!known && !own) return undefined
  return {
    code,
    name: own?.name || known?.name || code,
    emergency: own?.emergency || known!.emergency,
    urgent: own ? own.urgent : known?.urgent,
  }
}

/** One line for the footer and safety notes. */
export function emergencyLine(places: Places): string {
  const c = country(places, places.current)
  if (!c) return 'Emergency: add your local emergency number in Settings.'
  return `${c.name}: emergency ${c.emergency}${c.urgent ? ` · urgent advice ${c.urgent}` : ''}`
}

/** Add a home (keeping order) and make it current if none is. */
export function addHome(p: Places, code: string): Places {
  const homes = p.homes.includes(code) ? p.homes : [...p.homes, code]
  return { ...p, homes, current: p.current || code }
}

/** Remove a home; if it was current, fall back to the first remaining home. */
export function removeHome(p: Places, code: string): Places {
  const homes = p.homes.filter((h) => h !== code)
  return { ...p, homes, current: p.current === code ? (homes[0] ?? '') : p.current }
}
