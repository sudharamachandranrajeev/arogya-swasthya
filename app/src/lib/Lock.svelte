<script lang="ts">
  // Shown instead of the app while the data is closed or the app lock is on.
  import { Button, Card } from '@swasthya/ui'
  import { api, message, type Status } from './api.ts'

  let { status, onchange }: { status: Status; onchange: () => void } = $props()

  let pin = $state('')
  let recovery = $state('')
  let error = $state('')
  let busy = $state(false)
  // No PIN or biometric left to try (closed data, or the user chose this).
  let useRecovery = $state(false)
  const needRecovery = $derived(!status.open || useRecovery)

  async function run(f: () => Promise<unknown>) {
    busy = true
    error = ''
    try {
      await f()
      onchange()
    } catch (e) {
      error = message(e)
    } finally {
      busy = false
    }
  }

  const tryPin = () =>
    run(async () => {
      const ok = await api.unlockPin(pin)
      pin = ''
      if (!ok) throw 'That PIN is not right.'
    })
</script>

<main>
  <Card title="Arogya Swasthya">
    {#if needRecovery}
      {#if status.problem}<p class="note">{status.problem}</p>{/if}
      <form onsubmit={(e) => (e.preventDefault(), run(() => api.unlockWith(recovery)))}>
        <label for="recovery">Recovery key</label>
        <input id="recovery" bind:value={recovery} autocomplete="off" spellcheck="false" placeholder="xxxx-xxxx-…" />
        <p class="hint">The 64-character key you wrote down when you first set up the app.</p>
        <Button type="submit" disabled={busy || !recovery.trim()}>Open my records</Button>
      </form>
      {#if status.open}<button class="link" onclick={() => (useRecovery = false)}>Back</button>{/if}
    {:else}
      <p class="lead">Locked. Your records stay encrypted on this device.</p>
      {#if status.biometric}
        <Button disabled={busy} onclick={() => run(api.unlockBiometric)}>Unlock with fingerprint or face</Button>
      {/if}
      {#if status.hasPin}
        <form onsubmit={(e) => (e.preventDefault(), tryPin())}>
          <label for="pin">PIN</label>
          <input id="pin" bind:value={pin} type="password" inputmode="numeric" autocomplete="off" />
          <Button type="submit" disabled={busy || pin.length < 4 || status.waitSecs > 0}>Unlock</Button>
        </form>
      {/if}
      <button class="link" onclick={() => (useRecovery = true)}>Use the recovery key instead</button>
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </Card>
</main>

<style>
  main { min-height: 100dvh; display: grid; place-items: center; padding: 16px; background: var(--c-bg); }
  main :global(section) { width: min(420px, 100%); }
  form { display: grid; gap: 10px; margin: 12px 0; }
  label { font-weight: 600; font-size: var(--text-small); }
  input {
    font: inherit; padding: 10px 12px; border-radius: var(--r-control);
    border: 1px solid var(--c-line); background: var(--c-panel); color: var(--c-ink);
  }
  .lead, .hint, .note { color: var(--c-muted); }
  .hint { font-size: var(--text-small); margin: 0; }
  .error { color: var(--c-danger); }
  .link { background: none; border: 0; padding: 0; color: var(--c-accent); cursor: pointer; font: inherit; text-decoration: underline; }
</style>
