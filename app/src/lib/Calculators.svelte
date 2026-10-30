<script lang="ts">
  // Health calculators with public formulas. Each starts from the newest
  // records and shows its formula, its source and what it cannot tell you.
  import { Card, Pill } from '@swasthya/ui'
  import { api } from './api.ts'
  import { age, bmi, bmiBand, drink, egfr, egfrBand, known, packYears, UNITS_A_WEEK, whtr, whtrBand, CREATININE_UMOL, type Known } from './model/calc.ts'
  import { session } from './session.svelte.ts'

  let facts = $state<Known>({ dates: {} })
  let kg = $state('')
  let cm = $state('')
  let waist = $state('')
  let lower = $state(false)
  let scr = $state('')
  let scrUnit = $state<'mg/dL' | 'µmol/L'>('mg/dL')
  let years = $state('')
  let sex = $state<'' | 'female' | 'male'>('')
  let perDay = $state('')
  let smoked = $state('')
  let ml = $state('500')
  let abv = $state('4.5')
  let drinks = $state('')

  $effect(() => {
    void session.version
    const person = session.current
    if (!person) return
    api.records(person, 'Observation').then((rows) => {
      facts = known(rows)
      const s = (n?: number) => (n === undefined ? '' : String(n))
      kg = s(facts.kg)
      cm = s(facts.cm)
      waist = s(facts.waistCm)
      scr = s(facts.creatinine?.value)
      scrUnit = facts.creatinine?.unit ?? 'mg/dL'
    }, session.fail.bind(session))
  })
  $effect(() => {
    const p = session.person
    years = String(age(String(p?.birthDate ?? '')) ?? '')
    sex = p?.gender === 'female' || p?.gender === 'male' ? p.gender : ''
  })

  const num = (s: string) => (s.trim() === '' ? NaN : Number(s))
  const ok = (...xs: number[]) => xs.every((x) => Number.isFinite(x) && x > 0)
  const fixed = (n: number, p = 1) => n.toLocaleString(undefined, { maximumFractionDigits: p, minimumFractionDigits: p })
  const from = (d?: string) => (d ? ` (from ${d.slice(0, 10)})` : '')

  const b = $derived(ok(num(kg), num(cm)) ? bmi(num(kg), num(cm)) : null)
  const w = $derived(ok(num(waist), num(cm)) ? whtr(num(waist), num(cm)) : null)
  const adult = $derived(Number(years) >= 18)
  const g = $derived.by(() => {
    const s = num(scr) / (scrUnit === 'µmol/L' ? CREATININE_UMOL : 1)
    return ok(s) && adult && sex ? egfr(s, Number(years), sex) : null
  })
  const py = $derived(ok(num(perDay), num(smoked)) ? packYears(num(perDay), num(smoked)) : null)
  const one = $derived(ok(num(ml), num(abv)) && num(abv) <= 100 ? drink(num(ml), num(abv)) : null)
  const week = $derived(one && Number.isFinite(num(drinks)) && num(drinks) >= 0 ? one.units * num(drinks) : null)
</script>

<div class="grid">
  <Card title="Body mass index (BMI)" explain="Weight divided by height squared. A rough guide to whether weight is in a healthy range for an adult's height.">
    <div class="inputs">
      <label class="field"><span class="label">Weight (kg)</span><input inputmode="decimal" bind:value={kg} /></label>
      <label class="field"><span class="label">Height (cm)</span><input inputmode="decimal" bind:value={cm} /></label>
    </div>
    <p class="muted small">{facts.kg ? `Weight${from(facts.dates.kg)}` : 'No weight on Track yet'} · {facts.cm ? `height${from(facts.dates.cm)}` : 'no height on Track yet'}</p>
    <label class="check"><input type="checkbox" bind:checked={lower} /> South Asian, Chinese, other Asian, Middle Eastern, Black African or African-Caribbean family background</label>
    {#if b !== null}
      {@const band = bmiBand(b, lower)}
      <p class="out"><strong>{fixed(b)}</strong> kg/m² <Pill tone={band.tone}>{band.label}</Pill></p>
    {/if}
    <details>
      <summary>How it works</summary>
      <p>BMI = weight (kg) ÷ height (m)². Healthy range 18.5 to 24.9; above the range from 25; obesity from 30. With the box ticked, from 23 and 27.5, as NICE advises because health risks start at a lower BMI. Source: WHO; NICE. <a href="https://www.nhs.uk/health-assessment-tools/calculate-your-body-mass-index/" target="_blank" rel="noreferrer">NHS BMI calculator</a>.</p>
      <p>It cannot tell muscle from fat, and it is not for children, during pregnancy, or for some athletes and older people.</p>
    </details>
  </Card>

  <Card title="Waist to height" explain="Waist size divided by height. It shows fat around the middle, which carries more health risk than BMI shows.">
    <div class="inputs">
      <label class="field"><span class="label">Waist (cm)</span><input inputmode="decimal" bind:value={waist} /></label>
      <label class="field"><span class="label">Height (cm)</span><input inputmode="decimal" bind:value={cm} /></label>
    </div>
    <p class="muted small">{facts.waistCm ? `Waist${from(facts.dates.waistCm)}` : 'No waist measurement on Track yet'}. Measure halfway between the lowest rib and the top of the hip bone, after breathing out.</p>
    {#if w !== null}
      {@const band = whtrBand(w)}
      <p class="out"><strong>{fixed(w, 2)}</strong> <Pill tone={band.tone}>{band.label}</Pill></p>
    {/if}
    <details>
      <summary>How it works</summary>
      <p>Ratio = waist ÷ height, both in the same unit. 0.4 to 0.49: no increased risk; 0.5 to 0.59: increased; 0.6 or more: high. Source: NICE guidance on overweight and obesity (2022): keep your waist to less than half your height.</p>
      <p>For adults. It is not used during pregnancy, and it says nothing about fitness or diet.</p>
    </details>
  </Card>

  <Card title="Kidney function (eGFR)" explain="An estimate of how well the kidneys filter blood, from a creatinine blood test, age and sex.">
    <div class="inputs">
      <label class="field"><span class="label">Creatinine</span><input inputmode="decimal" bind:value={scr} /></label>
      <label class="field"><span class="label">Unit</span>
        <select bind:value={scrUnit}><option>mg/dL</option><option>µmol/L</option></select>
      </label>
      <label class="field"><span class="label">Age</span><input inputmode="numeric" bind:value={years} /></label>
      <label class="field"><span class="label">Sex</span>
        <select bind:value={sex}><option value="">Choose…</option><option value="female">Female</option><option value="male">Male</option></select>
      </label>
    </div>
    <p class="muted small">{facts.creatinine ? `Creatinine ${facts.creatinine.shown}${from(facts.creatinine.date)}` : 'No creatinine result yet: add one on Results, or type it here'}.</p>
    {#if years && !adult}
      <p class="muted">This formula is for adults (18 and over).</p>
    {:else if !sex}
      <p class="muted">Choose the sex used on lab reports to work it out: the formula differs.</p>
    {:else if g !== null}
      {@const band = egfrBand(g)}
      <p class="out"><strong>{Math.round(g)}</strong> mL/min/1.73 m² <Pill tone={band.tone}>{band.stage}: {band.label}</Pill></p>
    {/if}
    <details>
      <summary>How it works</summary>
      <p>CKD-EPI 2021, without a race term: 142 × min(Scr/κ, 1)<sup>α</sup> × max(Scr/κ, 1)<sup>−1.200</sup> × 0.9938<sup>age</sup> × 1.012 if female. κ is 0.7 (female) or 0.9 (male); α is −0.241 or −0.302. Creatinine in mg/dL (µmol/L ÷ {CREATININE_UMOL}). Categories G1 to G5: KDIGO. Source: <a href="https://www.kidney.org/professionals/gfr_calculator" target="_blank" rel="noreferrer">National Kidney Foundation</a>.</p>
      <p>One result is not a diagnosis: chronic kidney disease means a low eGFR for at least 3 months, or other signs of kidney damage. Muscle mass, a meat meal, some medicines and pregnancy change creatinine.</p>
    </details>
  </Card>

  <Card title="Pack-years" explain="How much someone has smoked over their life, used to decide who is offered lung cancer screening.">
    <div class="inputs">
      <label class="field"><span class="label">Cigarettes a day</span><input inputmode="decimal" bind:value={perDay} /></label>
      <label class="field"><span class="label">Years smoked</span><input inputmode="decimal" bind:value={smoked} /></label>
    </div>
    {#if py !== null}
      <p class="out"><strong>{fixed(py)}</strong> pack-years</p>
    {/if}
    <details>
      <summary>How it works</summary>
      <p>Pack-years = (cigarettes a day ÷ 20) × years smoked. For example, in the USA screening is offered from age 50 to 80 to people with 20 pack-years or more who smoke now or quit in the last 15 years. Source: <a href="https://www.uspreventiveservicestaskforce.org/uspstf/recommendation/lung-cancer-screening" target="_blank" rel="noreferrer">USPSTF (2021)</a>. Rules differ by country.</p>
      <p>It leaves out cigars, pipes, bidis and vaping, and it does not measure the harm already done. Stopping helps at any age.</p>
    </details>
  </Card>

  <Card title="Alcohol units" explain="How much pure alcohol is in a drink and in a usual week.">
    <div class="inputs">
      <label class="field"><span class="label">Drink size (mL)</span><input inputmode="decimal" bind:value={ml} /></label>
      <label class="field"><span class="label">Strength (% ABV)</span><input inputmode="decimal" bind:value={abv} /></label>
      <label class="field"><span class="label">Drinks a week</span><input inputmode="decimal" bind:value={drinks} /></label>
    </div>
    <p class="muted small">Common sizes: a pint 568 mL, a can 330 to 500 mL, a glass of wine 175 mL, a single spirit 25 to 30 mL.</p>
    {#if one}
      <p class="out"><strong>{fixed(one.units)}</strong> units, {fixed(one.grams)} g of alcohol per drink</p>
      {#if week !== null && drinks.trim()}
        <p class="out"><strong>{fixed(week)}</strong> units a week <Pill tone={week > UNITS_A_WEEK ? 'warn' : 'ok'}>{week > UNITS_A_WEEK ? `More than ${UNITS_A_WEEK}` : `${UNITS_A_WEEK} or fewer`}</Pill></p>
      {/if}
    {/if}
    <details>
      <summary>How it works</summary>
      <p>Units = mL × ABV ÷ 1000 (one UK unit is 10 mL of pure alcohol). Grams = mL × ABV ÷ 100 × 0.789. The UK advice is no more than 14 units a week, spread over 3 or more days; a US standard drink is 14 g. Source: <a href="https://www.nhs.uk/live-well/alcohol-advice/calculating-alcohol-units/" target="_blank" rel="noreferrer">NHS</a>.</p>
      <p>There is no completely safe amount. Advice differs by country, and it is lower during pregnancy, with some medicines and with some conditions.</p>
    </details>
  </Card>
</div>

<style>
  .grid { display: grid; gap: 14px; grid-template-columns: repeat(auto-fill, minmax(min(420px, 100%), 1fr)); align-items: start; }
  .inputs { display: flex; flex-wrap: wrap; gap: 10px 14px; margin-bottom: 6px; }
  .inputs .field { width: 10em; }
  .check { display: flex; gap: 8px; align-items: flex-start; font-size: var(--text-small); margin: 6px 0; }
  .check input { margin-top: 3px; }
  .out { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; margin: 10px 0 4px; }
  .out strong { font-size: var(--text-lead); font-variant-numeric: tabular-nums; }
  details { margin-top: 8px; font-size: var(--text-small); }
  summary { cursor: pointer; color: var(--c-accent); }
  details p { margin: 6px 0 0; }
  .small { font-size: var(--text-small); }
  .muted { color: var(--c-muted); }
</style>
