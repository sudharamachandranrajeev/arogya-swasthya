<script lang="ts">
  import { Card, Segmented, Shell, ThemeEditor, ThemePicker, ui, type NavItem } from '@swasthya/ui'
  import Activity from '@lucide/svelte/icons/activity'
  import Camera from '@lucide/svelte/icons/camera'
  import Dumbbell from '@lucide/svelte/icons/dumbbell'
  import FileText from '@lucide/svelte/icons/file-text'
  import FolderOpen from '@lucide/svelte/icons/folder-open'
  import History from '@lucide/svelte/icons/history'
  import ImportIcon from '@lucide/svelte/icons/import'
  import LayoutDashboard from '@lucide/svelte/icons/layout-dashboard'
  import ListChecks from '@lucide/svelte/icons/list-checks'
  import MapPin from '@lucide/svelte/icons/map-pin'
  import Palette from '@lucide/svelte/icons/palette'
  import PersonStanding from '@lucide/svelte/icons/person-standing'
  import PillIcon from '@lucide/svelte/icons/pill'
  import SearchIcon from '@lucide/svelte/icons/search'
  import SettingsIcon from '@lucide/svelte/icons/settings'
  import Documents from './lib/Documents.svelte'
  import Home from './lib/Home.svelte'
  import Import from './lib/Import.svelte'
  import Lock from './lib/Lock.svelte'
  import { allergy, condition, medication, procedure, result, vaccine } from './lib/model/fhir.ts'
  import { emergencyLine } from './lib/model/places.ts'
  import RecordList from './lib/RecordList.svelte'
  import Search from './lib/Search.svelte'
  import { session } from './lib/session.svelte.ts'
  import Settings from './lib/Settings.svelte'
  import Timeline from './lib/Timeline.svelte'
  import Toast from './lib/Toast.svelte'
  import UpdateBanner from './lib/UpdateBanner.svelte'
  import { updates } from './lib/updates.svelte.ts'
  import Wizard from './lib/Wizard.svelte'

  const items: NavItem[] = [
    { id: 'overview', label: 'Overview', icon: LayoutDashboard },
    { id: 'search', label: 'Search', icon: SearchIcon },
    { id: 'records', label: 'Records', icon: FileText },
    { id: 'medicines', label: 'Medicines', icon: PillIcon },
    { id: 'results', label: 'Results', icon: Activity },
    { id: 'documents', label: 'Documents', icon: FolderOpen },
    { id: 'import', label: 'Import', icon: ImportIcon },
    { id: 'timeline', label: 'Timeline', icon: History },
    { id: 'body', label: 'Body', icon: PersonStanding },
    { id: 'care', label: 'Find care', icon: MapPin },
    { id: 'exercise', label: 'Exercise', icon: Dumbbell },
    { id: 'photos', label: 'Photos', icon: Camera },
    { id: 'plan', label: 'Plan', icon: ListChecks },
    { id: 'settings', label: 'Settings', icon: SettingsIcon },
    { id: 'appearance', label: 'Appearance', icon: Palette },
  ]
  const later: Record<string, string> = {
    body: 'A 3D body that lights up where your conditions, procedures and photos belong.',
    care: 'Finding care near you, with every source checked and shown.',
    exercise: 'Exercise plans that respect your injuries and conditions.',
    photos: 'Body photo series with consistent light and angle checks.',
    plan: 'A care plan for life: screenings, vaccines and follow-ups.',
  }

  const recordKinds = [condition, allergy, procedure, vaccine]
  let recordKind = $state(condition.type)

  const fromHash = () => location.hash.replace(/^#\/?/, '').split('?')[0] || 'overview'
  let page = $state(fromHash())
  addEventListener('hashchange', () => (page = fromHash()))
  const label = $derived(items.find((i) => i.id === page)?.label ?? 'Overview')

  session.load().catch((e) => session.fail(e))
  const status = $derived(session.status)
  // Check for a new version once the app is open and set up.
  $effect(() => {
    if (status?.open && !status.locked && session.ready) updates.startup()
  })
</script>

{#if !status}
  <!-- Loading: a blank page avoids a flash of the wrong screen. -->
{:else if !status.open || status.locked}
  <Lock {status} onchange={() => session.load()} />
{:else if !session.ready}
  <Wizard />
{:else}
  <Shell brand="Arogya Swasthya" {items} current={page} onnavigate={(id) => (location.hash = `/${id}`)}>
    {#snippet footer()}
      {#if session.people.length > 1}
        <label class="who">Showing
          <select value={session.current} onchange={(e) => session.choose(e.currentTarget.value)}>
            {#each session.people as p (p.id)}<option value={p.id}>{(p.name as { text?: string }[] | undefined)?.[0]?.text ?? 'Unnamed'}</option>{/each}
          </select>
        </label>
      {/if}
      <span class="sos">{emergencyLine(session.places)}</span><br />
      Stored on this device · encrypted{ui.calm ? ' · calm' : ''}
    {/snippet}
    <UpdateBanner />
    {#key page}
      <div class="page">
        {#if !session.current && page !== 'settings' && page !== 'appearance'}
          <h1>Nobody here yet</h1>
          <p class="lead">Every record belongs to a person. Add yourself, or someone you look after, to start.</p>
          <p><a href="#/settings">Open Settings → People</a>, or restore someone from the trash there.</p>
        {:else if page === 'overview'}
          <Home />
        {:else if page === 'records'}
          <h1>Records</h1>
          <p class="lead">{session.personName}'s conditions, allergies, procedures and vaccines.</p>
          <div class="tabs">
            <Segmented label="Kind of record" options={recordKinds.map((k) => ({ value: k.type, label: k.title }))} value={recordKind} onchange={(v) => (recordKind = v)} />
          </div>
          {#key recordKind}<RecordList kind={recordKinds.find((k) => k.type === recordKind)!} />{/key}
        {:else if page === 'medicines'}
          <h1>Medicines</h1>
          <p class="lead">What {session.personName} takes, or has taken. Reminders arrive in a later version.</p>
          <RecordList kind={medication} />
        {:else if page === 'results'}
          <h1>Results</h1>
          <p class="lead">Test results with the lab's normal range. Outside the range is not always a problem: ask the person who ordered the test.</p>
          <RecordList kind={result} />
        {:else if page === 'search'}
          <h1>Search</h1>
          <p class="lead">Find anything in the records by the words in it, including words read from documents.</p>
          <Search />
        {:else if page === 'documents'}
          <h1>Documents</h1>
          <p class="lead">{session.personName}'s letters, reports, scans, photos and recordings, encrypted on this device.</p>
          <Documents />
        {:else if page === 'import'}
          <h1>Import</h1>
          <p class="lead">Bring in records from a clinic, a hospital or another health app.</p>
          <Import />
        {:else if page === 'timeline'}
          <h1>Timeline</h1>
          <p class="lead">Everything in {session.personName}'s record, newest first.</p>
          <Card><Timeline /></Card>
        {:else if page === 'settings'}
          <Settings />
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
          <p class="lead">{later[page] ?? ''} Coming in a later version.</p>
        {/if}
      </div>
    {/key}
  </Shell>
{/if}
<Toast />

<style>
  .page { animation: enter var(--dur-slow) var(--ease); }
  @keyframes enter { from { opacity: 0; translate: 0 8px; } }
  .gap { height: 18px; }
  .tabs { margin-bottom: 16px; overflow-x: auto; }
  .who { display: grid; gap: 4px; margin-bottom: 8px; }
  .who select { font: inherit; padding: 4px 6px; border-radius: var(--r-control); border: 1px solid var(--c-line); background: var(--c-panel); color: var(--c-ink); }
  .sos { font-weight: 600; }
</style>
