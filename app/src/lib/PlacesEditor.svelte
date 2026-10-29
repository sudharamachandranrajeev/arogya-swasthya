<script lang="ts">
  // The countries a person lives in and where they are now. Emergency numbers
  // follow "where I am now"; any number can be corrected, and a country not
  // in the list is added with its own name and number.
  import { Button, Explain } from '@swasthya/ui'
  import { COUNTRIES, addHome, country, removeHome, type Places } from './model/places.ts'

  let { places, onchange }: { places: Places; onchange: (p: Places) => void } = $props()

  const OTHER = '__other'
  let adding = $state('')
  let other = $state({ name: '', emergency: '', urgent: '' })
  let fixing = $state(false)
  let fix = $state({ emergency: '', urgent: '' })

  const nameOf = (code: string) => country(places, code)?.name ?? code
  const now = $derived(country(places, places.current))
  const listed = (code: string) => COUNTRIES.some((c) => c.code === code)
  const visiting = $derived(places.current && !places.homes.includes(places.current))

  function add(asHome: boolean) {
    let p = places
    let code = adding
    if (adding === OTHER) {
      if (!other.name.trim() || !other.emergency.trim()) return
      code = 'X-' + other.name.trim().toUpperCase().replace(/[^A-Z0-9]+/g, '-')
      p = { ...p, custom: { ...p.custom, [code]: { name: other.name.trim(), emergency: other.emergency.trim(), urgent: other.urgent.trim() || undefined } } }
    }
    if (!code) return
    onchange(asHome ? addHome(p, code) : { ...p, current: code })
    adding = ''
    other = { name: '', emergency: '', urgent: '' }
  }

  function saveFix() {
    if (!fix.emergency.trim()) return
    const own = { name: places.custom[places.current]?.name, emergency: fix.emergency.trim(), urgent: fix.urgent.trim() || undefined }
    onchange({ ...places, custom: { ...places.custom, [places.current]: own } })
    fixing = false
  }
</script>

<div class="places">
  <div class="field">
    <span class="label">Where you live <Explain title="Where you live" text="Add every country you live in for part of the year. You can switch between them below when you travel." /></span>
    {#if places.homes.length}
      <ul class="homes">
        {#each places.homes as h (h)}
          <li>{nameOf(h)} <button class="link" onclick={() => onchange(removeHome(places, h))} aria-label={`Remove ${nameOf(h)}`}>Remove</button></li>
        {/each}
      </ul>
    {:else}
      <p class="hint">No country yet.</p>
    {/if}
  </div>

  <div class="field">
    <label class="label" for="pl-add">Add a country</label>
    <div class="row">
      <select id="pl-add" bind:value={adding}>
        <option value="">Choose…</option>
        {#each COUNTRIES as c (c.code)}<option value={c.code}>{c.name}</option>{/each}
        <option value={OTHER}>Another country…</option>
      </select>
      <Button variant="quiet" disabled={!adding} onclick={() => add(true)}>I live here</Button>
      {#if places.homes.length}<Button variant="ghost" disabled={!adding} onclick={() => add(false)}>I'm visiting</Button>{/if}
    </div>
    {#if adding === OTHER}
      <div class="fields">
        <div class="field"><label class="label" for="pl-on">Country</label><input id="pl-on" bind:value={other.name} /></div>
        <div class="field"><label class="label" for="pl-oe">Emergency number</label><input id="pl-oe" bind:value={other.emergency} inputmode="tel" /></div>
        <div class="field"><label class="label" for="pl-ou">Urgent advice line (optional)</label><input id="pl-ou" bind:value={other.urgent} /></div>
      </div>
    {/if}
  </div>

  {#if places.homes.length}
    <div class="field">
      <label class="label" for="pl-now">Where you are now</label>
      <select id="pl-now" value={places.current} onchange={(e) => onchange({ ...places, current: e.currentTarget.value })}>
        {#each places.homes as h (h)}<option value={h}>{nameOf(h)}</option>{/each}
        {#if visiting}<option value={places.current}>{nameOf(places.current)} (visiting)</option>{/if}
      </select>
    </div>
  {/if}

  {#if places.current}
    <div class="now" role="status">
      {#if now}
        <strong>Emergency in {now.name}: {now.emergency}</strong>
        {#if now.urgent}<span>Urgent advice: {now.urgent}</span>{/if}
        {#if !fixing}
          <button class="link" onclick={() => ((fix = { emergency: now.emergency, urgent: now.urgent ?? '' }), (fixing = true))}>
            {listed(places.current) ? 'Correct these numbers' : 'Change these numbers'}
          </button>
        {/if}
      {/if}
      {#if fixing || !now}
        <div class="fields">
          <div class="field"><label class="label" for="pl-fe">Emergency number</label><input id="pl-fe" bind:value={fix.emergency} inputmode="tel" /></div>
          <div class="field"><label class="label" for="pl-fu">Urgent advice line (optional)</label><input id="pl-fu" bind:value={fix.urgent} /></div>
        </div>
        <div class="actions"><Button variant="quiet" onclick={saveFix}>Save numbers</Button>{#if fixing}<Button variant="ghost" onclick={() => (fixing = false)}>Cancel</Button>{/if}</div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .places { display: grid; gap: 16px; }
  .homes { margin: 0; padding-left: 18px; display: grid; gap: 4px; }
  .homes .link { margin-left: 8px; font-size: var(--text-small); }
  .row { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .row select { flex: 1 1 200px; width: auto; }
  .now { display: grid; gap: 6px; padding: 12px 14px; border-radius: var(--r-sm); background: var(--c-danger-soft); }
  .now .link { justify-self: start; font-size: var(--text-small); }
</style>
