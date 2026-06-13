# Emote AnimatorController m_TOS serialization order (blocked — wall extension)

**Why it matters:** Every emote AnimatorController stores an `m_TOS` table mapping CRC32 name hashes to the original strings (state names, transition names, parameters, the layer name). abgen emits the same key/value set as the reference — verified entry-for-entry on all val300 CAT7 emote pairs — but serializes it sorted ascending by the CRC32 key, while Unity writes it in the iteration order of an internal hash container. The result is a pure permutation: identical content, different byte order, a handful of struct windows in every otherwise wall-blocked controller. This is byte-cosmetic; closing it would not flip any bundle to byte-identical because the AnimationClip PathID and Mecanim clip-content walls remain.

**What the data proves about the order:**

- It is a *deterministic function of the name set*. Five distinct val300 emotes whose glb clip is named `Starting_Pose` (different entity CIDs, different conversion runs) produce byte-identical m_TOS orderings. So no session state, pointer values, or PRNG is involved — unlike the clip sub-asset index wall.
- It is *not insertion order*. All single-clip emote controllers are built by an identical code path differing only in the clip name, yet their orderings interleave the structurally-fixed entries (`Loop`, `GravityWeight`, `Base Layer`, the empty string) differently per clip name. A name-dependent hash is involved.
- It is *globally consistent*: pooling pairwise precedences across all 21 reference controllers (222 distinct names) yields zero contradictions, i.e. some global total order over names exists.
- Coarse structure: `GravityWeight` always lands near the front, `Base Layer` and the empty string mid-table, `Loop` near the back — stable per string, independent of the rest of the set. That is the signature of bucketed iteration of a hash table whose hash we have not identified.

**Hypotheses tested and ruled out** (each checked against all 21 reference orderings; bucket tests require the bucket sequence to be non-decreasing — grouped iteration — in ascending or descending direction, and were also run with a one-wrap cyclic tolerance):

- Insertion order of `build_animator_controller` (disproven by the single-clip interleaving argument above).
- CRC32-key bucket order, `crc % size`, sizes 4–1024 (all powers of two) and primes up to 389.
- Affine/multiplicative hashing of the CRC key: exhaustive sweep of **all 2^31 odd 32-bit multipliers** `K` for orderings by `(K*h) mod 2^32` top bits / mod, sizes 16–256, including the cyclic variant that an additive offset would produce. Zero candidates.
- Standard integer mixers of the CRC key: Murmur3 finalizer, Knuth, xorshift16, byteswap, bit-reverse, FNV-1a over the 4/8 LE bytes, splitmix64, Wang, Mueller, Jenkins one-at-a-time over the bytes.
- String hashes of the name: FNV-1/FNV-1a (32/64), djb2 (add and xor), sdbm, java-31/Mono `String.GetHashCode`, BKDR, ELF/PJW, RS, AP, DEK, JS, Jenkins one-at-a-time, SuperFastHash, Murmur2/Murmur2A/Murmur3-x86-32, Murmur64A, xxHash32/xxHash64.
- Alternate checksums as a full sort key: CRC32C, CRC32K, CRC32Q, CRC32 of the lowercased / reversed / NUL-terminated / UTF-16-LE name, un-xored CRC.

All of these behave as statistically random against the reference orderings (descent/inversion counts at chance level). Unity's container and hash are engine-internal; the clean-room rule (no Unity engine source, no disassembly) prevents looking the algorithm up, and black-box fitting of the standard hash space has been exhausted. This is the same blocker class as the clip sub-asset PathID wall, with the difference that this one is provably *derivable in principle* (it is content-deterministic) — it is the function that is unknown.

**Impact:** a fixed-count permutation inside the 21 CAT7 emote AnimatorControllers (the only producer of `m_TOS` in abgen output). No size delta, no semantic delta — consumers look keys up by hash.

**What would close it:** identifying Unity's internal hash + table-growth policy for the TOS container (e.g. via a future upstream converter change that re-serializes m_TOS in a documented order, or a licensed source consultation), then mirroring it in `build_animator_controller`'s `tos_set`/sort step in `src/animation_mecanim.rs`.

**Tools:** `examples/dump_tos.rs` prints a bundle's AnimatorController m_TOS entries in serialized order; `examples/ctrl_cmp.rs` leaf-diffs two controllers' typetrees (this is how the residual controller delta was decomposed into exactly: clip PPtr relabels (wall), m_TOS permutation (this doc), blend-tree `m_ClipID` (fixed), `m_ControllerSize` (fixed)); `examples/ctrlsize_probe.rs` compares stored `m_ControllerSize` against the release-typetree serialized size of `m_Controller`.

**2026-06-11 probe update.** Sixteen controlled-name probe emotes (clip names
`a b z A aa ab ba az aaaa aaaaaaaa a0 0a` plus four synthetic clips) were run
through the actual reference converter, twice. New facts:

- **The order is deterministic across converter re-runs** (run A == run B for
  all 17 emote controllers' m_TOS, while the clip PathIDs changed) — so it is
  content-derivable in principle, unlike the PathID rank.
- **The global-total-order hypothesis is disproven**: probe `a0` orders the
  empty string *before* `Base Layer` while every other probe orders it after.
  No sort by any per-name key can produce the reference orders; collisions /
  table-layout interactions are involved.
- New families ruled out beyond the earlier battery (probe + corpus orderings,
  38 sets): first-reference order over the serialized `m_Controller` blob
  (`examples/tos_firstref.rs`); string-hash *bucket-grouped* iteration (FNV-1/
  1a-32/64, djb2 add/xor, sdbm, java31, Jenkins OAAT, Murmur3, xxh32/64,
  SpookyV2-low, crc bit-reverse/byteswap; M=2..1024, both directions, mod and
  top-bits — `examples/tos_fit.rs`); and **open-addressing/linear-probing slot
  order** for all of those hashes at M=16/32 with full displacement freedom and
  cyclic wrap (a run-decomposition feasibility DP — zero of 33 orderings are
  even *feasible*). Whatever container Unity iterates, its hash matches no
  standard family; further black-box recovery would need adaptive
  collision-crafting against an unknown function class, which is not currently
  tractable. The probe orderings are preserved in the session notes for any
  future attempt.

**2026-06-11 (late) — data preserved + structural disproofs + stakes revised.**
The 21 reference orderings, the 12 controlled-name probe orderings, the prior
solver scripts, and a consolidated runnable harness are now checked into
`docs/tos_data/` (`emoteNN.tsv`, `probe-tos-*.tsv`, `tos_solver.py`) so this
work survives `/tmp` cleanup. New results this session:

- **The wall is no longer cosmetic — it is the SOLE remaining diff on the 21
  emote bundles, worth real byte-id.** Since the deterministic doc-order fix
  (`d931b01`) against the `974f971-val300-windows` reference, `examples/objalign`
  shows every other object in a glb-emote bundle byte-identical (all 133 objects
  on the sampled bundle, AnimationClip size-exact, no DIFF flags) and
  `examples/ctrl_cmp` shows the controller's ONLY leaf difference is the `m_TOS`
  permutation. So cracking this ordering would flip up to ~18 of the 21 emotes
  to byte-identical (the remaining ~3 carry independent one-lane float residues).
  Full val300 windows baseline on this worktree = **5109 byte-id**, glb-emote
  **0/21**. This raises the value of the wall well above its old "byte-cosmetic"
  framing.

- **The iteration is NOT a slot-walk of a single contiguous table, in any
  direction or rotation.** Decisive new disproofs (over all 21 reference + 12
  probe tables, home = key, dcrc(key), murmur(key), fib-top-bits; N = pow2
  16..32768):
  - *Cyclic-rotation of sort-by-(hash mod N): 0/33* — the observed order is not
    even a rotation of any home-slot ordering (some hashtables begin iteration
    at a non-zero slot; this kills that escape too).
  - *First-entry is neither consistently min-home nor max-home* across the 21
    (ident: 0/21 min, ≤2/21 max; dcrc best 6/21 min). Ascending- and
    descending-slot iteration are both excluded by the first element alone — the
    first serialized key has an interior home slot.
  - *Greedy linear-probe placement of `key % N` fails on the FIRST element*: the
    first observed entry's `key % N` is frequently the maximum home in the set
    (e.g. probe-a first key 405470207, 405470207 % 16 = 15), impossible under
    slot-0-ascending linear-probe iteration. So home ≠ `key % N` under any
    slot-order read.

- **A consistent total order over keys exists *within a fixed table size*.**
  Pooling pairwise precedences over the 15 n=14 reference tables (114 distinct
  keys) yields **zero contradictions** and a single topological linearization
  (`docs/tos_data/order114.py`) that every n=14 table respects exactly. This is
  the cleanest ground truth available: the order is a deterministic function and
  the n=14 instances are all slices of one 114-key master order. BUT that master
  order is **not** `sort by hash(key) mod N` for any tested hash/N — the
  bucket-monotone violation count bottoms out at ~55/113 (chance ≈ 57), i.e.
  statistically random for ident/dcrc/murmur at every N. (The earlier
  "global-total-order disproof" came from a *synthetic probe* `a0`, whose
  artificial names collide differently; for the natural corpus name family a
  per-N total order does hold.)

- **Linear-probe simulation with a reconstructed insertion order also fails.**
  Rebuilding abgen's TOS build sequence (empty, Loop, GravityWeight, clips,
  per-clip transition/state/AnyState names, then Base Layer) and simulating
  plain linear probing, Robin-Hood (insertion-independent), libstdc++-style
  `unordered_map` chaining, and a **growing/rehashing** open-addressing table
  (initial size 1..16, doubling at load 1/2..1/1, re-insert in iteration order
  on rehash) — all 0/33 across {ident, dcrc, murmur, fib} home functions. The
  binding unknown is therefore Unity's **true insertion order** (the native
  name-registration sequence during `AnimatorController` build / Mecanim
  `ControllerConstant` construction), which abgen's order does not match and
  which cannot be inferred offline.

**Conclusion / next step.** Offline mining is exhausted: the order is a
deterministic, within-size-consistent function whose container + hash match no
standard family, and whose insertion order is engine-internal. The only
remaining tractable lever is **controlled probes that vary entry count** (to
expose the table-growth boundaries) and **vary which names collide** (to read
off the within-bucket / probe-displacement rule and back out the insertion
order) — exactly the probe technique that cracked the other emote walls. Resume
from `tos_data/` (alongside this page): the harness (`tos_solver.py`) is
ready to fold new probe orderings in, and the open question to settle first is
the **insertion order** — design probes with 2,3,5,9,17 clips to bracket each
power-of-two growth step, and probes whose clip names CRC-collide modulo small N
to reveal within-bucket ordering.

**2026-06-11 (probe campaign) — clip-count + crc-collision + glb-order probes
run; every standard container model now disproven on controlled inputs.**
The prescribed campaign was executed: 13 controlled probe emotes were built by
duplicating one emote's GLB animation block under crafted names (recompute CID,
serve flat over HTTP, convert with the real Unity 6000.2.6f2 converter) and
their `m_TOS` tables read back. Probe set (committed in `docs/tos_data/probes2/`
with `map.tsv` + the builder `mkemote.py`):

- **count probes** `c01,c02,c03,c04,c05,c08,c09,c16,c17` — 1..17 identical-content
  clips (names `k00…`), giving tables of 14,24,34,44,54,84,94,164,174 keys
  (exactly +10 keys/clip), bracketing every power-of-two table-growth boundary.
- **collision probes** `x16` (clip names `aa,ax,a8,bf`, all `crc32 % 16 == 7`)
  and `x32` (`aa,ax,bf,c0`, all `% 32 == 23`) — isolate the within-bucket rule.
- **glb-order probes** `o_fwd` / `o_rev` — the SAME 4-clip name set in forward
  vs reversed GLB animation order — separates name-derived order from
  glb-insertion-derived order.

Decisive new results (all folded into `tos_solver.py`, sections [4]–[6]):

- **Insertion order matters, but ONLY within collision groups.** `o_fwd` and
  `o_rev` (identical name set, reversed clip order) produce `m_TOS` tables that
  differ in just **5 of 44 positions** — and those 5 are exactly the keys whose
  relative order flips with clip-insertion order. So the global order is a
  deterministic function of the name hashes, with insertion order breaking ties
  *within* a hash-collision group (the signature of a hash container whose
  within-bucket / probe order is insertion order). This is the first positive
  structural fact and it rules out any pure per-name sort.
- **NOT open addressing (any standard hash).** Reading a linear/quadratic-probe
  table in slot order requires the home-slot sequence to be non-decreasing with
  ≤1 wrap. The observed home-monotonicity wrap count is ~`n/2.4` for every
  probe under {ident, dcrc, murmur, fib} at every N (max 87 on the 174-key
  probe) — statistically random, scaling with table size. Growing/rehashing
  open-addressing with Robin-Hood, initial caps 1..16 and load factors
  1/2..1/1, over the exact abgen insertion order, is **0/13**.
- **NOT bucket-chaining by `key % N` (or `hash(key) % N`).** Chaining iterated
  in bucket order requires each bucket's keys to occupy a contiguous run; the
  observed runs are non-contiguous for every probe at N=16/32/64 (0/13, 0/13,
  1/13). Ascending **and** scrambled-but-fixed bucket-visit order are both
  excluded, since no fixed bucket function even makes the buckets contiguous.
- **NOT a registration/graph-traversal order.** Labeling each key by its build
  role (state/transition/parameter, per clip) shows the observed order is fully
  scrambled across roles (e.g. c05 begins `FT100 T103 ANY3 T011 FS14 …`), not
  any clip-major or role-major traversal — so `m_TOS` is genuinely hash-table
  iteration, not the order strings were registered.
- **String-hash bucket iteration falsified too** (FNV-1a 32/64, djb2, java31,
  Murmur2 over the UTF-8 name, mod and top-bits, both directions): 0/13.
- **Multiplicative full-sort excluded at scale:** `sort by (K·h) >> s` over
  ~200k odd multipliers K × shifts {0,8,16,24,28} on all 46 orderings: 0 hits.

**Net:** the controlled-probe campaign converts the prior "looks random" verdict
into hard disproofs of open-addressing, chaining, registration-order, and
sort-based models, while establishing the one positive constraint (within-
collision-group order == clip insertion order). The container is a hash table
whose **bucket/slot function matches no standard family** and whose iteration is
neither home-ascending nor contiguous — consistent with an engine-internal
`dynamic_hash_table` whose node-array layout is rehash-scrambled. Black-box
recovery of that layout function from output alone is not tractable without
either Unity-internal source (clean-room-forbidden) or a much larger
adaptive-collision-crafting campaign against an unknown hash class. The 46
orderings + the runnable harness are preserved; the remaining lever is
narrower than before (the *bucket-index* permutation of a power-of-two table,
which would need probes that pin one bucket's membership at a time).

**2026-06-12 — the "within-collision-group" reading is itself falsified; the
order is not a hash placement of the CRC keys at all.** A second harness
(`tos_data/tos_solver2.py`) re-attacked the single positive fact — the 5-of-44
swap between `o_fwd` and `o_rev` — and removed its previous interpretation:

- **Insertion order is now pinned exactly.** `c04` (the 4-clip count probe, built
  in forward glb order) is *byte-identical* to `o_fwd`, so the forward glb clip
  order **is** Unity's TOS build/insertion order. Every container simulation can
  use this exact sequence with no reconstruction guesswork.
- **No hash co-locates the swapped keys.** The two swap groups are
  `A = {419095881, 1614764057}` (clips k00/k03) and
  `B = {1310708297, 493921129, 1302005287}` (clips k01/k03). For the
  "within-bucket order == insertion order" model to hold, *some* home function
  must place each group in one bucket. Sweeping `{ident, dcrc, fib, murmur,
  revbits, byteswap}` × `{mod, top-bits}` × `N = 2..2^17`, **zero** functions
  even make the swapped pairs collide. The swap is therefore *not* a
  within-bucket tie, and the keys are **fully interleaved by clip** in the
  output (positions 0–10 read k00, k03, k03, k01, struct, k01, k01, k03, k03,
  k03, k00 — no per-clip block structure). The swaps are isolated single-key
  transpositions of *output-adjacent* keys, not block moves.
- **No stable sort, no probe-collision, no open addressing — confirmed jointly
  on `o_fwd` AND `o_rev` with the pinned insertion order.** Stable sort by any
  lossy `f(hash)` (radix high-bits / low-mask / mod over all 6 hashes), ties by
  insertion: 0 candidates. No `N` gives both swap groups near-adjacent home
  slots (probe-displacement collision): 0. Open-addressing slot-read
  monotonicity wraps bottom out at 14/44 (o_fwd), 18/54, 66/164 over *all*
  capacities `n..8192` — scaling with `n`, i.e. random, not the ≤1 a slot read
  needs. Chaining (FIFO/LIFO × mod/top × both bucket directions, `N` up to 2048)
  and linear/quadratic open addressing (all caps) from the pinned insertion
  order: **0 hits each**.
- **Not a global name-intern order either.** Pooling pairwise *name* precedences
  (not key precedences) across all 46 tables yields 20 contradictory name-pairs,
  and the structural names (`''`, `Loop`, `GravityWeight`, `Base Layer`) land at
  wildly different positions per table — so the order is not a process-global
  `FastPropertyName` registration index.

The net effect is to *shrink, not widen*, the model space: the m_TOS order is
not a sort of the CRC keys, not a placement (home/probe/chain) of the CRC keys
under any standard hash, and not a global intern order — yet it is
content-deterministic and insertion-sensitive only at a handful of
output-adjacent positions. This is the fingerprint of an engine-internal
container whose node identity is derived from something *other than the CRC32*
that abgen records as the key (e.g. a separate `FastPropertyName` 32-bit id, or
a pointer-stable allocation index keyed by a different hash), with the CRC merely
stored as the value. Recovering it offline is not possible: it requires either
Unity-internal source (clean-room-forbidden) or a probe that can *read the node
identity directly*. A clip-name probe cannot isolate a single key — each clip
name cascades to ~10 derived TOS entries (`clip`, `clip 0`, `Base Layer.clip`,
the transitions, the AnyState/Entry edges) whose strings, and therefore CRCs,
are all fixed by the one name — so no probe can hold the full key set constant
while moving exactly one key. That structural obstruction, not a lack of compute,
is why the offline and probe-by-clip-name avenues are both closed.

**Tooling added this session:** `tos_data/tos_solver2.py` reproduces every
negative above from the committed orderings; a CRC32 forge (construct a 4-byte
suffix to hit any 32-bit target, and the printable-name *selection* variant for
buildable collision sets) was implemented and verified but is not landable as a
probe for the reason just given. The CRC-sort at `src/animation_mecanim.rs:912`
(`tos_sorted.sort_by_key(|(h, _)| *h)`) is **unchanged** — there is no derived
ordering function to replace it with, so `examples/tos_only_proof` still reports
0/21 byte-equal on the glb-emotes (the 18/21 flip remains contingent on a
function we cannot derive without Unity-internal information).
