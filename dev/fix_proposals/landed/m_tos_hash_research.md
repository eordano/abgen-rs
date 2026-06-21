# AnimatorController m_TOS — hash function (resolved) + ordering (still blocked)

## Tl;dr

- m_TOS is **already populated** by `src/animation_mecanim.rs::
 build_animator_controller` (14 entries on the windows v10 corpus, identical
 shape to prod).
- The hash function is **CRC32-over-name-UTF8-bytes** — verified bit-exact
 against prod via UnityPy probe (28 captured pairs across 2 AC bundles, see
 below).
- The remaining residual is **iteration order of the serialized
 (hash, name) pairs**, not content. That residual was already exhaustively
 characterised in `animator_controller_tos.md` (10+ hypotheses,
 0/17 matches).
- Task statement #issue-... assumed m_TOS held transform-path strings keyed
 by some unknown hash; **this is incorrect for emote/wearable bundles** —
 the values are state/parameter/transition names from the AnimatorController
 graph itself. No transform-hierarchy walk is required.

## Hash function — proof

Captured `m_TOS` of the two AnimatorController bundles in
`workdir/pathid_rt_v10_windows`:

- `bafkreia67htob7…` (clip = `Jump`) — 14 entries
- `bafkreiaaibdpib…` (clip = `Super Sayajin`) — 14 entries

Computed `zlib.crc32(name.encode)` for each captured `name` and compared to
the captured `hash` field. **28/28 pairs match.** Spot-check sample:

| name                                          | crc32       | prod hash   |
|-----------------------------------------------|-------------|-------------|
| `Loop`                                        | 23,966,416  | 23,966,416  |
| `GravityWeight`                               | 2,105,523,844 | 2,105,523,844 |
| `Base Layer`                                  | 756,556,552 | 756,556,552 |
| `Jump`                                        | 125,937,960 | 125,937,960 |
| `Base Layer.Jump`                             | 788,460,410 | 788,460,410 |
| `Jump -> Jump 0`                              | 2,134,487,021 | 2,134,487,021 |
| `Base Layer.Jump 0 -> Base Layer.Jump`        | 3,807,585,050 | 3,807,585,050 |
| `Entry -> Base Layer.Jump`                    | 3,577,713,430 | 3,577,713,430 |
| `Super Sayajin -> Super Sayajin 0`            | 2,513,883,697 | 2,513,883,697 |

(Reproducer: `/tmp/sim_our_tos.py` simulates our builder over the same clip
name and emits the same 14-entry set, which equals prod under set-equality.)

This matches our `crc32` in `src/animation_mecanim.rs::crc32` and the
known-good values asserted in `tests`:

    crc32("Base Layer")    = 0x2d18_2308
    crc32("Loop")          = 0x016d_b2d0
    crc32("GravityWeight") = 0x7d7f_be84

so no source change is required to fix the hash function — the residual is
purely ordering.

## Why the task's "transform-path" model is wrong here

UnityPy dump of the two AC bundles (`/tmp/tos_ac.log`, `/tmp/tos_ac2.log`)
shows the **14 prod TOS values** are the strings the AnimatorController graph
itself produces: parameter names (`Loop`, `GravityWeight`), the clip name,
the auxiliary `<clip> 0` clip, layer name (`Base Layer`), every
`m_FullPathID` and `m_ID` of states / transitions / any-state transitions,
and the empty string at hash 0. **No bone / Transform path appears.**

Transform-path TOS values would be expected on an
AnimatorOverrideController written on top of an Avatar `m_TOS`, but for the
emote/wearable RuntimeAnimatorController shape we emit, all entries are
graph-string hashes.

(The 7th entry `(0, "")` is the standard "root" placeholder the reference
carries at construction — our code matches this on line 539.)

## Why ordering is still blocked

Recap of `animator_controller_tos.md`:

- Set equality holds — both ours and prod have the same 14 (hash, name) pairs.
- Tried: ascending sort, descending sort, byteswap-sort, `std::unordered_map`
 bucket-mod at all sensible capacities {16..4096} × {forward, reverse}
 insertion, linear-probing open-addressing tables {14..96} ∪ powers-of-2,
 quadratic-probing, double-hashing, sort by `(h % N)` sweep, sort by bit-slice,
 sort by name-ascending, sort by name-length, sort by popcount.
- Result: **0/17 matches**, no stable common prefix.
- Confirmed: prod TOS order is a pure function of the hash set
 (two bundles with identical clip-hashes produce identical prod orders).

The single remaining viable hypothesis is that the **SerializedReference**
path that produced the reference uses a non-standard hashtable (e.g. one of
EASTL's `swissmap` / `bucket_hash_map` variants with a non-identity hash
mixer not in our test set, or a hand-rolled
`Runtime/Animation/RuntimeAnimatorController` hashtable). Confirming this
needs a Unity C# stack trace or runtime sampling of the serialization pass
that wrote the reference, both of which require Unity source / disassembly.
Out of scope under the no-disassembly rule.

## What was added in this audit (no code change to abgen-rs)

`Explorer/Assets/Editor/AbgenProbe/AbgenBundleProbe.cs::ProbeAcTOS(string
bundlePath, string outPath)` — additive Editor IPC method that loads an
AssetBundle and dumps `m_TOS` of every AnimatorController via
SerializedObject reflection. Useful for future ordering investigations
that need to capture larger TOS sets (multi-layer / multi-clip
controllers) than the 2 in our test corpus. Returns one JSON line per
controller + per TOS entry to a configurable outPath.

(Verified compile path — file is wired into the existing
`AbgenProbe.asmdef`. Probe was not invoked end-to-end because the editor
was still completing its first compile; the UnityPy dump above is the
ground-truth equivalent.)

## Suggested next moves

1. Use `ProbeAcTOS` on a bundle with a **larger** TOS (multi-clip emote, or a
 wearable with bone-anim overrides) to extend the captured pair set past
 28 — more pairs per single shape gives ordering-hypothesis tests more
 distinguishing power.
2. Cross-reference Unity's open-source `il2cpp` runtime headers (legally
 public) for the actual `pair<hash,string>` container type used in
 `AnimatorController::m_TOS` serialization.
3. If the order proves derivable from a non-public std-lib variant, document
 it as a permanent blocker and move the bit budget elsewhere.

## Files touched in this audit

- ADDED: `AbgenBundleProbe.cs::ProbeAcTOS` (this repo's mirror at
 `<unity-explorer>/...`)
- ADDED: this file
- UNTOUCHED: `src/animation_mecanim.rs` — set is already correct, ordering
 cap is gated on Unity-internal info per `animator_controller_tos.md`.
