# Draco vertex COLOR_0 is UNorm16, not Float32 (FIXED)

**Why it matters:** abgen-rs serializes every mesh vertex-color channel as
`Float32 dim=4` (16 bytes/vertex). The reference stores **draco-decoded**
COLOR_0 as **`UNorm16 dim=4`** (8 bytes/vertex). On any draco mesh that carries
a vertex color, abgen's mesh is therefore 8 bytes/vertex too large, with a
different `m_Channels` layout and different color bytes. This is structural
(raw-length-different), not value-noise.

**How it was found:** the blindspot reference corpus
(`abc-abgenrs-799967c3-2026-06-20/blindspot-windows`) was built specifically to exercise rare
glTF paths the production corpus (val300) under-samples — including draco. The
draco wearable `bafkreigyis3hocfsqpmela7cwhdgm6vtxc7drfgyfgm6piqsig72lt72cm`
(bundle `bafybeig7xux7ukcnrtqkr4dhtny3fmbohyyx5msvymqbej5lxshqp3af6a_windows`,
flags `draco`+`multimat`+`texxform`) was the single largest structural diff:
ours +23,200 bytes across ~18 meshes, a consistent ~8 bytes/vertex on the
color-bearing meshes. Parsing the `m_Channels` ChannelInfo[8] array of one
diverging Mesh (object PathID `-9196413766792891959`) side by side:

```
OURS  pos s0 o0  Float32 dim3   nrm s0 o12 Float32 dim3
      col s1 o0  Float32 dim4   uv0 s1 o16 Float32 dim2   uv1 s1 o24 Float32 dim2   (stream1 stride 32)
REF   pos s0 o0  Float32 dim3   nrm s0 o12 Float32 dim3
      col s1 o0  UNorm16 dim4   uv0 s1 o8  Float32 dim2   uv1 s1 o16 Float32 dim2   (stream1 stride 24)
```

The only difference is the color channel format (`Float32`→`UNorm16`, vertex
attribute format byte `0`→`4`), which shrinks stream 1 by 8 bytes/vertex and
shifts every following attribute offset down by 8.

**Root cause (clean-room, fork source read):** the regular glTFast vertex path
hard-codes `VertexAttributeFormat.Float32` for colors
(`unity-gltf/Runtime/Scripts/VertexBufferColors.cs:89`). The **draco** path does
NOT go through that code — draco primitives are decoded by the native DracoUnity
plugin (`PrimitiveDracoCreateContext` → `DracoMeshLoader.ConvertDracoMeshToUnity`),
which builds the Unity Mesh directly from draco's quantized attributes and keeps
COLOR_0 as a `UNorm16` vertex attribute (draco stores colors as normalized
integers). So the format is *draco-branch specific*: a mesh with COLOR_0 imported
via plain glTF → Float32 color; the same mesh via draco → UNorm16 color.

abgen decodes draco to float in `src/draco.rs::materialize` and then runs the
shared mesh path (`src/mesh_layout.rs::build_channels`), which always emits
`FMT_FLOAT32` for `CH_COLOR`. Hence the divergence is confined to draco + COLOR_0.

**Scope:** draco glbs are rare (377 in the production corpus per
`blindspot_entities.json`), and only the subset carrying a vertex-color
attribute is affected. val300 has effectively none — this path is exactly what
the blindspot corpus exists to surface. The fix does not touch the val300 5139
byte-id baseline.

## Fix (landed)

Gated to draco-sourced primitives only (`color_unorm16 = prim.from_draco &&
prim.colors.is_some()`, plumbed into `MeshAttributes`):

1. **`src/mesh_layout.rs::build_channels`** — emits `CH_COLOR` as
   `ch(stream, off, FMT_UNORM16, 4)` (8 bytes) instead of `FMT_FLOAT32` (16
   bytes) when `color_unorm16`, advancing the stream-1 offset cursor by the
   format-correct width (`fmt_bytes(fmt)*dim`) so the trailing UV offsets shift
   down by 8.

2. **`src/mesh_layout.rs::vertex_buffer`** — the stride is sized by per-format
   component bytes (`fmt_bytes`), and the color is packed via `pack_unorm16`
   (`round(clamp(c,0,1) * 65535)` per component, little-endian u16) instead of
   four f32.

**Result:** the blindspot draco wearable `bafkreigyis3...` bundle
`bafybeig7xux7...` went from +23,200 structural bytes (wrong channel layout) to
**raw-length-equal** — every diverging Mesh now matches the reference size
exactly (16652=16652, etc). One mesh shows ~114/16652 residual bytes: a mix of
the documented draco position/normal float-residue wall and ±1-2 LSB on the
UNorm16 color (the exact DracoUnity quantization round mode is the remaining
detail, same class as other native-plugin rounding walls — small and bounded).

**Gate:** val300 stays 5139 byte-id with ZERO bundles changing identity
(strict per-bundle byte-id set diff), since draco+COLOR_0 is unsampled there.
The stride-calc change is identity for all-Float32 channels. 98 tests green.
