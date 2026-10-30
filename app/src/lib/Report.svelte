<script lang="ts">
  // The doctor report: choose what to share and the period, add a note, and
  // save it as a PDF made on this device.
  import { Button, Card, Segmented } from '@swasthya/ui'
  import { api } from './api.ts'
  import { buildReport, SECTIONS, type Section } from './model/report.ts'
  import type { Span } from './model/trends.ts'
  import { session } from './session.svelte.ts'

  let chosen = $state<Record<Section, boolean>>(Object.fromEntries(SECTIONS.map((s) => [s.id, true])) as Record<Section, boolean>)
  let span = $state<Span>('1y')
  let note = $state('')
  let busy = $state(false)

  async function make() {
    const person = session.person
    if (!person) return
    busy = true
    try {
      const records = await api.timeline(person.id!)
      const data = buildReport({ person, records }, new Set(SECTIONS.filter((s) => chosen[s.id]).map((s) => s.id)), span, note)
      const { save } = await import('@tauri-apps/plugin-dialog')
      const target = await save({
        title: 'Save the report',
        defaultPath: `Health summary, ${data.person.name}, ${new Date().toISOString().slice(0, 10)}.pdf`,
        filters: [{ name: 'PDF', extensions: ['pdf'] }],
      })
      if (!target) return
      await api.saveReport(data, target)
      session.say({ text: 'Report saved. That PDF is not encrypted: share it only with people you choose.' })
    } catch (e) {
      session.fail(e)
    } finally {
      busy = false
    }
  }
</script>

<Card title="What to include" explain="Lists show everything recorded. Test results and charts cover the period chosen below. Patterns appear only when there are at least 14 days of readings.">
  <div class="checks">
    {#each SECTIONS as s (s.id)}
      <label class="check"><input type="checkbox" bind:checked={chosen[s.id]} /> {s.label}</label>
    {/each}
  </div>
  <div class="period">
    <span class="label">Results and charts from</span>
    <Segmented
      label="Period"
      options={[
        { value: '3m', label: 'Last 3 months' },
        { value: '1y', label: 'Last year' },
        { value: 'all', label: 'All' },
      ]}
      value={span}
      onchange={(v) => (span = v as Span)}
    />
  </div>
  <div class="field">
    <label class="label" for="rep-note">A note for the doctor (optional)</label>
    <textarea id="rep-note" bind:value={note} placeholder="What you would like to talk about, in your own words."></textarea>
  </div>
  <div class="actions">
    <Button disabled={busy || !Object.values(chosen).some(Boolean)} onclick={make}>{busy ? 'Making the PDF…' : 'Save as PDF'}</Button>
  </div>
  <p class="muted small">Made on this device: nothing is sent anywhere. The PDF is an ordinary file, so anyone who has it can read it.</p>
</Card>

<style>
  .checks { display: grid; gap: 6px; grid-template-columns: repeat(auto-fill, minmax(min(260px, 100%), 1fr)); margin-bottom: 16px; }
  .check { display: flex; gap: 8px; align-items: center; }
  .period { display: grid; gap: 6px; margin-bottom: 16px; justify-items: start; }
  .label { font-weight: 600; font-size: var(--text-small); }
  .small { font-size: var(--text-small); }
  .muted { color: var(--c-muted); }
  p { margin: 10px 0 0; }
</style>
