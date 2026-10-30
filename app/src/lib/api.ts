// The Rust shell's commands. Outside Tauri (the browser preview) a small
// in-memory stand-in answers instead, so every screen can be tried with
// made-up data that disappears on reload.
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
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
  /** Something to tell the user once, such as a failed restore at start. */
  notice: string | null
}

export interface Trashed {
  resource: Resource
  deletedAt: string
}

export type BackupKind = 'daily' | 'manual' | 'preUpdate' | 'beforeRestore'

export interface Backup {
  name: string
  kind: BackupKind
  bytes: number
}

export interface Backups {
  folder: string
  files: Backup[]
  userFolder: string | null
  userFiles: string[]
}

/** What a backup holds, shown before restoring it. */
export interface BackupSummary {
  schema: number
  people: string[]
  records: number
  trashed: number
  lastChange: string | null
}

export type InstallKind = 'installer' | 'appImage' | 'pacman' | 'portable' | 'android' | 'other'
export type UpdateMode = 'ask' | 'off'

export interface UpdateStatus {
  kind: InstallKind
  canInstall: boolean
  current: string
  previous: string | null
  mode: UpdateMode
}

export interface UpdateInfo {
  version: string
  notes: string | null
  date: string | null
  canInstall: boolean
}

export interface RollbackPlan {
  version: string
  backup: string | null
  needsRestore: boolean
}

export interface Hit {
  resource: Resource
  snippet: string
}

export interface Candidate {
  resource: Resource
  /** A record already kept that looks the same (type, name and date). */
  duplicateOf: string | null
}

/** A file read for import, waiting for the user to pick what to keep. */
export interface Pending {
  person: string
  fileName: string
  candidates: Candidate[]
  /** Types in the file that are not kept here, with how many of each. */
  skipped: [string, number][]
}

/** What a SMART Health Link says, before anything is fetched. */
export interface LinkInfo {
  url: string
  /** Seconds since 1970 after which the link stops working. */
  exp: number | null
  label: string | null
  needsPasscode: boolean
  direct: boolean
}

type Choice = 'on' | 'off' | null

export interface TextStatus {
  /** Reading photos and scans: on, off, or not chosen yet. */
  ocr: Choice
  /** Transcribing recordings, the same way. */
  transcribe: Choice
  /** This device can transcribe (desktop, for now). */
  canTranscribe: boolean
  /** Documents whose words are not read yet. */
  waiting: number
}

export type Page = 'releases' | 'bug' | 'idea'

export interface ReminderSettings {
  on: boolean
  /** Show the medicine's name in reminders (seen on a locked screen). */
  names: boolean
  /** No second reminder between these times: ["22:00", "07:00"]. */
  quiet: [string, string] | null
}

export interface Dose {
  med: string
  person: string
  personName: string
  name: string
  /** "2026-10-30" */
  date: string
  /** "08:00" */
  time: string
  status: 'taken' | 'skipped' | null
  at: string | null
  /** When this medicine was last marked taken, on any day. */
  lastTaken: string | null
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
  pickBackup(): Promise<string | null>
  /** Ask for a file, encrypt it and save `doc` pointing at it. Null when cancelled. */
  addDocument(doc: Resource): Promise<Resource | null>
  documentBytes(id: string): Promise<ArrayBuffer>
  textStatus(): Promise<TextStatus>
  /** On downloads the reading models first (about 12 MB, once). */
  setOcr(on: boolean): Promise<void>
  /** Turn transcripts of recordings on (downloading the model) or off. */
  setTranscribe(on: boolean): Promise<void>
  saveDocumentTo(id: string, target: string): Promise<void>
  /** Ask for a FHIR JSON or C-CDA file and read it into a review list. Null when cancelled. */
  importPick(person: string): Promise<Pending | null>
  importPending(): Promise<Pending | null>
  /** Save the candidates at these places as one change. Returns how many. */
  importAccept(picks: number[]): Promise<number>
  importDiscard(): Promise<void>
  linkInfo(link: string): Promise<LinkInfo>
  /** Fetch what a SMART Health Link holds into a review list. */
  importLink(person: string, link: string, passcode: string | null): Promise<Pending | null>
  inspectBackup(name: string | null, recovery: string | null): Promise<BackupSummary>
  restoreBackup(name: string | null, recovery: string | null): Promise<string>
  updateStatus(): Promise<UpdateStatus>
  setUpdateMode(mode: UpdateMode): Promise<void>
  checkUpdate(): Promise<UpdateInfo | null>
  installUpdate(version: string): Promise<void>
  rollbackPlan(): Promise<RollbackPlan | null>
  rollback(restore: boolean): Promise<void>
  openPage(page: Page, fields?: Record<string, string>): Promise<void>
  getSetting<T>(key: UiSetting): Promise<T | null>
  putSetting(key: UiSetting, value: unknown): Promise<void>
  people(): Promise<Resource[]>
  records(person: string, kind: string): Promise<Resource[]>
  timeline(person: string): Promise<Resource[]>
  /** Best matches first; `snippet` marks each match between \u0002 and \u0003. */
  search(person: string | null, words: string): Promise<Hit[]>
  save(resource: Resource): Promise<Resource>
  /** Save several as one change, so one Undo takes them all back. */
  saveAll(resources: Resource[], label: string): Promise<Resource[]>
  remove(id: string): Promise<void>
  restore(id: string): Promise<void>
  trash(): Promise<Trashed[]>
  undo(): Promise<string | null>
  exportBundle(person: string | null): Promise<Resource>
  reminderSettings(): Promise<ReminderSettings>
  /** Asks for permission to notify when turning reminders on. */
  setReminders(settings: ReminderSettings): Promise<void>
  /** Today's doses for everyone, in time order. */
  dosesToday(): Promise<Dose[]>
  /** Mark a dose taken or skipped; null clears the mark. */
  markDose(dose: Dose, status: 'taken' | 'skipped' | null): Promise<void>
  /** Reschedule reminders after medicines change. */
  refreshReminders(): Promise<void>
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
  pickBackup: () => invoke('pick_backup'),
  addDocument: (doc) => invoke('add_document', { doc }),
  documentBytes: (id) => invoke('document_bytes', { id }),
  textStatus: () => invoke('text_status'),
  setOcr: (on) => invoke('set_ocr', { on }),
  setTranscribe: (on) => invoke('set_transcribe', { on }),
  saveDocumentTo: (id, target) => invoke('save_document_to', { id, target }),
  importPick: (person) => invoke('import_pick', { person }),
  importPending: () => invoke('import_pending'),
  importAccept: (picks) => invoke('import_accept', { picks }),
  importDiscard: () => invoke('import_discard'),
  linkInfo: (link) => invoke('link_info', { link }),
  importLink: (person, link, passcode) => invoke('import_link', { person, link, passcode }),
  inspectBackup: (name, recovery) => invoke('inspect_backup', { name, recovery }),
  restoreBackup: (name, recovery) => invoke('restore_backup', { name, recovery }),
  updateStatus: () => invoke('update_status'),
  setUpdateMode: (mode) => invoke('set_update_mode', { mode }),
  checkUpdate: () => invoke('check_update'),
  installUpdate: (version) => invoke('install_update', { version }),
  rollbackPlan: () => invoke('rollback_plan'),
  rollback: (restore) => invoke('rollback', { restore }),
  openPage: (page, fields = {}) => invoke('open_page', { page, fields }),
  getSetting: (key) => invoke('get_setting', { key }),
  putSetting: (key, value) => invoke('put_setting', { key, value }),
  people: () => invoke('people'),
  records: (person, kind) => invoke('records', { person, kind }),
  timeline: (person) => invoke('timeline', { person }),
  search: (person, words) => invoke('search', { person, words }),
  save: (resource) => invoke('save_record', { resource }),
  saveAll: (resources, label) => invoke('save_records', { resources, label }),
  reminderSettings: () => invoke('reminder_settings'),
  setReminders: (settings) => invoke('set_reminders', { settings }),
  dosesToday: () => invoke('doses_today'),
  markDose: (d, status) => invoke('mark_dose', { med: d.med, date: d.date, time: d.time, status }),
  refreshReminders: () => invoke('refresh_reminders'),
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
    status: async () => ({ open: true, locked: false, problem: null, recoveryPending: !settings.get('recovery.saved'), keyStorage: null, hasPin: !!pin, biometric: false, waitSecs: 0, platform: 'browser', notice: null }),
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
    pickBackup: async () => { throw 'The browser preview has no backups to restore.' },
    addDocument: async () => { throw 'The browser preview cannot keep files.' },
    documentBytes: async () => { throw 'The browser preview cannot keep files.' },
    textStatus: async () => ({
      ocr: (settings.get('ocr.mode') as Choice) ?? null,
      transcribe: (settings.get('transcribe.mode') as Choice) ?? null,
      canTranscribe: true,
      waiting: 0,
    }),
    setOcr: async (on) => void settings.set('ocr.mode', on ? 'on' : 'off'),
    setTranscribe: async (on) => void settings.set('transcribe.mode', on ? 'on' : 'off'),
    saveDocumentTo: async () => { throw 'The browser preview cannot keep files.' },
    // The preview has no file dialog: "choosing a file" gives a made-up sample.
    importPick: async (person) => {
      const subject = { reference: `Patient/${person}` }
      const pending: Pending = {
        person,
        fileName: 'sample-summary.json',
        candidates: [
          { resource: { resourceType: 'Condition', subject, code: { text: 'Sample condition' }, onsetDateTime: '2015-03-01' }, duplicateOf: null },
          { resource: { resourceType: 'Immunization', patient: subject, status: 'completed', vaccineCode: { text: 'Sample vaccine' }, occurrenceDateTime: '2025-10-04' }, duplicateOf: null },
        ],
        skipped: [['Practitioner', 1]],
      }
      settings.set('import.pending', pending)
      return clone(pending)
    },
    importPending: async () => clone((settings.get('import.pending') as Pending | undefined) ?? null),
    importAccept: async (picks) => {
      const p = settings.get('import.pending') as Pending | undefined
      settings.delete('import.pending')
      const chosen = (p?.candidates ?? []).filter((_, i) => picks.includes(i))
      if (!chosen.length) return 0
      change(`import of ${chosen.length} record${chosen.length === 1 ? '' : 's'}`, () => {
        for (const c of chosen) {
          const r = clone(c.resource)
          r.id = crypto.randomUUID()
          r.meta = { lastUpdated: new Date().toISOString() }
          rows.set(r.id, { r })
        }
      })
      return chosen.length
    },
    importDiscard: async () => void settings.delete('import.pending'),
    linkInfo: async () => { throw 'The browser preview cannot open links.' },
    importLink: async () => { throw 'The browser preview cannot open links.' },
    inspectBackup: async () => { throw 'The browser preview has no backups to restore.' },
    restoreBackup: async () => { throw 'The browser preview has no backups to restore.' },
    updateStatus: async () => ({ kind: 'other', canInstall: false, current: 'preview', previous: null, mode: (settings.get('updates.mode') as UpdateMode) ?? 'ask' }),
    setUpdateMode: async (mode) => void settings.set('updates.mode', mode),
    checkUpdate: async () => null,
    installUpdate: async () => { throw 'The browser preview cannot update itself.' },
    rollbackPlan: async () => null,
    rollback: async () => { throw 'The browser preview cannot update itself.' },
    openPage: async () => { throw 'The browser preview cannot open pages.' },
    getSetting: async <T,>(key: UiSetting) => (settings.get(key) as T) ?? null,
    putSetting: async (key, value) => void settings.set(key, clone(value)),
    people: async () => live().filter((r) => r.resourceType === 'Patient'),
    records: async (person, kind) => live().filter((r) => r.resourceType === kind && personOf(r) === person).sort(newest),
    search: async (person, words) => {
      const ws = words.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter(Boolean)
      if (!ws.length) return []
      return live()
        .filter((r) => r.resourceType !== 'Patient' && (!person || personOf(r) === person))
        .filter((r) => { const t = JSON.stringify(r).toLowerCase(); return ws.every((w) => t.includes(w)) })
        .map((resource) => ({ resource, snippet: '' }))
    },
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
    saveAll: async (resources, label) => {
      const out = resources.map((resource) => {
        const r = clone(resource)
        r.id ??= crypto.randomUUID()
        r.meta = { lastUpdated: new Date().toISOString() }
        return r
      })
      change(label, () => out.forEach((r) => rows.set(r.id!, { r })))
      return out
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
    reminderSettings: async () => clone((settings.get('reminders') as ReminderSettings | undefined) ?? { on: false, names: false, quiet: null }),
    setReminders: async (s) => void settings.set('reminders', clone(s)),
    dosesToday: async () => {
      const date = new Date().toLocaleDateString('en-CA')
      const log = (settings.get('reminders.log') as Record<string, { status: Dose['status']; at: string }>) ?? {}
      const people = live().filter((r) => r.resourceType === 'Patient')
      return live()
        .filter((r) => r.resourceType === 'MedicationStatement' && r.status === 'active')
        .flatMap((r) => {
          const times = ((r as { dosage?: { timing?: { repeat?: { timeOfDay?: string[] } } }[] }).dosage?.[0]?.timing?.repeat?.timeOfDay ?? []).map((t) => t.slice(0, 5))
          const person = personOf(r) ?? ''
          const taken = Object.entries(log).filter(([k, m]) => k.split('|')[1] === r.id && m.status === 'taken').map(([, m]) => m.at).sort().pop() ?? null
          return [...new Set(times)].map((time): Dose => {
            const m = log[`${date}|${r.id}|${time}`]
            return { med: r.id!, person, personName: name(people.find((p) => p.id === person) ?? r), name: name(r), date, time, status: m?.status ?? null, at: m?.at ?? null, lastTaken: taken }
          })
        })
        .sort((a, b) => a.time.localeCompare(b.time))
    },
    markDose: async (d, status) => {
      const log = { ...((settings.get('reminders.log') as Record<string, unknown>) ?? {}) }
      if (status) log[`${d.date}|${d.med}|${d.time}`] = { status, at: new Date().toISOString() }
      else delete log[`${d.date}|${d.med}|${d.time}`]
      settings.set('reminders.log', log)
    },
    refreshReminders: async () => {},
  }
}

export const api: Api = inApp ? tauri : memory()

/** Download progress of an update: bytes so far and the total, if known. */
export function onUpdateProgress(f: (done: number, total: number | null) => void): Promise<UnlistenFn> {
  if (!inApp) return Promise.resolve(() => {})
  return listen<[number, number | null]>('update-progress', (e) => f(e.payload[0], e.payload[1]))
}

/** A command's error as plain text. */
export const message = (e: unknown) => (typeof e === 'string' ? e : e instanceof Error ? e.message : String(e))
