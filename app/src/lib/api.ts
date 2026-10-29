// The Rust shell's commands. Outside Tauri (the browser preview) a small
// in-memory stand-in answers instead, so every screen can be tried with
// made-up data that disappears on reload.
import { invoke } from '@tauri-apps/api/core'
import { clone, type Resource } from './model/fhir.ts'

export const inApp = '__TAURI_INTERNALS__' in globalThis

export type KeyStorage = 'keyring' | 'file' | 'androidKeystore'

export interface Status {
  open: boolean
  locked: boolean
  problem: string | null
  recoveryPending: boolean
  keyStorage: KeyStorage | null
  hasPin: boolean
  biometric: boolean
  waitSecs: number
  platform: string
}

export interface Trashed {
  resource: Resource
  deletedAt: string
}

export interface Backups {
  folder: string
  files: string[]
  userFolder: string | null
  userFiles: string[]
}

export type UiSetting = 'places' | 'terms.accepted' | 'setup.done' | 'person.current'

export interface Api {
  status(): Promise<Status>
  unlockPin(pin: string): Promise<boolean>
  unlockBiometric(): Promise<void>
  unlockWith(recovery: string): Promise<void>
  lock(): Promise<boolean>
  recoveryKey(): Promise<string>
  recoverySaved(): Promise<void>
  setPin(pin: string | null): Promise<void>
  setBiometric(on: boolean): Promise<void>
  backups(): Promise<Backups>
  setBackupFolder(folder: string | null): Promise<void>
  backupNow(): Promise<string>
  saveBackupTo(target: string): Promise<void>
  getSetting<T>(key: UiSetting): Promise<T | null>
  putSetting(key: UiSetting, value: unknown): Promise<void>
  people(): Promise<Resource[]>
  records(person: string, kind: string): Promise<Resource[]>
  timeline(person: string): Promise<Resource[]>
  save(resource: Resource): Promise<Resource>
  remove(id: string): Promise<void>
  restore(id: string): Promise<void>
  trash(): Promise<Trashed[]>
  undo(): Promise<string | null>
  exportBundle(person: string | null): Promise<Resource>
}

const tauri: Api = {
  status: () => invoke('status'),
  unlockPin: (pin) => invoke('unlock_pin', { pin }),
  unlockBiometric: () => invoke('unlock_biometric'),
  unlockWith: (recovery) => invoke('unlock_with', { recovery }),
  lock: () => invoke('lock'),
  recoveryKey: () => invoke('recovery_key'),
  recoverySaved: () => invoke('recovery_saved'),
  setPin: (pin) => invoke('set_pin', { pin }),
  setBiometric: (on) => invoke('set_biometric', { on }),
  backups: () => invoke('backups'),
  setBackupFolder: (folder) => invoke('set_backup_folder', { folder }),
  backupNow: () => invoke('backup_now'),
  saveBackupTo: (target) => invoke('save_backup_to', { target }),
  getSetting: (key) => invoke('get_setting', { key }),
  putSetting: (key, value) => invoke('put_setting', { key, value }),
  people: () => invoke('people'),
  records: (person, kind) => invoke('records', { person, kind }),
  timeline: (person) => invoke('timeline', { person }),
  save: (resource) => invoke('save_record', { resource }),
  remove: (id) => invoke('delete_record', { id }),
  restore: (id) => invoke('restore_record', { id }),
  trash: () => invoke('trash'),
  undo: () => invoke('undo'),
  exportBundle: (person) => invoke('export_bundle', { person }),
}

// ---- browser preview ----

const DATES: Record<string, string[]> = {
  Condition: ['onsetDateTime', 'recordedDate'],
  MedicationStatement: ['effectivePeriod.start'],
  AllergyIntolerance: ['onsetDateTime'],
  Procedure: ['performedDateTime'],
  Immunization: ['occurrenceDateTime'],
  Observation: ['effectiveDateTime'],
}
const dateOf = (r: Resource) =>
  (DATES[r.resourceType] ?? [])
    .map((p) => p.split('.').reduce<any>((v, k) => v?.[k], r))
    .find((v) => typeof v === 'string') as string | undefined
const personOf = (r: Resource) =>
  r.resourceType === 'Patient' ? r.id : ((r.subject ?? r.patient) as { reference?: string })?.reference?.slice(8)
const newest = (a: Resource, b: Resource) => (dateOf(b) ?? '').localeCompare(dateOf(a) ?? '')

function memory(): Api {
  type Row = { r: Resource; deletedAt?: string }
  let rows = new Map<string, Row>()
  const settings = new Map<string, unknown>()
  const history: { label: string; rows: Map<string, Row> }[] = []
  const recovery = Array.from(crypto.getRandomValues(new Uint8Array(32)), (b) => b.toString(16).padStart(2, '0'))
    .join('')
    .match(/.{4}/g)!
    .join('-')
  let pin: string | null = null
  const live = () => [...rows.values()].filter((x) => !x.deletedAt).map((x) => x.r)
  const change = (label: string, f: () => void) => {
    history.push({ label, rows: new Map(clone([...rows])) })
    f()
  }
  const name = (r: Resource) =>
    String((r as any).code?.text ?? (r as any).name?.[0]?.text ?? (r as any).medicationCodeableConcept?.text ?? (r as any).vaccineCode?.text ?? r.resourceType)
  return {
    status: async () => ({ open: true, locked: false, problem: null, recoveryPending: !settings.get('recovery.saved'), keyStorage: null, hasPin: !!pin, biometric: false, waitSecs: 0, platform: 'browser' }),
    unlockPin: async (p) => p === pin,
    unlockBiometric: async () => {},
    unlockWith: async () => {},
    lock: async () => false,
    recoveryKey: async () => recovery,
    recoverySaved: async () => void settings.set('recovery.saved', true),
    setPin: async (p) => void (pin = p),
    setBiometric: async () => { throw 'Fingerprint and face unlock are for phones.' },
    backups: async () => ({ folder: '(browser preview: no backups)', files: [], userFolder: null, userFiles: [] }),
    setBackupFolder: async () => { throw 'The browser preview cannot write backups.' },
    backupNow: async () => { throw 'The browser preview cannot write backups.' },
    saveBackupTo: async () => { throw 'The browser preview cannot write backups.' },
    getSetting: async <T,>(key: UiSetting) => (settings.get(key) as T) ?? null,
    putSetting: async (key, value) => void settings.set(key, clone(value)),
    people: async () => live().filter((r) => r.resourceType === 'Patient'),
    records: async (person, kind) => live().filter((r) => r.resourceType === kind && personOf(r) === person).sort(newest),
    timeline: async (person) => live().filter((r) => r.resourceType !== 'Patient' && personOf(r) === person).sort(newest),
    save: async (resource) => {
      const r = clone(resource)
      r.id ??= crypto.randomUUID()
      r.meta = { lastUpdated: new Date().toISOString() }
      const person = personOf(r)
      if (r.resourceType !== 'Patient' && !live().some((p) => p.resourceType === 'Patient' && p.id === person))
        throw `no person with id ${person}, or they are in the trash`
      change(`${rows.has(r.id!) ? 'edit' : 'add'} ${r.resourceType} “${name(r)}”`, () => rows.set(r.id!, { r }))
      return r
    },
    remove: async (id) => {
      const row = rows.get(id)
      if (!row || row.deletedAt) throw `no record with id ${id}`
      const at = new Date().toISOString()
      change(`delete ${row.r.resourceType} “${name(row.r)}”`, () => {
        for (const x of rows.values())
          if (x.r.id === id || (row.r.resourceType === 'Patient' && !x.deletedAt && personOf(x.r) === id)) x.deletedAt = at
      })
    },
    restore: async (id) => {
      const row = rows.get(id)
      if (!row?.deletedAt) throw `no record with id ${id}`
      const at = row.deletedAt
      change(`restore ${row.r.resourceType} “${name(row.r)}”`, () => {
        for (const x of rows.values())
          if (x.r.id === id || (row.r.resourceType === 'Patient' && x.deletedAt === at && personOf(x.r) === id)) delete x.deletedAt
      })
    },
    trash: async () =>
      [...rows.values()].filter((x) => x.deletedAt).map((x) => ({ resource: x.r, deletedAt: x.deletedAt! })),
    undo: async () => {
      const last = history.pop()
      if (!last) return null
      rows = last.rows
      return last.label
    },
    exportBundle: async (person) => ({
      resourceType: 'Bundle',
      type: 'collection',
      entry: live().filter((r) => !person || personOf(r) === person).map((resource) => ({ resource })),
    }),
  }
}

export const api: Api = inApp ? tauri : memory()

/** A command's error as plain text. */
export const message = (e: unknown) => (typeof e === 'string' ? e : e instanceof Error ? e.message : String(e))
