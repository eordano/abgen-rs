# Cylinder.002 — UV-less + normal-mapped material gets `(1,0,0,1)` tangent

Landed fix for the bafybeigabziz7h / Cylinder.002 finding flagged in
`glb_scene_followup.md` (May-25 top-20 audit, rank 4 by combined bits).

## Diagnosis

glTF prim `Cylinder.002` in `models/events/ciberpunk.glb` declares
`['POSITION', 'NORMAL']` only — no `TEXCOORD_0`, no `TANGENT`. Material 3
(`Material.012`) declares `normalTexture` referencing `texCoord: 0`. Unity
fabricates a per-vertex tangent of `(1.0, 0.0, 0.0, 1.0)` for every vertex
(400/400 identical), pushing prod's stride0 to 40. abgen-rs's tangent
post-pass was gated by `prim.uvs.is_none → continue`, so we emitted no
TANGENT channel (stride 24, paired-diff 86,538 bits = 28 % of the bundle).

## Corpus probe — discriminator is CLEAN

Probed every prod bundle in `pathid_rt_v10_windows` + `validation_2`
(5,262 bundles total) for UV-less primitives (no `TEXCOORD_*`, no
`TANGENT`). Matched Unity mesh ↔ glTF prim by `(name, vertex_count)` to
defeat name collisions across multiple GLBs per entity.

| signal                          | prod ships tangent | doesn't |
|---------------------------------|-------------------:|--------:|
| `mat_has_normal_map = True`     |                  1 |       0 |
| `mat_has_normal_map = False`    |                  0 |     814 |

n = 815 UV-less primitives, 126 distinct bundles. **`mat_has_normal_map`
is a 1-to-1 discriminator** — zero false positives, zero false negatives.
The lone tangent-emitting case is Cylinder.002 itself.

For reference, the other candidate discriminators DON'T split:

- `is_collider_only`: 96 collider-only emit no tangent, 1 collider-only
 (Cylinder.002) does → not the rule.
- `has_mat` (material assigned): 748 with mat emit none, 66 without mat emit
 none → not the rule.

## Fix (`src/gltf.rs`)

Removed the `prim.uvs.is_none` short-circuit; let `calculate_tangents`
handle empty UVs (it already returns the degenerate `(1,0,0,1)` per
vertex):

```rust
for node in scene.nodes.iter_mut {
    for prim in node.primitives.iter_mut() {
        if prim.tangents.is_some() {
            continue;
        }
        let mi = match prim.material_index {
            Some(mi) if mi < scene.materials.len() => mi,
            _ => continue,
        };
        if scene.materials[mi].normal_image.is_none() {
            continue;
        }
        let empty_uvs: Vec<[f64; 2]> = Vec::new();
        let uvs = prim.uvs.as_deref().unwrap_or(&empty_uvs);
        let tangents = calculate_tangents(&prim.positions, &prim.normals, uvs, &prim.indices);
        prim.tangents = Some(tangents);
    }
}
```

## Numbers

**Per-bundle (paired bits-diff vs prod):**

| metric                                  | pre-fix    | post-fix   | delta    |
|----------------------------------------|-----------:|-----------:|---------:|
| bafybeigabziz7h… (Cylinder.002 bundle) |    308,111 |    221,573 |  -86,538 |
| Cylinder.002 mesh itself               |     86,538 |          0 |  -86,538 |

Cylinder.002 in OURS post-fix: stride0=40, ch2 (TANGENT, 4×float32), v0
tangent = `(1.0, 0.0, 0.0, 1.0)` — byte-exact with prod.

**Corpus-wide regression sweep (126 bundles with any UV-less prim,
pre-fix binary vs post-fix binary, paired bits-diff):**

| outcome | count |
|---------|------:|
| better  |     1 (Cylinder.002, −86,538 bits) |
| same    |   115 |
| worse   |     0 |

Total: pre = 600,694 bits, post = 514,156 bits, delta = **−86,538 bits**.
(10 bundles fail at `src/gltf.rs:87 read_accessor` in BOTH pre and post —
pre-existing crash unrelated to this fix.)

**glb-scene per_kind ppm projection** (denom 2,458,477,856 bits):

| metric       | pre-fix      | post-fix     |
|--------------|-------------:|-------------:|
| diff-bits    |    6,601,112 |    6,514,574 |
| ppm          |      2,685.0 |      2,649.8 |
| Δ            |              |      **−35.2 ppm** |

## Tests

- `cargo test --release --lib`: 119 passed (was 118; one tangent-fixture
 test renumbered earlier — unchanged by this fix).
- `cargo test --release --test parity_bytes`: 2 passed; all 10 per-fixture
 ppm caps unchanged (Cylinder.002 isn't in the fixture set).

## Artefacts

- `/tmp/cyl002_v2_probe.py` — corpus-wide UV-less prim probe (5,262 bundles,
 name+vc matching).
- `/tmp/cyl002_uvless_v2.json` — 815 UV-less primitive rows.
- `/tmp/uvless_focus_regression.py` — 126-bundle pre-vs-post regression sweep.
