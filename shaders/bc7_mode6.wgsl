// BC7 mode-6 compute shader. One workgroup per 4x4 block; one thread per pixel.
//
// Mode-6 layout (128 bits, little-endian within the 16-byte block):
//   bits 0..5    = 0 (zero prefix, six bits)
//   bit  6       = 1 (mode-6 marker)
//   bits 7..62   = endpoint channels packed as <Rlo7 Rhi7 Glo7 Ghi7 Blo7 Bhi7 Alo7 Ahi7>
//   bit  63      = P0  (low-endpoint pbit)
//   bit  64      = P1  (high-endpoint pbit)
//   bits 65..67  = sel[0]  (3 bits, anchor; MSB of the 4-bit index is implicit 0)
//   bits 68..71  = sel[1]  (4 bits)
//   ... through sel[15] at bits 124..127.
//
// Endpoint reconstruction (matches the CPU encoder bit-for-bit):
//   c8 = (c7 << 1) | p     // 8-bit value used to interpolate
//   col(t) = ( (64 - w[t]) * lo8 + w[t] * hi8 + 32 ) >> 6
// where w[t] is G_WEIGHTS4 (mode-6 uses 4-bit indices, 16 interpolation steps).
//
// Algorithm:
//   1. Each thread loads its pixel; subgroup reduces to mean (RGBA).
//   2. Compute 4x4 covariance, run 8 power-iteration steps to get principal axis.
//   3. Project pixels onto axis -> [min, max]; clamp to [0, 255]; round to int.
//   4. Try both pbit combinations (0,0) and (1,1) AND (0,1)/(1,0); for each:
//        - quantize endpoints to 7 bits given the pbit
//        - interpolate 16 palette entries with G_WEIGHTS4
//        - assign each pixel to nearest palette index, accumulate SSE
//      Pick lowest SSE.
//   5. If sel[0] has MSB=1, invert: swap endpoints, complement selectors.
//   6. Pack 128 bits, store as 4xu32 in the output buffer.

struct Params {
    num_blocks: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<storage, read>      pixels_in : array<u32>;  // 16 u32 per block (rgba8 packed)
@group(0) @binding(1) var<storage, read_write> blocks_out : array<u32>; // 4 u32 per block
@group(0) @binding(2) var<uniform>             params : Params;

const G_WEIGHTS4 = array<u32, 16>(
    0u, 4u, 9u, 13u, 17u, 21u, 26u, 30u, 34u, 38u, 43u, 47u, 51u, 55u, 60u, 64u
);

// Per-workgroup shared state (one block per workgroup, 16 threads = one per pixel).
var<workgroup> sh_pixels    : array<vec4<f32>, 16>;   // float copies for PCA
var<workgroup> sh_pixels_i  : array<vec4<i32>, 16>;   // int copies for selector match
var<workgroup> sh_mean      : vec4<f32>;
var<workgroup> sh_axis      : vec4<f32>;
var<workgroup> sh_t_min     : f32;
var<workgroup> sh_t_max     : f32;
var<workgroup> sh_best_err  : u32;
var<workgroup> sh_best_lo7  : vec4<i32>;
var<workgroup> sh_best_hi7  : vec4<i32>;
var<workgroup> sh_best_p    : vec2<i32>;
var<workgroup> sh_selectors : array<i32, 16>;
var<workgroup> sh_invert    : u32;

fn unpack_rgba(p: u32) -> vec4<i32> {
    return vec4<i32>(
        i32(p         & 0xffu),
        i32((p >>  8) & 0xffu),
        i32((p >> 16) & 0xffu),
        i32((p >> 24) & 0xffu),
    );
}

// Quantize a single 8-bit channel into a 7-bit endpoint given a fixed pbit.
// Mirrors `scale_color`/`quantize_endpoint` from bc7_pure.rs: take the
// 8-bit value `v` plus the chosen pbit `p`, return the 7-bit field such that
// reconstruct == ((c7 << 1) | p) reproduces the closest representable value.
fn quantize7_for_pbit(v: i32, p: i32) -> i32 {
    // Round (v - p) / 2 to nearest, clamp to [0, 127].
    let n = (v - p + 1) >> 1;
    return clamp(n, 0, 127);
}

fn dequant8(c7: i32, p: i32) -> i32 {
    return (c7 << 1) | p;
}

fn interp_color(lo8: vec4<i32>, hi8: vec4<i32>, w: u32) -> vec4<i32> {
    let w_i = i32(w);
    let inv = 64 - w_i;
    return (lo8 * inv + hi8 * w_i + vec4<i32>(32)) >> vec4<u32>(6u);
}

fn channel_sq(d: vec4<i32>) -> u32 {
    let s = d * d;
    return u32(s.x + s.y + s.z + s.w);
}

// One full encode trial with a given pbit pair. Each thread inspects one pixel,
// accumulates into shared memory under barrier. Returns nothing; writes into
// `sh_best_*` if SSE is lower than the previously stored best.
fn try_pbit(local_id: u32, lo7: vec4<i32>, hi7: vec4<i32>, p_lo: i32, p_hi: i32) {
    let lo8 = vec4<i32>(
        dequant8(lo7.x, p_lo), dequant8(lo7.y, p_lo),
        dequant8(lo7.z, p_lo), dequant8(lo7.w, p_lo),
    );
    let hi8 = vec4<i32>(
        dequant8(hi7.x, p_hi), dequant8(hi7.y, p_hi),
        dequant8(hi7.z, p_hi), dequant8(hi7.w, p_hi),
    );

    // Each thread finds the best selector for its pixel.
    let px = sh_pixels_i[local_id];
    var best_idx = 0u;
    var best_err = 0xffffffffu;
    for (var t = 0u; t < 16u; t = t + 1u) {
        let col = interp_color(lo8, hi8, G_WEIGHTS4[t]);
        let err = channel_sq(px - col);
        if (err < best_err) {
            best_err = err;
            best_idx = t;
        }
    }
    sh_selectors[local_id] = i32(best_idx);

    // Thread 0 sums errors (16 entries, sequential, no need for parallel reduction).
    workgroupBarrier();
    if (local_id == 0u) {
        var sum_err = 0u;
        var sels : array<i32, 16>;
        for (var i = 0u; i < 16u; i = i + 1u) {
            sels[i] = sh_selectors[i];
            let col = interp_color(lo8, hi8, G_WEIGHTS4[u32(sels[i])]);
            let d = sh_pixels_i[i] - col;
            sum_err = sum_err + channel_sq(d);
        }
        if (sum_err < sh_best_err) {
            sh_best_err = sum_err;
            sh_best_lo7 = lo7;
            sh_best_hi7 = hi7;
            sh_best_p   = vec2<i32>(p_lo, p_hi);
            // Persist selectors. We re-derive them below from saved endpoints
            // when packing the block, so we don't need to copy them here.
        }
    }
    workgroupBarrier();
}

@compute @workgroup_size(16, 1, 1)
fn main(
    @builtin(workgroup_id) wg_id : vec3<u32>,
    @builtin(local_invocation_id) lid : vec3<u32>,
) {
    let block_idx = wg_id.x;
    if (block_idx >= params.num_blocks) {
        return;
    }
    let local_id = lid.x;

    // Phase 1: load pixel
    let p_word = pixels_in[block_idx * 16u + local_id];
    let px_i = unpack_rgba(p_word);
    let px_f = vec4<f32>(px_i);
    sh_pixels[local_id] = px_f;
    sh_pixels_i[local_id] = px_i;

    if (local_id == 0u) {
        sh_best_err = 0xffffffffu;
    }
    workgroupBarrier();

    // Phase 2: mean (thread 0 — small N, no contention)
    if (local_id == 0u) {
        var m = vec4<f32>(0.0);
        for (var i = 0u; i < 16u; i = i + 1u) {
            m = m + sh_pixels[i];
        }
        sh_mean = m / 16.0;
    }
    workgroupBarrier();

    // Phase 3: principal axis via power iteration on 4x4 covariance.
    if (local_id == 0u) {
        var cov = array<f32, 16>();
        for (var i = 0u; i < 16u; i = i + 1u) {
            let d = sh_pixels[i] - sh_mean;
            cov[ 0] = cov[ 0] + d.x * d.x;
            cov[ 1] = cov[ 1] + d.x * d.y;
            cov[ 2] = cov[ 2] + d.x * d.z;
            cov[ 3] = cov[ 3] + d.x * d.w;
            cov[ 5] = cov[ 5] + d.y * d.y;
            cov[ 6] = cov[ 6] + d.y * d.z;
            cov[ 7] = cov[ 7] + d.y * d.w;
            cov[10] = cov[10] + d.z * d.z;
            cov[11] = cov[11] + d.z * d.w;
            cov[15] = cov[15] + d.w * d.w;
        }
        // Symmetric fill
        cov[ 4] = cov[ 1]; cov[ 8] = cov[ 2]; cov[ 9] = cov[ 6];
        cov[12] = cov[ 3]; cov[13] = cov[ 7]; cov[14] = cov[11];

        // Seed: largest-variance axis (rough guess).
        var axis = vec4<f32>(
            max(cov[0], 1e-6),
            max(cov[5], 1e-6),
            max(cov[10], 1e-6),
            max(cov[15], 1e-6),
        );
        // 8 power-iteration steps converge quickly on 4-D PSD matrices.
        for (var k = 0u; k < 8u; k = k + 1u) {
            let next = vec4<f32>(
                cov[ 0] * axis.x + cov[ 1] * axis.y + cov[ 2] * axis.z + cov[ 3] * axis.w,
                cov[ 4] * axis.x + cov[ 5] * axis.y + cov[ 6] * axis.z + cov[ 7] * axis.w,
                cov[ 8] * axis.x + cov[ 9] * axis.y + cov[10] * axis.z + cov[11] * axis.w,
                cov[12] * axis.x + cov[13] * axis.y + cov[14] * axis.z + cov[15] * axis.w,
            );
            let n2 = next.x*next.x + next.y*next.y + next.z*next.z + next.w*next.w;
            if (n2 < 1e-12) {
                break;
            }
            axis = next * inverseSqrt(n2);
        }
        sh_axis = axis;

        // Project to find extents
        var t_min = 1e30;
        var t_max = -1e30;
        for (var i = 0u; i < 16u; i = i + 1u) {
            let d = sh_pixels[i] - sh_mean;
            let t = d.x*axis.x + d.y*axis.y + d.z*axis.z + d.w*axis.w;
            t_min = min(t_min, t);
            t_max = max(t_max, t);
        }
        sh_t_min = t_min;
        sh_t_max = t_max;
    }
    workgroupBarrier();

    // Phase 4: derive endpoints in 8-bit space, then try pbit combos.
    if (local_id == 0u) {
        let lo_f = sh_mean + sh_axis * sh_t_min;
        let hi_f = sh_mean + sh_axis * sh_t_max;
        // Clamp + round to nearest 8-bit int (matches `round_half_up_u8`).
        let lo8 = vec4<i32>(clamp(round(lo_f), vec4<f32>(0.0), vec4<f32>(255.0)));
        let hi8 = vec4<i32>(clamp(round(hi_f), vec4<f32>(0.0), vec4<f32>(255.0)));

        // For each (p_lo, p_hi) in {0,1}^2, quantize the 8-bit endpoints
        // through that pbit's lattice and re-broadcast. The next dispatch
        // (try_pbit) is the per-thread part — but we run all 4 trials inline
        // since we have 16 threads idle outside try_pbit's parallel section.
        // sh_best_* stays valid across try_pbit calls.
        for (var combo = 0u; combo < 4u; combo = combo + 1u) {
            let p_lo = i32(combo & 1u);
            let p_hi = i32((combo >> 1u) & 1u);
            let lo7 = vec4<i32>(
                quantize7_for_pbit(lo8.x, p_lo),
                quantize7_for_pbit(lo8.y, p_lo),
                quantize7_for_pbit(lo8.z, p_lo),
                quantize7_for_pbit(lo8.w, p_lo),
            );
            let hi7 = vec4<i32>(
                quantize7_for_pbit(hi8.x, p_hi),
                quantize7_for_pbit(hi8.y, p_hi),
                quantize7_for_pbit(hi8.z, p_hi),
                quantize7_for_pbit(hi8.w, p_hi),
            );
            sh_best_p = vec2<i32>(-1, -1);  // sentinel; try_pbit will overwrite
            // Stash the trial endpoints for try_pbit to use through shared mem.
            sh_best_lo7 = lo7;
            sh_best_hi7 = hi7;
            sh_best_p   = vec2<i32>(p_lo, p_hi);
            // Run try_pbit out-of-band so all threads cooperate.
            // We can't actually re-enter try_pbit from inside a uniform-control-flow
            // branch (only thread 0 here). Instead: stash combo index, exit branch,
            // run try_pbit at outer scope. But the 4-combo loop has to be outside
            // any thread-0-only branch to remain uniform across the workgroup.
            // (Workaround: we treat combo 0 as the "best" and skip the others.
            // The 4-combo refinement happens at Phase 5 below in uniform scope.)
            if (combo == 0u) {
                sh_best_err = 0xffffffffu;  // reset for outer scope try_pbit
            }
        }
    }
    workgroupBarrier();

    // Phase 5: 4-combo pbit refinement (uniform scope, all threads participate).
    for (var combo = 0u; combo < 4u; combo = combo + 1u) {
        let p_lo = i32(combo & 1u);
        let p_hi = i32((combo >> 1u) & 1u);

        // Re-derive lo7/hi7 from sh_mean/sh_axis/sh_t_min/sh_t_max (all threads
        // read the same values — uniform).
        let lo_f = sh_mean + sh_axis * sh_t_min;
        let hi_f = sh_mean + sh_axis * sh_t_max;
        let lo8 = vec4<i32>(clamp(round(lo_f), vec4<f32>(0.0), vec4<f32>(255.0)));
        let hi8 = vec4<i32>(clamp(round(hi_f), vec4<f32>(0.0), vec4<f32>(255.0)));
        let lo7 = vec4<i32>(
            quantize7_for_pbit(lo8.x, p_lo),
            quantize7_for_pbit(lo8.y, p_lo),
            quantize7_for_pbit(lo8.z, p_lo),
            quantize7_for_pbit(lo8.w, p_lo),
        );
        let hi7 = vec4<i32>(
            quantize7_for_pbit(hi8.x, p_hi),
            quantize7_for_pbit(hi8.y, p_hi),
            quantize7_for_pbit(hi8.z, p_hi),
            quantize7_for_pbit(hi8.w, p_hi),
        );
        try_pbit(local_id, lo7, hi7, p_lo, p_hi);
    }

    // Phase 6: pack. Re-derive selectors from the winning endpoints first.
    workgroupBarrier();
    let lo8 = vec4<i32>(
        dequant8(sh_best_lo7.x, sh_best_p.x),
        dequant8(sh_best_lo7.y, sh_best_p.x),
        dequant8(sh_best_lo7.z, sh_best_p.x),
        dequant8(sh_best_lo7.w, sh_best_p.x),
    );
    let hi8 = vec4<i32>(
        dequant8(sh_best_hi7.x, sh_best_p.y),
        dequant8(sh_best_hi7.y, sh_best_p.y),
        dequant8(sh_best_hi7.z, sh_best_p.y),
        dequant8(sh_best_hi7.w, sh_best_p.y),
    );
    let px = sh_pixels_i[local_id];
    var my_best_idx = 0u;
    var my_best_err = 0xffffffffu;
    for (var t = 0u; t < 16u; t = t + 1u) {
        let col = interp_color(lo8, hi8, G_WEIGHTS4[t]);
        let err = channel_sq(px - col);
        if (err < my_best_err) {
            my_best_err = err;
            my_best_idx = t;
        }
    }
    sh_selectors[local_id] = i32(my_best_idx);
    workgroupBarrier();

    // Thread 0 packs the 128-bit block.
    if (local_id == 0u) {
        // Anchor MSB rule: sh_selectors[0] must have bit 3 clear. If set, invert
        // (swap endpoints, complement all selectors).
        var lo7 = sh_best_lo7;
        var hi7 = sh_best_hi7;
        var p_lo = sh_best_p.x;
        var p_hi = sh_best_p.y;
        var sels : array<i32, 16>;
        for (var i = 0u; i < 16u; i = i + 1u) {
            sels[i] = sh_selectors[i];
        }
        if ((sels[0] & 8) != 0) {
            // swap endpoints + pbits
            let tmp7 = lo7; lo7 = hi7; hi7 = tmp7;
            let tmpp = p_lo; p_lo = p_hi; p_hi = tmpp;
            for (var i = 0u; i < 16u; i = i + 1u) {
                sels[i] = 15 - sels[i];
            }
        }

        // Build the 128-bit value across two u64s, but WGSL doesn't have u64 —
        // emit four u32 words (little-endian byte order matching CPU output).
        var w0 = 0u;
        var w1 = 0u;
        var w2 = 0u;
        var w3 = 0u;

        // bit 6 = 1 (mode-6 marker)
        w0 = w0 | (1u << 6u);
        // bits 7..13:  Rlo
        w0 = w0 | (u32(lo7.x) << 7u);
        // bits 14..20: Rhi
        w0 = w0 | (u32(hi7.x) << 14u);
        // bits 21..27: Glo
        w0 = w0 | (u32(lo7.y) << 21u);
        // bits 28..31 of Ghi go into w0, remaining 3 bits into w1
        w0 = w0 | (u32(hi7.y & 0xf) << 28u);
        w1 = w1 | (u32(hi7.y >> 4));
        // bits 35..41: Blo  (bits 35..63 are in w1, offset = (bit - 32))
        w1 = w1 | (u32(lo7.z) << 3u);   // 35 - 32 = 3
        // bits 42..48: Bhi
        w1 = w1 | (u32(hi7.z) << 10u);  // 42 - 32
        // bits 49..55: Alo
        w1 = w1 | (u32(lo7.w) << 17u);  // 49 - 32
        // bits 56..62: Ahi
        w1 = w1 | (u32(hi7.w) << 24u);  // 56 - 32
        // bit 63: p_lo
        w1 = w1 | (u32(p_lo) << 31u);
        // bit 64: p_hi  -> w2 bit 0
        w2 = w2 | u32(p_hi);
        // sel[0] is 3 bits at bits 65..67 (anchor, MSB stripped)
        w2 = w2 | ((u32(sels[0]) & 7u) << 1u);
        // sel[1..15] are 4 bits each, starting at bit 68
        for (var i = 1u; i < 16u; i = i + 1u) {
            let bit_pos = 68u + (i - 1u) * 4u;     // 68, 72, 76, ..., 124
            let word = bit_pos / 32u;              // 2 or 3
            let shift = bit_pos % 32u;
            let val = u32(sels[i]) & 0xfu;
            if (word == 2u) {
                if (shift + 4u <= 32u) {
                    w2 = w2 | (val << shift);
                } else {
                    let lo_bits = 32u - shift;
                    w2 = w2 | (val << shift);
                    w3 = w3 | (val >> lo_bits);
                }
            } else {
                w3 = w3 | (val << shift);
            }
        }

        let out_base = block_idx * 4u;
        blocks_out[out_base + 0u] = w0;
        blocks_out[out_base + 1u] = w1;
        blocks_out[out_base + 2u] = w2;
        blocks_out[out_base + 3u] = w3;
    }
}
