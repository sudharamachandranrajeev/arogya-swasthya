<script lang="ts">
  import { Card, Shell, ThemeEditor, ThemePicker, ui, type NavItem } from '@swasthya/ui'
  import Activity from '@lucide/svelte/icons/activity'
  import Camera from '@lucide/svelte/icons/camera'
  import Dumbbell from '@lucide/svelte/icons/dumbbell'
  import FileText from '@lucide/svelte/icons/file-text'
  import LayoutDashboard from '@lucide/svelte/icons/layout-dashboard'
  import ListChecks from '@lucide/svelte/icons/list-checks'
  import MapPin from '@lucide/svelte/icons/map-pin'
  import Palette from '@lucide/svelte/icons/palette'
  import PersonStanding from '@lucide/svelte/icons/person-standing'
  import PillIcon from '@lucide/svelte/icons/pill'
  import Overview from './lib/Overview.svelte'
  import Lock from './lib/Lock.svelte'
  import { api, inApp, type Status } from './lib/api'

  const items: NavItem[] = [
    { id: 'overview', label: 'Overview', icon: LayoutDashboard },
    { id: 'body', label: 'Body', icon: PersonStanding },
    { id: 'records', label: 'Records', icon: FileText },
    { id: 'care', label: 'Find care', icon: MapPin },
    { id: 'results', label: 'Results', icon: Activity },
    { id: 'medicines', label: 'Medicines', icon: PillIcon },
    { id: 'exercise', label: 'Exercise', icon: Dumbbell },
    { id: 'photos', label: 'Photos', icon: Camera },
    { id: 'plan', label: 'Plan', icon: ListChecks },
    { id: 'appearance', label: 'Appearance', icon: Palette },
  ]

  const fromHash = () => location.hash.replace(/^#\/?/, '').split('?')[0] || 'overview'
  let page = $state(fromHash())
  addEventListener('hashchange', () => (page = fromHash()))
  const label = $derived(items.find((i) => i.id === page)?.label ?? 'Overview')

  // In the app, nothing shows until the data is open and the app lock is off.
  let status = $state<Status | null>(null)
  const refresh = async () => (status = await api.status())
  if (inApp) refresh()
  const gated = $derived(inApp && (!status || !status.open || status.locked))
</script>

{#if gated}
  {#if status}<Lock {status} onchange={refresh} />{/if}
{:else}

<Shell brand="Arogya Swasthya" {items} current={page} onnavigate={(id) => (location.hash = `/${id}`)}>
  {#snippet footer()}Stored on this device · encrypted<br />Theme: {ui.current.label}{ui.calm ? ' · calm' : ''}{/snippet}
  {#key page}
    <div class="page">
      {#if page === 'overview'}
        <Overview />
      {:else if page === 'appearance'}
        <h1>Appearance</h1>
        <p class="lead">One choice changes every screen. Make your own theme below; it is checked so text stays easy to read.</p>
        <Card title="Theme" explain="Pick a look for the whole app. Light or dark and motion apply to every theme.">
          <ThemePicker />
        </Card>
        <div class="gap"></div>
        <Card title="Make your own" explain="Start from any theme, change colours, fonts and shapes, and see the result live before saving.">
          <ThemeEditor />
        </Card>
      {:else}
        <h1>{label}</h1>
        <p class="lead">Coming in a later phase.</p>
      {/if}
    </div>
  {/key}
</Shell>
{/if}

<style>
  .page { animation: enter var(--dur-slow) var(--ease); }
  @keyframes enter { from { opacity: 0; translate: 0 8px; } }
  h1 { font-size: var(--text-display); font-weight: var(--weight-display); }
  .lead { color: var(--c-muted); margin: 4px 0 20px; max-width: 70ch; }
  .gap { height: 18px; }
</style>
