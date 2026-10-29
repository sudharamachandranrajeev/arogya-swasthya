// What every screen shares: lock status, the people kept here and who is
// being looked at, the person's places, and the undo message.
import { api, message, type Status } from './api.ts'
import { emptyPlaces, type Places } from './model/places.ts'
import { patient, type Resource } from './model/fhir.ts'
import { TERMS_VERSION } from './model/terms.ts'

export interface Toast {
  text: string
  /** Offer "Undo" (the change was saved). */
  undo?: boolean
  error?: boolean
}

class Session {
  status = $state<Status | null>(null)
  /** The first-run wizard is finished and the current terms are accepted. */
  ready = $state(false)
  places = $state<Places>(emptyPlaces())
  people = $state<Resource[]>([])
  current = $state('')
  /** Bumped on every change, so lists reload. */
  version = $state(0)
  toast = $state<Toast | null>(null)
  #toastTimer: ReturnType<typeof setTimeout> | undefined

  person = $derived(this.people.find((p) => p.id === this.current))
  personName = $derived(this.person ? patient.summary(this.person).title : '')

  /** Load everything; call at start and after unlocking. */
  async load() {
    this.status = await api.status()
    if (!this.status.open || this.status.locked) return
    const [done, terms, places] = await Promise.all([
      api.getSetting<boolean>('setup.done'),
      api.getSetting<{ version: number }>('terms.accepted'),
      api.getSetting<Places>('places'),
    ])
    this.places = places ?? emptyPlaces()
    this.ready = !!done && terms?.version === TERMS_VERSION
    await this.loadPeople()
  }

  async loadPeople() {
    this.people = await api.people()
    const saved = await api.getSetting<string>('person.current')
    const id = [saved, this.current].find((x) => x && this.people.some((p) => p.id === x))
    this.current = id ?? this.people[0]?.id ?? ''
  }

  async choose(id: string) {
    this.current = id
    await api.putSetting('person.current', id)
    this.version++
  }

  async setPlaces(p: Places) {
    await api.putSetting('places', p)
    this.places = p
  }

  /** After any saved change: reload and say what happened, with Undo. */
  async changed(text: string) {
    await this.loadPeople()
    this.version++
    this.say({ text, undo: true })
  }

  async undo() {
    try {
      const what = await api.undo()
      await this.loadPeople()
      this.version++
      this.say({ text: what ? `Undone: ${what}` : 'Nothing to undo.' })
    } catch (e) {
      this.fail(e)
    }
  }

  say(t: Toast) {
    clearTimeout(this.#toastTimer)
    this.toast = t
    this.#toastTimer = setTimeout(() => (this.toast = null), t.undo ? 8000 : 5000)
  }

  fail(e: unknown) {
    this.say({ text: message(e), error: true })
  }
}

export const session = new Session()
