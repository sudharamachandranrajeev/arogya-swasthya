<script lang="ts">
  // The 3D body. Regions with records glow in their status colour; when any
  // do, the rest fade so a lit organ shows through the ribs. Drag to turn,
  // pinch or scroll to zoom, tap a part to see what is recorded there.
  // Draws only when something changes, so it costs nothing while still.
  import { onMount } from 'svelte'
  import * as THREE from 'three'
  import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js'
  import { OrbitControls } from 'three/addons/controls/OrbitControls.js'
  import { MeshoptDecoder } from 'three/addons/libs/meshopt_decoder.module.js'
  import { labelOf, MARKERS, REGIONS, type Tone } from './model/body.ts'

  let {
    lit,
    selected = null,
    hidden = [],
    skin = true,
    onselect,
  }: {
    lit: Map<string, { tone: Tone }>
    selected?: string | null
    /** Regions not to draw (the prostate for a woman, say). */
    hidden?: string[]
    skin?: boolean
    onselect: (id: string | null) => void
  } = $props()

  let host: HTMLDivElement
  let failed = $state('')
  let loading = $state(true)
  let draw = () => {}
  let view = (_side: 'front' | 'back' | 'left' | 'right') => {}

  const css = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim() || '#888'
  const TONE_VAR: Record<Tone, string> = { danger: '--c-danger', warn: '--c-warn', ok: '--c-ok', accent: '--c-accent' }

  const meshes = new Map<string, THREE.Mesh<THREE.BufferGeometry, THREE.MeshStandardMaterial>>()
  const boxes = new Map<string, THREE.Box3>()
  const markers = new Map<string, THREE.Mesh<THREE.SphereGeometry, THREE.MeshStandardMaterial>>()

  /** Colours and see-through from the current state and theme. */
  function paint() {
    const any = lit.size > 0
    const base = { skin: css('--c-body-fill'), bone: css('--c-panel-2'), organ: css('--c-body-line') }
    for (const [id, m] of meshes) {
      const kind = REGIONS[id]?.kind ?? 'organ'
      const hit = lit.get(id)
      const mat = m.material
      m.visible = !hidden.includes(id) && (kind !== 'skin' || skin || !!hit)
      if (hit) {
        const c = css(TONE_VAR[hit.tone])
        mat.color.set(c)
        mat.emissive.set(c)
        mat.emissiveIntensity = id === selected ? 0.75 : 0.35
        // A chosen part shows through the other lit ones.
        mat.opacity = kind === 'skin' ? 0.55 : selected && id !== selected ? 0.4 : 1
      } else {
        mat.color.set(base[kind])
        mat.emissive.set('#000000')
        mat.emissiveIntensity = 0
        mat.opacity = kind === 'skin' ? (any ? 0.07 : 0.16) : any ? 0.28 : 1
      }
      mat.transparent = mat.opacity < 1
      mat.depthWrite = !mat.transparent
      m.renderOrder = kind === 'skin' ? 2 : hit ? 0 : 1
    }
    for (const [id, s] of markers) {
      const hit = lit.get(id)
      s.visible = !!hit && !hidden.includes(id)
      if (hit) {
        const c = css(TONE_VAR[hit.tone])
        s.material.color.set(c)
        s.material.emissive.set(c)
        s.material.emissiveIntensity = id === selected ? 0.8 : 0.4
      }
    }
    draw()
  }

  $effect(() => {
    void lit
    void selected
    void hidden
    void skin
    paint()
  })

  onMount(() => {
    let renderer: THREE.WebGLRenderer
    try {
      renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true })
    } catch {
      failed = 'This device cannot show 3D here. The list beside it shows the same records.'
      loading = false
      return
    }
    renderer.setPixelRatio(Math.min(devicePixelRatio, 2))
    host.append(renderer.domElement)
    const scene = new THREE.Scene()
    const camera = new THREE.PerspectiveCamera(30, 1, 0.05, 20)
    scene.add(new THREE.HemisphereLight('#ffffff', '#8a9a92', 2.2))
    const sun = new THREE.DirectionalLight('#ffffff', 1.4)
    sun.position.set(1, 2, 3)
    scene.add(sun)
    const back = new THREE.DirectionalLight('#ffffff', 0.6)
    back.position.set(-1, 1, -3)
    scene.add(back)

    const controls = new OrbitControls(camera, renderer.domElement)
    controls.enablePan = false
    controls.minDistance = 0.6
    controls.maxDistance = 6
    let frame = 0
    draw = () => {
      if (frame) return
      frame = requestAnimationFrame(() => {
        frame = 0
        renderer.render(scene, camera)
      })
    }
    controls.addEventListener('change', draw)

    const target = new THREE.Vector3(0, 0.9, 0)
    view = (side) => {
      const d = camera.position.distanceTo(controls.target)
      const dir = { front: [0, 0, 1], back: [0, 0, -1], left: [1, 0, 0], right: [-1, 0, 0] }[side]
      camera.position.set(controls.target.x + dir[0] * d, controls.target.y, controls.target.z + dir[2] * d)
      controls.update()
    }

    const resize = () => {
      const w = host.clientWidth
      const h = host.clientHeight
      renderer.setSize(w, h)
      camera.aspect = w / h
      camera.updateProjectionMatrix()
      draw()
    }
    const ro = new ResizeObserver(resize)
    ro.observe(host)

    // Theme changes repaint the colours.
    const mo = new MutationObserver(() => paint())
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'data-mode', 'style'] })

    // A tap, not a drag, picks the part under the finger. Skin only when nothing else is there.
    const ray = new THREE.Raycaster()
    let down: [number, number] | null = null
    const el = renderer.domElement
    const onDown = (e: PointerEvent) => (down = [e.clientX, e.clientY])
    const onUp = (e: PointerEvent) => {
      if (!down || Math.hypot(e.clientX - down[0], e.clientY - down[1]) > 6) return
      const r = el.getBoundingClientRect()
      ray.setFromCamera(new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1), camera)
      const pickable = [...meshes.values(), ...markers.values()].filter((m) => m.visible)
      const hits = ray.intersectObjects(pickable, false)
      const best = hits.find((h) => h.object.name !== 'skin' || lit.has('skin')) ?? hits.find((h) => h.object.name !== 'skin') ?? hits[0]
      onselect(best ? best.object.name : null)
    }
    el.addEventListener('pointerdown', onDown)
    el.addEventListener('pointerup', onUp)

    new GLTFLoader()
      .setMeshoptDecoder(MeshoptDecoder)
      .loadAsync(`${import.meta.env.BASE_URL}anatomy/body.glb`)
      .then((gltf) => {
        gltf.scene.traverse((o) => {
          if (!(o instanceof THREE.Mesh)) return
          o.geometry.computeVertexNormals()
          o.material = new THREE.MeshStandardMaterial({ roughness: 0.75, metalness: 0 })
          meshes.set(o.name, o)
        })
        scene.add(gltf.scene)
        gltf.scene.updateMatrixWorld(true)
        for (const [id, m] of meshes) boxes.set(id, new THREE.Box3().setFromObject(m))
        for (const [id, mk] of Object.entries(MARKERS)) {
          const [anchor, fx, fy, fz] = mk.at
          const b = boxes.get(anchor)
          if (!b) continue
          const s = new THREE.Mesh(new THREE.SphereGeometry(0.022, 20, 14), new THREE.MeshStandardMaterial({ roughness: 0.5 }))
          s.name = id
          s.renderOrder = 0
          s.position.set(b.min.x + fx * (b.max.x - b.min.x), b.min.y + fy * (b.max.y - b.min.y), b.min.z + fz * (b.max.z - b.min.z))
          scene.add(s)
          markers.set(id, s)
        }
        const all = new THREE.Box3().setFromObject(gltf.scene)
        all.getCenter(target)
        controls.target.copy(target)
        // Far enough for the whole height, with a little room.
        const height = all.max.y - all.min.y
        camera.position.set(target.x, target.y, target.z + (height * 0.56) / Math.tan(THREE.MathUtils.degToRad(camera.fov / 2)))
        controls.update()
        loading = false
        paint()
      })
      .catch(() => {
        failed = 'The body model could not be loaded. The list beside it shows the same records.'
        loading = false
      })

    return () => {
      ro.disconnect()
      mo.disconnect()
      el.removeEventListener('pointerdown', onDown)
      el.removeEventListener('pointerup', onUp)
      controls.dispose()
      for (const m of [...meshes.values(), ...markers.values()]) {
        m.geometry.dispose()
        m.material.dispose()
      }
      renderer.dispose()
      draw = () => {}
    }
  })
</script>

<div class="wrap">
  <div class="canvas" bind:this={host} role="img" aria-label={lit.size ? `Body with ${[...lit.keys()].map(labelOf).join(', ')} marked` : 'Body, nothing marked'}></div>
  {#if loading}<p class="note">Loading the body…</p>{/if}
  {#if failed}<p class="note">{failed}</p>{/if}
  {#if !failed}
    <div class="views" role="group" aria-label="Turn the body">
      <button onclick={() => view('front')}>Front</button>
      <button onclick={() => view('back')}>Back</button>
      <button onclick={() => view('left')}>Left side</button>
      <button onclick={() => view('right')}>Right side</button>
    </div>
  {/if}
</div>

<style>
  .wrap { position: relative; }
  .canvas { height: min(68vh, 620px); min-height: 360px; touch-action: none; border-radius: var(--r-card); background: radial-gradient(ellipse at 50% 40%, var(--c-panel), transparent 70%); }
  .canvas :global(canvas) { display: block; cursor: grab; }
  .note { position: absolute; inset: 40% 0 auto; text-align: center; color: var(--c-muted); padding: 0 16px; }
  .views { display: flex; flex-wrap: wrap; gap: 6px; justify-content: center; margin-top: 8px; }
  .views button { font: inherit; font-size: var(--text-small); padding: 4px 10px; border-radius: 999px; border: 1px solid var(--c-line); background: var(--c-panel); color: var(--c-ink); cursor: pointer; }
  .views button:focus-visible { outline: 2px solid var(--c-focus); outline-offset: 2px; }
</style>
