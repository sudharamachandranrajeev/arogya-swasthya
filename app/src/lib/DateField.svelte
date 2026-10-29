<script lang="ts">
  // A date box with a drop-down calendar. Health dates are often partial
  // ("2015", "2015-06"), so the box stays editable and the calendar can also
  // fill in just the month or just the year. Future days cannot be picked.
  import CalendarIcon from '@lucide/svelte/icons/calendar'
  import ChevronLeft from '@lucide/svelte/icons/chevron-left'
  import ChevronRight from '@lucide/svelte/icons/chevron-right'

  let { id, value = $bindable() }: { id: string; value?: string } = $props()

  const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December']
  const DAYS = ['Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su']
  const pad = (n: number) => String(n).padStart(2, '0')
  const now = new Date()
  const today = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`
  const years = Array.from({ length: now.getFullYear() - 1899 }, (_, i) => now.getFullYear() - i)

  let year = $state(now.getFullYear())
  let month = $state(now.getMonth())
  let button: HTMLButtonElement
  let panel: HTMLDivElement
  const panelId = $derived(`${id}-calendar`)

  // Weeks start on Monday; blanks before the 1st.
  const cells = $derived.by(() => {
    const first = (new Date(year, month, 1).getDay() + 6) % 7
    const count = new Date(year, month + 1, 0).getDate()
    return [...Array(first).fill(0), ...Array.from({ length: count }, (_, i) => i + 1)] as number[]
  })
  const iso = (d: number) => `${year}-${pad(month + 1)}-${pad(d)}`
  const future = (d: number) => iso(d) > today
  const atEnd = $derived(year === now.getFullYear() && month === now.getMonth())

  function step(by: number) {
    const d = new Date(year, month + by, 1)
    year = d.getFullYear()
    month = d.getMonth()
  }

  function open(e: Event) {
    if ((e as ToggleEvent).newState !== 'open') return
    // Start at the date already typed, if any.
    const m = /^(\d{4})(?:-(\d{2}))?/.exec(value ?? '')
    if (m) {
      year = Math.min(Number(m[1]), now.getFullYear())
      month = m[2] ? Number(m[2]) - 1 : 0
    }
    const b = button.getBoundingClientRect()
    const w = Math.min(300, innerWidth - 24)
    panel.style.width = `${w}px`
    panel.style.left = `${Math.max(12, Math.min(b.right - w, innerWidth - w - 12))}px`
    const h = panel.offsetHeight
    panel.style.top = `${b.bottom + 6 + h > innerHeight - 12 ? Math.max(12, b.top - h - 6) : b.bottom + 6}px`
  }

  function pick(v: string) {
    value = v
    panel.hidePopover()
    button.focus()
  }
</script>

<div class="date">
  <input {id} bind:value inputmode="numeric" placeholder="YYYY-MM-DD" autocomplete="off" />
  <button bind:this={button} type="button" class="open" popovertarget={panelId} aria-label="Choose a date from a calendar">
    <CalendarIcon size={18} />
  </button>
</div>

<div bind:this={panel} id={panelId} popover class="panel" role="dialog" aria-label="Calendar" ontoggle={open}>
  <div class="head">
    <button type="button" class="nav" onclick={() => step(-1)} aria-label="Previous month"><ChevronLeft size={18} /></button>
    <select bind:value={month} aria-label="Month">
      {#each MONTHS as m, i (m)}<option value={i} disabled={year === now.getFullYear() && i > now.getMonth()}>{m}</option>{/each}
    </select>
    <select bind:value={year} aria-label="Year">
      {#each years as y (y)}<option value={y}>{y}</option>{/each}
    </select>
    <button type="button" class="nav" onclick={() => step(1)} disabled={atEnd} aria-label="Next month"><ChevronRight size={18} /></button>
  </div>
  <div class="grid" role="grid">
    {#each DAYS as d (d)}<span class="dow" aria-hidden="true">{d}</span>{/each}
    {#each cells as d, i (i)}
      {#if d === 0}<span></span>{:else}
        <button
          type="button"
          class="day"
          class:chosen={value === iso(d)}
          class:today={iso(d) === today}
          disabled={future(d)}
          aria-label={`${d} ${MONTHS[month]} ${year}`}
          onclick={() => pick(iso(d))}>{d}</button>
      {/if}
    {/each}
  </div>
  <div class="foot">
    <button type="button" class="link" onclick={() => pick(`${year}-${pad(month + 1)}`)}>Only {MONTHS[month]} {year}</button>
    <button type="button" class="link" onclick={() => pick(String(year))}>Only {year}</button>
    <button type="button" class="link" onclick={() => pick(today)}>Today</button>
  </div>
</div>

<style>
  .date { display: flex; gap: 6px; }
  .date input { flex: 1; min-width: 0; }
  .open {
    flex: 0 0 auto; width: 42px; display: grid; place-items: center; cursor: pointer;
    border: 1px solid var(--c-line); border-radius: var(--r-control); background: var(--c-panel); color: var(--c-muted);
  }
  .open:hover { color: var(--c-accent); border-color: var(--c-accent); }
  .panel {
    position: fixed; inset: auto; margin: 0; padding: 12px;
    background: var(--c-panel); color: var(--c-ink); border: 1px solid var(--c-line); border-radius: var(--r-sm);
    box-shadow: 0 12px 32px rgb(0 0 0 / .18);
  }
  .head { display: flex; gap: 6px; align-items: center; margin-bottom: 8px; }
  .head select {
    flex: 1; min-width: 0; font: inherit; font-size: var(--text-small); padding: 4px 6px;
    border: 1px solid var(--c-line); border-radius: var(--r-control); background: var(--c-panel); color: var(--c-ink);
  }
  .nav { background: none; border: 0; color: var(--c-ink); cursor: pointer; padding: 4px; display: grid; place-items: center; border-radius: var(--r-control); }
  .nav:disabled { opacity: .3; cursor: default; }
  .grid { display: grid; grid-template-columns: repeat(7, 1fr); gap: 2px; text-align: center; }
  .dow { font-size: var(--text-caption); color: var(--c-muted); padding: 4px 0; }
  .day {
    font: inherit; font-size: var(--text-small); aspect-ratio: 1; border: 0; border-radius: var(--r-control);
    background: none; color: var(--c-ink); cursor: pointer;
  }
  .day:hover:not(:disabled) { background: var(--c-accent-soft); }
  .day.today { box-shadow: inset 0 0 0 1px var(--c-accent); }
  .day.chosen { background: var(--c-accent); color: var(--c-on-accent); }
  .day:disabled { opacity: .3; cursor: default; }
  .foot { display: flex; flex-wrap: wrap; gap: 6px 14px; margin-top: 10px; font-size: var(--text-small); }
</style>
