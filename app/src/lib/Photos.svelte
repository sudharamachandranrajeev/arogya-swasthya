<script lang="ts">
  // Body photo series: one part of the body photographed over time. A new
  // photo can be taken with the last one shown faintly over the camera, so
  // the angle and distance match; checks then point out blur and light.
  // Photos are encrypted like every document and never leave the device.
  import { onDestroy } from 'svelte'
  import { Button, Card, Pill, Segmented } from '@swasthya/ui'
  import { api } from './api.ts'
  import { attachment, type Resource } from './model/fhir.ts'
  import { lists } from './model/lists.ts'
  import { hints, measure, photoDoc, series, siteOf, takenOf, type Hint, type Measure } from './model/photo.ts'
  import { session } from './session.svelte.ts'

  let rows = $state<Resource[]>([])
  $effect(() => {
    void session.version
    const person = session.current
    if (!person) return
    api.records(person, 'DocumentReference').then((r) => (rows = r), session.fail.bind(session))
  })
  const all = $derived(series(rows))

  // ---- photo bytes, decrypted on demand and kept as object URLs ----
  const urls = new Map<string, Promise<string>>()
  function urlOf(r: Resource): Promise<string> {
    const a = attachment(r)
    if (!a) return Promise.reject('No file')
    if (!urls.has(a.id)) urls.set(a.id, api.documentBytes(a.id).then((b) => URL.createObjectURL(new Blob([b], { type: a.type }))))
    return urls.get(a.id)!
  }
  onDestroy(() => {
    for (const u of urls.values()) u.then(URL.revokeObjectURL, () => {})
    stopCamera()
    if (taken) URL.revokeObjectURL(taken.url)
  })

  const day = new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'short', year: 'numeric' })
  const at = new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'short', year: 'numeric', hour: 'numeric', minute: '2-digit' })
  /** The date, with the time when another photo of the series was taken the same day. */
  const when = (r: Resource) => {
    const t = takenOf(r)
    if (!t) return ''
    const same = all.some((s) => s.photos.some((p) => p !== r && takenOf(p).slice(0, 10) === t.slice(0, 10) && siteOf(p) === siteOf(r)))
    return (same ? at : day).format(new Date(t))
  }

  // ---- a new photo ----
  let adding = $state(false)
  let site = $state('')
  let note = $state('')
  let lastOfSite = $derived(all.find((s) => s.site.toLowerCase() === site.trim().toLowerCase())?.photos.at(-1))
  let ghost = $state<string | null>(null)
  let ghostLevel = $state(35)
  let lastMeasure: Measure | undefined
  $effect(() => {
    ghost = null
    lastMeasure = undefined
    const last = lastOfSite
    if (!last) return
    urlOf(last).then(async (u) => {
      ghost = u
      lastMeasure = measure(small(await loadImage(u)))
    }, () => {})
  })

  let video = $state<HTMLVideoElement>()
  let stream: MediaStream | null = null
  let facing = $state<'environment' | 'user'>('environment')
  let cameraOn = $state(false)
  const canCamera = typeof navigator !== 'undefined' && !!navigator.mediaDevices?.getUserMedia

  async function startCamera() {
    stopCamera()
    try {
      stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: facing, width: { ideal: 1920 } }, audio: false })
      cameraOn = true
      await Promise.resolve()
      if (video) {
        video.srcObject = stream
        await video.play()
      }
    } catch (e) {
      cameraOn = false
      session.fail(e instanceof DOMException && e.name === 'NotAllowedError' ? 'The camera was not allowed. You can choose a photo instead.' : 'The camera could not start. You can choose a photo instead.')
    }
  }
  function stopCamera() {
    stream?.getTracks().forEach((t) => t.stop())
    stream = null
    cameraOn = false
  }

  /** Scaled down so the longest side is at most `max`. */
  function draw(src: CanvasImageSource, w: number, h: number, max: number) {
    const k = Math.min(1, max / Math.max(w, h))
    const c = window.document.createElement('canvas')
    c.width = Math.round(w * k)
    c.height = Math.round(h * k)
    c.getContext('2d')!.drawImage(src, 0, 0, c.width, c.height)
    return c
  }
  const small = (img: HTMLImageElement | HTMLCanvasElement) => {
    const c = draw(img, img.width, img.height, 256)
    return c.getContext('2d')!.getImageData(0, 0, c.width, c.height)
  }
  const loadImage = (url: string) =>
    new Promise<HTMLImageElement>((ok, bad) => {
      const i = new Image()
      i.onload = () => ok(i)
      i.onerror = bad
      i.src = url
    })

  let taken = $state<{ url: string; blob: Blob; hints: Hint[] } | null>(null)

  /** Keep a 2048 px JPEG (which also drops the camera's location and other details) and check it. */
  async function prepare(src: CanvasImageSource, w: number, h: number) {
    const c = draw(src, w, h, 2048)
    const blob = await new Promise<Blob | null>((ok) => c.toBlob(ok, 'image/jpeg', 0.9))
    if (!blob) return session.fail('That photo could not be prepared.')
    if (taken) URL.revokeObjectURL(taken.url)
    taken = { url: URL.createObjectURL(blob), blob, hints: hints(measure(small(c)), lastMeasure) }
  }

  async function snap() {
    if (!video?.videoWidth) return
    await prepare(video, video.videoWidth, video.videoHeight)
    stopCamera()
  }

  async function chosen(e: Event) {
    const input = e.currentTarget as HTMLInputElement
    const f = input.files?.[0]
    input.value = ''
    if (!f) return
    const u = URL.createObjectURL(f)
    try {
      const img = await loadImage(u)
      await prepare(img, img.naturalWidth, img.naturalHeight)
    } catch {
      session.fail('That file is not a photo this app can read.')
    } finally {
      URL.revokeObjectURL(u)
    }
  }

  let busy = $state(false)
  async function keep() {
    if (!taken || !site.trim()) return
    busy = true
    try {
      await api.addPhoto(photoDoc(session.current, site, note, new Date()), await taken.blob.arrayBuffer())
      URL.revokeObjectURL(taken.url)
      taken = null
      adding = false
      const s = site
      note = ''
      await session.changed(`Added a photo to ${s}.`)
      open = s
    } catch (e) {
      session.fail(e)
    } finally {
      busy = false
    }
  }

  function retake() {
    if (taken) URL.revokeObjectURL(taken.url)
    taken = null
  }

  function cancel() {
    retake()
    stopCamera()
    adding = false
  }

  // ---- one series ----
  let open = $state<string | null>(null)
  const shown = $derived(all.find((s) => s.site === open))
  let a = $state(0)
  let b = $state(0)
  let mode = $state<'side' | 'slide'>('side')
  let split = $state(50)
  $effect(() => {
    const n = shown?.photos.length ?? 0
    a = 0
    b = Math.max(0, n - 1)
  })
  const daysApart = $derived(
    shown && shown.photos[a] && shown.photos[b]
      ? Math.round(Math.abs(new Date(takenOf(shown.photos[b])).getTime() - new Date(takenOf(shown.photos[a])).getTime()) / 86_400_000)
      : 0,
  )

  async function remove(r: Resource) {
    try {
      await api.remove(r.id!)
      await session.changed('Moved the photo to the trash.')
    } catch (e) {
      session.fail(e)
    }
  }

  function startNew(s = '') {
    site = s
    note = ''
    adding = true
    open = null
  }
</script>

{#if adding}
  <Card title="New photo">
    {#if !taken}
      <div class="form">
        <label class="field">
          <span class="label">Part of the body</span>
          <input list="photo-sites" bind:value={site} placeholder="For example: left forearm, mole near elbow" />
          <datalist id="photo-sites">
            {#each all as s (s.site)}<option value={s.site}></option>{/each}
            {#each lists.bodySites as s (s)}<option value={s}></option>{/each}
          </datalist>
        </label>
        <label class="field">
          <span class="label">Note (optional)</span>
          <input bind:value={note} placeholder="Anything worth remembering about it" />
        </label>
      </div>
      <p class="muted small">
        {#if lastOfSite}The last photo of this part ({when(lastOfSite)}) shows faintly over the camera: line it up, use the same light and distance.{:else}Tip: use daylight, the same distance each time, and a plain background. A ruler or coin beside it shows size.{/if}
      </p>

      {#if cameraOn}
        <div class="camera">
          <!-- svelte-ignore a11y_media_has_caption -->
          <video bind:this={video} playsinline muted></video>
          {#if ghost}<img class="ghost" src={ghost} alt="" style:opacity={ghostLevel / 100} />{/if}
        </div>
        {#if ghost}
          <label class="level">Last photo <input type="range" min="0" max="80" bind:value={ghostLevel} aria-label="How strongly the last photo shows" /></label>
        {/if}
        <div class="actions">
          <Button onclick={snap}>Take photo</Button>
          <Button variant="quiet" onclick={() => { facing = facing === 'environment' ? 'user' : 'environment'; startCamera() }}>Switch camera</Button>
          <Button variant="ghost" onclick={cancel}>Cancel</Button>
        </div>
      {:else}
        <div class="actions">
          {#if canCamera}<Button disabled={!site.trim()} onclick={startCamera}>Use the camera</Button>{/if}
          <label class="file" class:off={!site.trim()}>
            Choose a photo
            <input type="file" accept="image/*" disabled={!site.trim()} onchange={chosen} />
          </label>
          <Button variant="ghost" onclick={cancel}>Cancel</Button>
        </div>
      {/if}
    {:else}
      <div class="check">
        <img src={taken.url} alt="Just taken" />
        <ul class="hints">
          {#each taken.hints as h (h.text)}<li><Pill tone={h.tone}>{h.tone === 'ok' ? 'Looks good' : 'Check'}</Pill> {h.text}</li>{/each}
        </ul>
      </div>
      <div class="actions">
        <Button disabled={busy} onclick={keep}>Save to {site.trim() || 'the series'}</Button>
        <Button variant="quiet" onclick={retake}>Take another</Button>
        <Button variant="ghost" onclick={cancel}>Cancel</Button>
      </div>
    {/if}
  </Card>
{:else if shown}
  <Card title={shown.site}>
    {#snippet action()}<button class="link" onclick={() => (open = null)}>All series</button>{/snippet}
    {#if shown.photos.length > 1}
      <div class="pick">
        <label>From <select bind:value={a}>{#each shown.photos as r, i (r.id)}<option value={i}>{when(r)}</option>{/each}</select></label>
        <label>To <select bind:value={b}>{#each shown.photos as r, i (r.id)}<option value={i}>{when(r)}</option>{/each}</select></label>
        <Segmented label="How to compare" options={[{ value: 'side', label: 'Side by side' }, { value: 'slide', label: 'Slider' }]} value={mode} onchange={(v) => (mode = v)} />
      </div>
      <p class="muted small">{daysApart} days apart. If something has changed, show a clinician: the app does not judge what a photo shows.</p>
      {#await Promise.all([urlOf(shown.photos[a]), urlOf(shown.photos[b])]) then [ua, ub]}
        {#if mode === 'side'}
          <div class="pair">
            <figure><img src={ua} alt="{shown.site}, {when(shown.photos[a])}" /><figcaption>{when(shown.photos[a])}</figcaption></figure>
            <figure><img src={ub} alt="{shown.site}, {when(shown.photos[b])}" /><figcaption>{when(shown.photos[b])}</figcaption></figure>
          </div>
        {:else}
          <div class="slide">
            <img src={ua} alt="{shown.site}, {when(shown.photos[a])}" />
            <img class="top" src={ub} alt="{shown.site}, {when(shown.photos[b])}" style:clip-path="inset(0 0 0 {split}%)" />
            <span class="tag left">{when(shown.photos[a])}</span><span class="tag right">{when(shown.photos[b])}</span>
          </div>
          <input class="splitter" type="range" min="0" max="100" bind:value={split} aria-label="Move between the two photos" />
        {/if}
      {:catch}
        <p class="muted">These photos could not be opened.</p>
      {/await}
    {/if}
    <ul class="grid">
      {#each [...shown.photos].reverse() as r (r.id)}
        <li>
          {#await urlOf(r)}<div class="thumb"></div>{:then u}<img class="thumb" src={u} alt="{shown.site}, {when(r)}" />{:catch}<div class="thumb"></div>{/await}
          <span class="small">{when(r)}</span>
          {#if r.description && r.description !== `${shown.site} photo`}<span class="muted small">{r.description}</span>{/if}
          <button class="link small" onclick={() => remove(r)}>Delete</button>
        </li>
      {/each}
    </ul>
    <div class="actions"><Button onclick={() => startNew(shown.site)}>Add a photo to this series</Button></div>
  </Card>
{:else}
  <div class="actions top"><Button onclick={() => startNew()}>New photo</Button></div>
  {#if all.length}
    <div class="series">
      {#each all as s (s.site)}
        <button class="one" onclick={() => (open = s.site)}>
          {#await urlOf(s.photos[s.photos.length - 1])}<div class="thumb"></div>{:then u}<img class="thumb" src={u} alt="" />{:catch}<div class="thumb"></div>{/await}
          <strong>{s.site}</strong>
          <span class="muted small">{s.photos.length} {s.photos.length === 1 ? 'photo' : 'photos'} · last {when(s.photos[s.photos.length - 1])}</span>
        </button>
      {/each}
    </div>
  {:else}
    <Card>
      <p>No body photos yet. A series of photos of one place, such as a mole, a rash or a scar, shows how it changes over time.</p>
      <p class="muted small">Photos are encrypted on this device. They are kept apart from other documents here, and never sent anywhere.</p>
    </Card>
  {/if}
{/if}

<style>
  .form { display: grid; gap: 12px; grid-template-columns: repeat(auto-fit, minmax(min(240px, 100%), 1fr)); margin-bottom: 8px; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin-top: 12px; }
  .actions.top { margin: 0 0 16px; }
  .file { display: inline-flex; align-items: center; padding: 8px 14px; border-radius: var(--r-control); background: var(--c-accent-soft); color: var(--c-accent); font-weight: 600; cursor: pointer; }
  .file input { display: none; }
  .file.off { opacity: 0.5; cursor: default; }
  .camera { position: relative; max-width: 640px; border-radius: var(--r-card); overflow: hidden; background: #000; }
  .camera video { display: block; width: 100%; }
  .ghost { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; pointer-events: none; }
  .level { display: flex; gap: 8px; align-items: center; margin-top: 8px; font-size: var(--text-small); }
  .check { display: grid; gap: 14px; grid-template-columns: minmax(0, 320px) 1fr; align-items: start; }
  @media (max-width: 640px) { .check { grid-template-columns: 1fr; } }
  .check img { width: 100%; border-radius: var(--r-control); }
  .hints { list-style: none; padding: 0; margin: 0; display: grid; gap: 8px; }
  .pick { display: flex; flex-wrap: wrap; gap: 12px; align-items: center; margin-bottom: 6px; }
  .pick select { font: inherit; padding: 4px 6px; border-radius: var(--r-control); border: 1px solid var(--c-line); background: var(--c-panel); color: var(--c-ink); }
  .pair { display: grid; gap: 10px; grid-template-columns: 1fr 1fr; }
  figure { margin: 0; }
  figure img, .slide img { width: 100%; border-radius: var(--r-control); display: block; }
  figcaption { font-size: var(--text-small); color: var(--c-muted); margin-top: 4px; }
  .slide { position: relative; max-width: 640px; }
  .slide .top { position: absolute; inset: 0; height: 100%; object-fit: cover; }
  .tag { position: absolute; top: 8px; font-size: var(--text-small); background: var(--c-panel); color: var(--c-ink); padding: 2px 8px; border-radius: 999px; }
  .tag.left { left: 8px; }
  .tag.right { right: 8px; }
  .splitter { width: 100%; max-width: 640px; accent-color: var(--c-accent); }
  .grid { list-style: none; padding: 0; margin: 16px 0 0; display: grid; gap: 12px; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); }
  .grid li { display: grid; gap: 2px; }
  .thumb { width: 100%; aspect-ratio: 4 / 3; object-fit: cover; border-radius: var(--r-control); background: var(--c-panel-2); display: block; }
  .series { display: grid; gap: 14px; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); }
  .one { display: grid; gap: 4px; text-align: left; font: inherit; color: inherit; background: var(--c-panel); border: 1px solid var(--c-line); border-radius: var(--r-card); padding: 10px; cursor: pointer; }
  .one:hover { background: var(--c-panel-2); }
  .one:focus-visible, .link:focus-visible { outline: 2px solid var(--c-focus); outline-offset: 2px; }
  .muted { color: var(--c-muted); }
  .small { font-size: var(--text-small); }
  .link { font: inherit; color: var(--c-accent); background: none; border: 0; cursor: pointer; padding: 0; justify-self: start; }
</style>
