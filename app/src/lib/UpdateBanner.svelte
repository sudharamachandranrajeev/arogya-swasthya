<script lang="ts">
  // Shown at the top of every page when a new version is out. Nothing is
  // installed until the user presses the button.
  import { Button } from '@swasthya/ui'
  import { api } from './api.ts'
  import { session } from './session.svelte.ts'
  import { updates } from './updates.svelte.ts'

  const u = $derived(updates.found)
  let notes = $state(false)
</script>

{#if u && !updates.dismissed}
  <div class="banner" role="status">
    <p>
      <strong>Arogya Swasthya {u.version} is out.</strong>
      {#if u.canInstall}
        You have {updates.status?.current}. Your records are backed up before anything is installed, and you can go back afterwards in Settings → Updates.
      {:else if updates.status?.kind === 'portable'}
        This is the portable version, so it cannot update itself. Download the new version and replace this folder's files with it. Your records stay where they are.
      {:else}
        This copy cannot update itself. Download the new version from the release page.
      {/if}
      {#if u.notes}<button class="link" onclick={() => (notes = !notes)}>{notes ? 'Hide what is new' : 'What is new'}</button>{/if}
    </p>
    {#if notes && u.notes}<pre class="notes">{u.notes}</pre>{/if}
    {#if updates.progress !== null}<progress max="1" value={updates.progress} aria-label="Download"></progress>{/if}
    <div class="actions">
      {#if u.canInstall}
        <Button disabled={updates.busy} onclick={() => updates.install()}>{updates.busy ? 'Installing…' : 'Install and restart'}</Button>
      {:else}
        <Button onclick={() => api.openPage('releases').catch((e) => session.fail(e))}>Open the download page</Button>
      {/if}
      <Button variant="ghost" disabled={updates.busy} onclick={() => (updates.dismissed = true)}>Later</Button>
    </div>
  </div>
{/if}

<style>
  .banner {
    margin-bottom: 18px; padding: 14px 18px; border-radius: var(--r-sm);
    background: var(--c-accent-soft); border-left: 4px solid var(--c-accent);
  }
  .banner p { margin: 0; }
  .banner .actions { margin-top: 10px; }
  .notes { white-space: pre-wrap; font: inherit; max-height: 200px; overflow: auto; color: var(--c-muted); }
  progress { width: 100%; margin-top: 10px; accent-color: var(--c-accent); }
</style>
