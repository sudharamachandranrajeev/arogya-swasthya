// Update state shared by the start-up banner and Settings → Updates. The app
// checks once at start (unless turned off) and never installs without the
// user pressing the button.
import { api, onUpdateProgress, type InstallKind, type UpdateInfo, type UpdateStatus } from './api.ts'
import { session } from './session.svelte.ts'

export const kindName: Record<InstallKind, string> = {
  installer: 'installed with the Windows installer',
  appImage: 'the AppImage',
  pacman: 'the Arch Linux package',
  portable: 'the portable version for Windows',
  android: 'the Android app',
  other: 'a copy built from source or installed another way',
}

class Updates {
  status = $state<UpdateStatus | null>(null)
  found = $state<UpdateInfo | null>(null)
  busy = $state(false)
  /** 0 to 1 while downloading, null otherwise. */
  progress = $state<number | null>(null)
  /** The banner was put away for this run. */
  dismissed = $state(false)
  #started = false
  #listening = false

  async load() {
    this.status = await api.updateStatus()
  }

  /** Once, after unlocking: check quietly unless updates are off. Offline is not an error here. */
  async startup() {
    if (this.#started) return
    this.#started = true
    try {
      await this.load()
      if (this.status?.mode === 'ask') await this.check(true)
    } catch {
      // Status could not be read: Settings shows the problem.
    }
  }

  async check(quiet = false) {
    try {
      this.found = await api.checkUpdate()
      if (!quiet && !this.found) session.say({ text: 'Arogya Swasthya is up to date.' })
    } catch (e) {
      if (!quiet) session.fail(e)
    }
  }

  async #listen() {
    if (this.#listening) return
    this.#listening = true
    await onUpdateProgress((done, total) => (this.progress = total ? done / total : null))
  }

  /** Back up, download, check the signature, install and restart. */
  async install() {
    if (!this.found) return
    await this.#run(() => api.installUpdate(this.found!.version))
    if (this.status?.kind === 'android') session.say({ text: 'Android is asking you to confirm the update.' })
  }

  async rollback(restore: boolean) {
    await this.#run(() => api.rollback(restore))
  }

  async #run(f: () => Promise<void>) {
    this.busy = true
    await this.#listen()
    try {
      await f()
    } catch (e) {
      session.fail(e)
    } finally {
      this.busy = false
      this.progress = null
    }
  }
}

export const updates = new Updates()
