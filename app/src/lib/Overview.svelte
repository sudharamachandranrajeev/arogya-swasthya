<script lang="ts">
  // Overview built only from framework parts. Synthetic demo data until the record store lands.
  import { Card, Pill, Row, Stat } from '@swasthya/ui'
  import BodyMap, { type Region } from './BodyMap.svelte'

  const regions: Region[] = [
    { id: 'thyroid', x: 80, y: 56, r: 13, tone: 'accent' },
    { id: 'pancreas', x: 90, y: 128, r: 16, tone: 'warn' },
    { id: 'lumbar', x: 80, y: 168, r: 18, tone: 'danger' },
    { id: 'knee', x: 95, y: 282, r: 13, tone: 'ok' },
    { id: 'forearm', x: 34, y: 160, r: 9, tone: 'accent' },
  ]
  const legend = [
    ['danger', 'Lower back, L4–L5', 'Recurring · pain 4/10 this week'],
    ['warn', 'Type 2 diabetes', 'Active · HbA1c improving'],
    ['accent', 'Thyroid', 'Hypothyroidism · stable on treatment'],
    ['accent-2', 'Right knee', 'ACL repair 2019 · resolved'],
    ['accent', 'Left forearm patch', 'Photo series · week 6, no measured change'],
  ]
</script>

<header class="top">
  <div>
    <h1>Good evening, Asha</h1>
    <p class="sub">Demo data · 2 of 3 tasks done today</p>
  </div>
  <div class="ask">Search your record, or ask a question <kbd>Ctrl K</kbd></div>
</header>

<div class="grid">
  <Card title="Your body" explain="Each glow is something in your record that belongs to a part of the body. Red needs attention, amber is worth a look, green is settled. Open 3D to turn the body and see details." class="body">
    {#snippet action()}<a href="#/body">Open 3D</a>{/snippet}
    <div class="figure">
      <BodyMap {regions} />
      <ul class="legend">
        {#each legend as [tone, name, note] (name)}
          <li><i style="background: var(--c-{tone})"></i><span>{name}<small>{note}</small></span></li>
        {/each}
      </ul>
    </div>
  </Card>

  <Card title="Today" explain="Medicines, exercise and photo check-ins planned for today. Tap a label to mark it done.">
    <div class="list">
      <Row lead="07:00" title="Levothyroxine 75 mcg" sub="Before breakfast">{#snippet trailing()}<Pill tone="done">Taken</Pill>{/snippet}</Row>
      <Row lead="08:00" title="Metformin 500 mg" sub="With food">{#snippet trailing()}<Pill tone="done">Taken</Pill>{/snippet}</Row>
      <Row lead="18:30" title="Knee-safe strength, 20 min" sub="Avoids loaded spinal flexion">{#snippet trailing()}<Pill onclick={() => {}}>Start</Pill>{/snippet}</Row>
      <Row lead="Sun" title="Forearm photo, week 7" sub="Same light and distance as last time">{#snippet trailing()}<Pill tone="danger">Due</Pill>{/snippet}</Row>
    </div>
  </Card>

  <Card title="Latest results" explain="Your most recent test results. The shaded band is the usual range for that test.">
    {#snippet action()}<a href="#/results">All results</a>{/snippet}
    <div class="stats">
      <Stat label="HbA1c" value="52" unit="mmol/mol" note="↓ 6" tone="ok" trend={[58, 57, 55, 52]} range={[20, 48]} explain="Average blood sugar over the last 2–3 months. Lower is better for diabetes; under 48 is the usual target." />
      <Stat label="Home BP" value="132/84" unit="mmHg" note="7-day avg" tone="warn" trend={[134, 131, 136, 130, 133, 132]} explain="Blood pressure measured at home. Under 135/85 at home is the usual aim." />
      <Stat label="eGFR" value="84" unit="mL/min" note="normal" tone="ok" trend={[86, 85, 84, 84]} explain="How well your kidneys filter blood. Above 60 is usually normal." />
      <Stat label="TSH" value="2.1" unit="mU/L" note="in range" tone="ok" trend={[3.2, 2.6, 2.3, 2.1]} explain="Thyroid-stimulating hormone. Shows whether your thyroid dose is right." />
    </div>
  </Card>

  <Card title="Find care: lower back" explain="Options near you, checked against several official sources. Each fact shows where it came from.">
    {#snippet action()}<a href="#/care">See all 9</a>{/snippet}
    <div class="opts">
      <div class="opt"><span>NHS MSK physio, self-referral</span><b>4–6 wk</b><small>2.1 mi · registered · rated Good</small></div>
      <div class="opt"><span>Private physio, back specialist</span><b>2 days</b><small>0.8 mi · registration checked · £55</small></div>
      <div class="opt"><span>Community pain clinic</span><b>GP referral</b><small>6.4 mi · <em>specialty differs between 2 sources</em></small></div>
    </div>
  </Card>

  <Card title="Coming up" explain="Checks and tests due, worked out from your age, conditions and past results.">
    <div class="list">
      <Row lead="Dec" title="Diabetic eye screening">{#snippet trailing()}<Pill onclick={() => {}}>Book</Pill>{/snippet}</Row>
      <Row lead="Jan" title="HbA1c and kidney tests">{#snippet trailing()}<Pill onclick={() => {}}>Remind</Pill>{/snippet}</Row>
      <Row lead="Oct" title="Flu vaccine recorded" sub="Next due next autumn" />
    </div>
  </Card>
</div>

<style>
  .top { display: flex; align-items: center; gap: 18px; margin-bottom: 22px; flex-wrap: wrap; }
  h1 { font-size: var(--text-display); font-weight: var(--weight-display); }
  .sub { color: var(--c-muted); margin-top: 2px; }
  .ask {
    margin-left: auto; min-width: min(420px, 100%); display: flex; align-items: center; gap: 10px; padding: 11px 16px;
    border-radius: var(--r-control); background: var(--card-bg); border: var(--card-border); box-shadow: var(--card-shadow); color: var(--c-muted);
  }
  kbd { margin-left: auto; font-family: var(--font-number); font-size: 11px; border: 1px solid var(--c-line); padding: 2px 6px; border-radius: 6px; }
  .grid { display: grid; grid-template-columns: minmax(340px, 400px) 1fr 1fr; gap: 18px; }
  .grid :global(.body) { grid-row: span 2; }
  @media (max-width: 1200px) { .grid { grid-template-columns: 1fr 1fr; } .grid :global(.body) { grid-row: auto; grid-column: span 2; } }
  @media (max-width: 860px) { .grid { grid-template-columns: 1fr; } .grid :global(.body) { grid-column: auto; } .ask { display: none; } }
  .figure { display: flex; gap: 14px; align-items: center; }
  .legend { list-style: none; display: flex; flex-direction: column; gap: 10px; font-size: var(--text-small); min-width: 0; }
  .legend li { display: flex; gap: 9px; line-height: 1.3; }
  .legend i { width: 10px; height: 10px; border-radius: 50%; margin-top: 4px; flex: 0 0 auto; }
  .legend small { display: block; color: var(--c-muted); font-size: 11.5px; }
  .list { display: flex; flex-direction: column; gap: 8px; }
  .stats { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .opt { display: grid; grid-template-columns: 1fr auto; gap: 2px 10px; padding: 9px 0; border-top: 1px solid var(--c-line); }
  .opt:first-child { border-top: 0; }
  .opt b { font-family: var(--font-number); font-weight: 500; font-size: var(--text-small); }
  .opt small { color: var(--c-muted); font-size: var(--text-caption); }
  em { color: var(--c-warn); font-style: normal; font-weight: 600; }
  a { text-decoration: none; }
</style>
