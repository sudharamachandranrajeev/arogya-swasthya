<script lang="ts">
  // Settings → Updates: the check-at-start setting, going back a version, and
  // reporting a problem or suggesting a feature as a prefilled GitHub issue.
  import { Button, Card, ui } from '@swasthya/ui'
  import { api, inApp, type Page, type RollbackPlan, type UpdateMode } from './api.ts'
  import { recentErrors, session } from './session.svelte.ts'
  import { kindName, updates } from './updates.svelte.ts'

  let plan = $state<RollbackPlan | null>(null)
  let confirming = $state(false)
  let checking = $state(false)
  const s = $derived(updates.status)

  $effect(() => {
    updates
      .load()
      .then(() => api.rollbackPlan())
      .then((p) => (plan = p))
      .catch((e) => session.fail(e))
  })

  async function setMode(mode: UpdateMode) {
    try {
      await api.setUpdateMode(mode)
      await updates.load()
    } catch (e) {
      session.fail(e)
    }
  }

  async function checkNow() {
    checking = true
    updates.dismissed = false
    await updates.check()
    checking = false
  }

  async function report(page: Page) {
    try {
      await api.openPage(page, { theme: ui.theme, motion: ui.motion, errors: recentErrors().join('\n') })
    } catch (e) {
      session.fail(e)
    }
  }
</script>

<Card title="Updates" explain="New versions come from the project's GitHub releases. Every download is checked against the project's signature before it is installed.">
  {#if s}
    <p>
      This is Arogya Swasthya {s.current}, {kindName[s.kind]}.
      {#if s.canInstall}It can update itself.{:else}It cannot update itself, but it tells you when a new version is out.{/if}
    </p>
    <div class="field">
      <label class="label" for="u-mode">When the app starts</label>
      <select id="u-mode" value={s.mode} onchange={(e) => setMode(e.currentTarget.value as UpdateMode)}>
        <option value="ask">Check for a new version and ask me before installing</option>
        <option value="off">Do not check</option>
      </select>
      <p class="muted small">Checking needs the internet. Offline, the app skips the check and says nothing.</p>
    </div>
    <div class="actions">
      <Button variant="quiet" disabled={checking || !inApp} onclick={checkNow}>{checking ? 'Checking…' : 'Check now'}</Button>
    </div>
    {#if updates.found}<p class="muted small">Version {updates.found.version} is out: see the message at the top of the page.</p>{/if}

    <h3>Go back a version</h3>
    {#if s.kind === 'android'}
      <p class="muted">Android does not install an older version over a newer one. Your records from before each update are kept under Backups and can be restored.</p>
    {:else if !plan}
      <p class="muted">There is no earlier version to go back to yet. This appears after the app updates itself.</p>
    {:else if !s.canInstall}
      <p class="muted">This copy cannot install versions itself. Download {plan.version} from the release page.</p>
    {:else}
      <p class="muted">Before the last update you had version {plan.version}. Going back downloads it, checks it the same way as an update, backs up your records, and restarts.</p>
      {#if plan.needsRestore}
        <p class="warn">Version {plan.version} cannot read your records as they are now. Going back puts the backup from before the update in their place, so changes made since the update are lost. Your current records are backed up first.</p>
      {/if}
      {#if confirming}
        <div class="actions">
          <Button disabled={updates.busy} onclick={() => updates.rollback(plan!.needsRestore)}>{updates.busy ? 'Working…' : `Go back to ${plan.version}`}</Button>
          <Button variant="ghost" disabled={updates.busy} onclick={() => (confirming = false)}>Cancel</Button>
        </div>
        {#if updates.progress !== null}<progress max="1" value={updates.progress} aria-label="Download"></progress>{/if}
      {:else}
        <div class="actions"><Button variant="quiet" onclick={() => (confirming = true)}>Go back to {plan.version}…</Button></div>
      {/if}
    {/if}
  {/if}
</Card>
<div class="gap"></div>
<Card title="Tell the makers" explain="Opens a new GitHub issue in your browser with the version, system, theme and the last three error messages filled in. Words from your records are taken out of the error messages. Nothing is sent until you press Submit on GitHub.">
  <p>Something not working, or an idea that would help? You see everything on GitHub before you send it.</p>
  <div class="actions">
    <Button variant="quiet" disabled={!inApp} onclick={() => report('bug')}>Report a problem</Button>
    <Button variant="quiet" disabled={!inApp} onclick={() => report('idea')}>Suggest a feature</Button>
  </div>
</Card>

<style>
  h3 { margin: 22px 0 6px; font-size: 1rem; }
  .field { display: grid; gap: 6px; margin-top: 12px; }
  .small { font-size: var(--text-small); margin: 0; }
  .warn { color: var(--c-warn); font-weight: 600; }
  .gap { height: 18px; }
  progress { width: 100%; margin-top: 10px; accent-color: var(--c-accent); }
</style>
