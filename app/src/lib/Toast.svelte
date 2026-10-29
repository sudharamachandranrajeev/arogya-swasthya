<script lang="ts">
  // The one message line at the bottom: what just happened, with Undo.
  import { session } from './session.svelte.ts'
</script>

{#if session.toast}
  <div class="toast" class:error={session.toast.error} role={session.toast.error ? 'alert' : 'status'}>
    <span>{session.toast.text}</span>
    {#if session.toast.undo}<button class="link" onclick={() => session.undo()}>Undo</button>{/if}
    <button class="close" aria-label="Close" onclick={() => (session.toast = null)}>×</button>
  </div>
{/if}

<style>
  .toast {
    position: fixed; z-index: 50; left: 50%; translate: -50% 0; bottom: calc(16px + env(safe-area-inset-bottom) + var(--tabbar-h, 0px));
    display: flex; gap: 14px; align-items: center; max-width: min(560px, calc(100vw - 32px));
    padding: 10px 14px; border-radius: var(--r-sm); background: var(--c-ink); color: var(--c-bg);
    box-shadow: 0 10px 30px rgb(0 0 0 / .25); animation: rise var(--dur-base) var(--ease);
  }
  .toast.error { background: var(--c-danger); color: #fff; }
  .toast .link { color: inherit; font-weight: 700; }
  .close { background: none; border: 0; color: inherit; font-size: 1.2rem; cursor: pointer; padding: 0 2px; }
  @keyframes rise { from { opacity: 0; translate: -50% 8px; } }
  @media (max-width: 720px) { .toast { bottom: calc(80px + env(safe-area-inset-bottom)); } }
</style>
