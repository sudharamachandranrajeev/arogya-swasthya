// The Rust shell's commands. Outside Tauri (the browser preview) there is no
// shell: `inApp` is false and the screens show made-up content.
import { invoke } from '@tauri-apps/api/core'

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

export const api = {
  status: () => invoke<Status>('status'),
  unlockPin: (pin: string) => invoke<boolean>('unlock_pin', { pin }),
  unlockBiometric: () => invoke<void>('unlock_biometric'),
  unlockWith: (recovery: string) => invoke<void>('unlock_with', { recovery }),
  lock: () => invoke<boolean>('lock'),
}

/** A command's error as plain text. */
export const message = (e: unknown) => (typeof e === 'string' ? e : e instanceof Error ? e.message : String(e))
