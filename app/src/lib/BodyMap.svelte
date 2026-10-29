<script module lang="ts">
  export interface Region { id: string; x: number; y: number; r: number; tone: 'danger' | 'warn' | 'accent' | 'ok' }
</script>

<script lang="ts">
  // Flat body map for the overview. The 3D viewer (phase 5) replaces it on the Body page.
  let { regions, label = 'Body map' }: { regions: Region[]; label?: string } = $props()
  const colour = { danger: 'var(--c-danger)', warn: 'var(--c-warn)', accent: 'var(--c-accent)', ok: 'var(--c-accent-2)' }
</script>

<svg viewBox="0 0 160 360" role="img" aria-label={label}>
  <defs>
    {#each Object.entries(colour) as [tone, c] (tone)}
      <radialGradient id="glow-{tone}">
        <stop offset="0" style="stop-color:{c};stop-opacity:.95" />
        <stop offset="1" style="stop-color:{c};stop-opacity:0" />
      </radialGradient>
    {/each}
  </defs>
  <g class="body">
    <ellipse cx="80" cy="30" rx="19" ry="23" />
    <path d="M70 50 h20 v10 h-20z" />
    <path d="M48 64 Q80 54 112 64 L118 120 Q114 160 106 176 L54 176 Q46 160 42 120 Z" />
    <path d="M48 66 L30 130 L24 190 L34 192 L42 136 L56 96Z" />
    <path d="M112 66 L130 130 L136 190 L126 192 L118 136 L104 96Z" />
    <path d="M54 174 L106 174 L104 214 L82 216 L78 216 L56 214Z" />
    <path d="M56 212 L78 214 L74 280 L72 344 L58 346 L56 280Z" />
    <path d="M104 212 L82 214 L86 280 L88 344 L102 346 L104 280Z" />
  </g>
  {#each regions as reg (reg.id)}
    <circle class="spot" cx={reg.x} cy={reg.y} r={reg.r} fill="url(#glow-{reg.tone})" />
  {/each}
</svg>

<style>
  svg { height: 100%; max-height: 330px; width: auto; flex: 0 0 auto; }
  .body { fill: var(--c-body-fill); stroke: var(--c-body-line); stroke-width: 1.4; }
  .spot { transform-box: fill-box; transform-origin: center; animation: breathe 3.2s var(--ease) infinite alternate; }
  @keyframes breathe { from { opacity: .75; scale: .92; } to { opacity: 1; scale: 1.06; } }
</style>
