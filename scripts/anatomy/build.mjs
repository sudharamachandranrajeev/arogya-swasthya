// Builds app/public/anatomy/body.glb from BodyParts3D 4.0 (the PART-OF set).
//
//   1. Download and unzip into one folder (about 210 MB unpacked):
//        https://dbarchive.biosciencedbc.jp/data/bodyparts3d/LATEST/partof_BP3D_4.0_obj_99.zip
//        https://dbarchive.biosciencedbc.jp/data/bodyparts3d/LATEST/partof_element_parts.txt
//   2. node scripts/anatomy/build.mjs <that folder>
//
// Each region in app/src/lib/model/anatomy.json becomes one named node: every part file that makes up its FMA
// concept, merged, simplified and compressed with meshoptimizer. Positions are in metres, Y up,
// the body facing +Z, feet at Y = 0.
//
// BodyParts3D has no surface for lung tissue, only the airways and vessels inside it. Regions
// marked "hull" are drawn as the convex hull of their parts, which follows the lung's outline.
import fs from 'node:fs'
import path from 'node:path'
import { Document, NodeIO } from '@gltf-transform/core'
import { EXTMeshoptCompression, KHRMeshQuantization } from '@gltf-transform/extensions'
import { meshopt, quantize, reorder, simplify, weld } from '@gltf-transform/functions'
import { MeshoptEncoder, MeshoptSimplifier } from 'meshoptimizer'
import { Vector3 } from 'three'
import { ConvexGeometry } from 'three/addons/geometries/ConvexGeometry.js'

const here = path.dirname(new URL(import.meta.url).pathname)
const OUT = path.join(here, '../../app/public/anatomy/body.glb')
/** Largest simplification error, as a share of each region's size. */
const ERROR = 0.01

const src = process.argv[2]
if (!src) {
  console.error('usage: node scripts/anatomy/build.mjs <folder with BodyParts3D 4.0 PART-OF files>')
  process.exit(1)
}
// The zip unpacks into partof_BP3D_4.0_obj_99/; accept that folder or its parent.
const objDir = [src, path.join(src, 'partof_BP3D_4.0_obj_99'), path.join(src, 'obj/partof_BP3D_4.0_obj_99')].find((d) =>
  fs.existsSync(path.join(d, 'FJ2810.obj')),
)
if (!objDir) throw new Error(`No BodyParts3D OBJ files under ${src}`)
const regions = JSON.parse(fs.readFileSync(path.join(here, '../../app/src/lib/model/anatomy.json'), 'utf8'))

const parts = new Map()
for (const line of fs.readFileSync(path.join(src, 'partof_element_parts.txt'), 'utf8').split('\n').slice(1)) {
  const [fma, , file] = line.split('\t')
  if (!file) continue
  if (!parts.has(fma)) parts.set(fma, [])
  parts.get(fma).push(file.trim())
}

/** Append one OBJ's vertices and triangles; BodyParts3D is in mm, Z up, Y towards the back. */
function readObj(file, pos, idx) {
  const base = pos.length / 3
  for (const line of fs.readFileSync(file, 'utf8').split('\n')) {
    if (line.startsWith('v ')) {
      const [x, y, z] = line.slice(2).trim().split(/\s+/).map(Number)
      pos.push(x / 1000, z / 1000, -y / 1000)
    } else if (line.startsWith('f ')) {
      const v = line.slice(2).trim().split(/\s+/).map((s) => parseInt(s, 10) - 1 + base)
      for (let i = 1; i + 1 < v.length; i++) idx.push(v[0], v[i], v[i + 1])
    }
  }
}

/** Replace the parts with their convex hull, in place. */
function hull(pos, idx) {
  const points = []
  for (let i = 0; i < pos.length; i += 3) points.push(new Vector3(pos[i], pos[i + 1], pos[i + 2]))
  const g = new ConvexGeometry(points)
  pos.length = 0
  idx.length = 0
  pos.push(...g.getAttribute('position').array)
  for (let i = 0; i < pos.length / 3; i++) idx.push(i)
}

const doc = new Document()
doc.getRoot().getAsset().copyright =
  'Derived from BodyParts3D, © The Database Center for Life Science, licensed under CC BY 4.0 (files marked CC BY-SA 2.1 JP). This model: CC BY-SA 4.0.'
const buffer = doc.createBuffer()
const scene = doc.createScene('body')
let floor = Infinity
const built = []
for (const [id, r] of Object.entries(regions)) {
  const files = parts.get(r.fma)
  if (!files) throw new Error(`No parts for ${id} (${r.fma})`)
  const pos = []
  const idx = []
  for (const f of files) readObj(path.join(objDir, `${f}.obj`), pos, idx)
  if (r.hull) hull(pos, idx)
  for (let i = 1; i < pos.length; i += 3) floor = Math.min(floor, pos[i])
  built.push({ id, r, pos, idx })
}
for (const { id, r, pos, idx } of built) {
  for (let i = 1; i < pos.length; i += 3) pos[i] -= floor
  const prim = doc
    .createPrimitive()
    .setAttribute('POSITION', doc.createAccessor().setType('VEC3').setArray(new Float32Array(pos)).setBuffer(buffer))
    .setIndices(doc.createAccessor().setType('SCALAR').setArray(new Uint32Array(idx)).setBuffer(buffer))
  scene.addChild(doc.createNode(id).setMesh(doc.createMesh(id).addPrimitive(prim)).setExtras({ fma: r.fma, kind: r.kind }))
}

await MeshoptSimplifier.ready
await MeshoptEncoder.ready
doc.createExtension(KHRMeshQuantization).setRequired(true)
doc.createExtension(EXTMeshoptCompression).setRequired(true).setEncoderOptions({ method: EXTMeshoptCompression.EncoderMethod.QUANTIZE })
await doc.transform(
  weld(),
  simplify({ simplifier: MeshoptSimplifier, ratio: 0.02, error: ERROR }),
  reorder({ encoder: MeshoptEncoder }),
  quantize(),
  meshopt({ encoder: MeshoptEncoder }),
)

let triangles = 0
for (const node of scene.listChildren()) {
  const prim = node.getMesh().listPrimitives()[0]
  triangles += prim.getIndices().getCount() / 3
}
fs.mkdirSync(path.dirname(OUT), { recursive: true })
await new NodeIO()
  .registerExtensions([EXTMeshoptCompression, KHRMeshQuantization])
  .registerDependencies({ 'meshopt.encoder': MeshoptEncoder })
  .write(OUT, doc)
console.log(`${path.relative(process.cwd(), OUT)}: ${Math.round(fs.statSync(OUT).size / 1024)} KB, ${triangles} triangles, ${built.length} regions`)
