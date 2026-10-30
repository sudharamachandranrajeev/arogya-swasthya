<script lang="ts">
  // Today's doses from the reminder times on each medicine, marked taken or
  // skipped, and the reminder settings. Marking a medicine taken twice within
  // two hours asks first, to help avoid a double dose.
  import { Button, Card, Pill } from '@swasthya/ui'
  import { api, type Dose, type ReminderSettings } from './api.ts'
  import { session } from './session.svelte.ts'

  const SOON_MS = 2 * 3600_000

  let doses = $state<Dose[]>([])
  let settings = $state<ReminderSettings | null>(null)
  let asking = $state<Dose | null>(null)
  let busy = $state(false)
  let quietFrom = $state('22:00')
  let quietTo = $state('07:00')

  const load = () =>
    Promise.all([api.dosesToday(), api.reminderSettings()]).then(([d, s]) => {
      doses = d
      settings = s
      if (s.quiet) [quietFrom, quietTo] = s.quiet
    }, session.fail.bind(session))

  $effect(() => {
    void session.version
    // Medicines may have changed: reschedule, then show the new list.
    api.refreshReminders().catch(() => {})
    load()
  })

  const many = $derived(new Set(doses.map((d) => d.person)).size > 1)
  const clock = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' })
  const when = (at: string | null) => (at ? clock.format(new Date(at)) : '')
  const key = (d: Dose) => `${d.med}|${d.time}`

  async function mark(d: Dose, status: 'taken' | 'skipped' | null, sure = false) {
    if (status === 'taken' && !sure && d.lastTaken && Date.now() - Date.parse(d.lastTaken) < SOON_MS) {
      asking = d
      return
    }
    asking = null
    busy = true
    try {
      await api.markDose(d, status)
      await load()
    } catch (e) {
      session.fail(e)
    } finally {
      busy = false
    }
  }

  async function save(next: ReminderSettings, said?: string) {
    busy = true
    try {
      await api.setReminders(next)
      settings = next
      if (said) session.say({ text: said })
    } catch (e) {
      session.fail(e)
    } finally {
      busy = false
    }
  }

  const time = /^([01]\d|2[0-3]):[0-5]\d$/
  function setQuiet(on: boolean) {
    if (!settings) return
    if (on && !(time.test(quietFrom) && time.test(quietTo))) {
      session.say({ text: 'Quiet hours: use 24-hour times like 22:00 and 07:00.', error: true })
      return
    }
    save({ ...settings, quiet: on ? [quietFrom, quietTo] : null })
  }
</script>

<Card
  title="Today"
  explain="Doses due today, from the reminder times on each medicine being taken now. Mark each one taken or skipped; reminders stop once a dose is marked."
>
  {#if !doses.length}
    <p class="muted">No reminder times yet. Edit a medicine below and fill in “Reminder times”, e.g. 08:00, 20:00.</p>
  {:else}
    <ul class="doses">
      {#each doses as d (key(d))}
        <li>
          <span class="time">{d.time}</span>
          <span class="what">
            <strong>{d.name}</strong>
            {#if many}<span class="muted small">{d.personName}</span>{/if}
          </span>
          <span class="state">
            {#if d.status}
              <Pill tone={d.status === 'taken' ? 'ok' : 'done'}>{d.status === 'taken' ? 'Taken' : 'Skipped'} {when(d.at)}</Pill>
              <button class="link" disabled={busy} onclick={() => mark(d, null)}>Undo</button>
            {:else}
              <Button variant="quiet" disabled={busy} onclick={() => mark(d, 'taken')}>Taken</Button>
              <Button variant="ghost" disabled={busy} onclick={() => mark(d, 'skipped')}>Skip</Button>
            {/if}
          </span>
        </li>
        {#if asking && key(asking) === key(d)}
          <li class="ask" role="alert">
            <p>{d.name} was marked taken at {when(d.lastTaken)}, less than 2 hours ago. Taking it again could be a double dose. Mark this one taken too?</p>
            <div class="actions">
              <Button disabled={busy} onclick={() => mark(d, 'taken', true)}>Yes, I took it</Button>
              <Button variant="ghost" onclick={() => (asking = null)}>Cancel</Button>
            </div>
          </li>
        {/if}
      {/each}
    </ul>
  {/if}

  {#if settings}
    <div class="settings">
      <label class="check">
        <input type="checkbox" checked={settings.on} disabled={busy} onchange={(e) => save({ ...settings!, on: e.currentTarget.checked }, e.currentTarget.checked ? 'Reminders are on.' : 'Reminders are off.')} />
        Remind me when a dose is due
      </label>
      {#if settings.on}
        <p class="muted small">
          {session.status?.platform === 'android'
            ? 'Reminders come from the phone, even when the app is closed. Battery saving may delay one by a few minutes.'
            : 'Closing the window keeps the app running in the tray, and it starts when you log in, so reminders still arrive. Quit from the tray icon to stop them.'}
          If a dose is still not marked half an hour later, you get one more reminder.
        </p>
        <label class="check">
          <input type="checkbox" checked={settings.names} disabled={busy} onchange={(e) => save({ ...settings!, names: e.currentTarget.checked })} />
          Show the medicine’s name in reminders
        </label>
        <p class="muted small indent">Off by default: anyone who sees your screen, even locked, can read a reminder. Off, it says only “Time for a medicine”.</p>
        <label class="check">
          <input type="checkbox" checked={!!settings.quiet} disabled={busy} onchange={(e) => setQuiet(e.currentTarget.checked)} />
          Quiet hours: no second reminder from
          <input class="hhmm" aria-label="Quiet hours start" inputmode="numeric" bind:value={quietFrom} onchange={() => settings?.quiet && setQuiet(true)} />
          to
          <input class="hhmm" aria-label="Quiet hours end" inputmode="numeric" bind:value={quietTo} onchange={() => settings?.quiet && setQuiet(true)} />
        </label>
      {/if}
      <p class="muted small">Reminders help, but they are not a safety system: check the label and ask your pharmacist if you are unsure whether you took a dose.</p>
    </div>
  {/if}
</Card>

<style>
  .doses { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }
  .doses li { display: flex; align-items: center; gap: 12px; padding: 8px 0; border-bottom: 1px solid var(--c-line); flex-wrap: wrap; }
  .time { font-variant-numeric: tabular-nums; font-weight: 600; min-width: 3.2em; }
  .what { flex: 1; min-width: 10em; display: grid; }
  .state { display: inline-flex; align-items: center; gap: 8px; font-size: var(--text-small); }
  .ask { display: grid !important; background: var(--c-warn-soft); border-radius: 8px; padding: 10px 12px !important; }
  .ask p { margin: 0 0 8px; }
  .actions { display: flex; gap: 8px; flex-wrap: wrap; }
  .settings { margin-top: 16px; display: grid; gap: 6px; }
  .check { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .hhmm {
    width: 4.5em; font: inherit; color: var(--c-ink); background: var(--c-panel);
    border: 1px solid var(--c-line); border-radius: var(--r-control); padding: 4px 8px;
  }
  .indent { margin-left: 26px; }
  .small { font-size: var(--text-small); }
  .muted { color: var(--c-muted); }
  p { margin: 0; }
</style>
