// bc7kernel_harness: run the REAL bc7e.ispc kernel on captured mip0 inputs and
// score block-exact concordance vs ours (sanity) and ref (the prize).
//
// Probe record (100 bytes, little-endian):
//   u8 input[64];   // 16 RGBA pixels
//   u8 ours[16];    // abgen's bundle block (Rust bc7e port)
//   u8 ref[16];     // Unity reference block
//   u8 is_diff;     // 1 if ours!=ref
//   u8 pad[3];
//
// Usage: bc7kernel_harness <probe> <perceptual 0|1> <profile>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "bc7e_ispc.h"

#define REC 100

int main(int argc, char** argv) {
    if (argc < 4) { fprintf(stderr, "usage: %s <probe> <perc 0|1> <profile>\n", argv[0]); return 2; }
    const char* probe = argv[1];
    int perc = atoi(argv[2]);
    const char* prof = argv[3];

    FILE* f = fopen(probe, "rb");
    if (!f) { perror("open probe"); return 2; }
    uint32_t n = 0;
    if (fread(&n, 4, 1, f) != 1) { fprintf(stderr, "read count\n"); return 2; }
    uint8_t* buf = (uint8_t*)malloc((size_t)n * REC);
    if (fread(buf, REC, n, f) != n) { fprintf(stderr, "short read\n"); free(buf); return 2; }
    fclose(f);

    bc7e_compress_block_init();
    struct bc7e_compress_block_params p;
    if      (!strcmp(prof, "basic"))    bc7e_compress_block_params_init_basic(&p, perc);
    else if (!strcmp(prof, "slow"))     bc7e_compress_block_params_init_slow(&p, perc);
    else if (!strcmp(prof, "veryslow")) bc7e_compress_block_params_init_veryslow(&p, perc);
    else if (!strcmp(prof, "slowest"))  bc7e_compress_block_params_init_slowest(&p, perc);
    else if (!strcmp(prof, "fast"))     bc7e_compress_block_params_init_fast(&p, perc);
    else { fprintf(stderr, "unknown profile %s\n", prof); free(buf); return 2; }

    long diff_total = 0, diff_match_ref = 0, diff_match_ours = 0;
    long match_total = 0, match_match_ref = 0, match_match_ours = 0;
    long kernel_eq_ours_all = 0;

    for (uint32_t i = 0; i < n; i++) {
        uint8_t* rec = buf + (size_t)i * REC;
        const uint32_t* pixels = (const uint32_t*)rec;  // 16 RGBA
        uint8_t* ours = rec + 64;
        uint8_t* refb = rec + 80;
        uint64_t out[2];
        bc7e_compress_blocks(1, out, pixels, &p);
        uint8_t* ko = (uint8_t*)out;
        int eq_ours = (memcmp(ko, ours, 16) == 0);
        int eq_ref  = (memcmp(ko, refb, 16) == 0);
        int diff    = (memcmp(ours, refb, 16) != 0);
        if (eq_ours) kernel_eq_ours_all++;
        if (diff) { diff_total++; if (eq_ref) diff_match_ref++; if (eq_ours) diff_match_ours++; }
        else      { match_total++; if (eq_ref) match_match_ref++; if (eq_ours) match_match_ours++; }
    }

    printf("profile=%-8s perceptual=%d  N=%u\n", prof, perc, n);
    printf("  kernel==ours (sanity, all):   %8ld / %u  (%.2f%%)\n",
           kernel_eq_ours_all, n, 100.0*kernel_eq_ours_all/n);
    printf("  DIFF-blocks (ours!=ref):      %8ld\n", diff_total);
    printf("    -> kernel==ref (RECOVERED): %8ld  (%.2f%%)\n",
           diff_match_ref, diff_total? 100.0*diff_match_ref/diff_total:0.0);
    printf("    -> kernel==ours (no help):  %8ld  (%.2f%%)\n",
           diff_match_ours, diff_total? 100.0*diff_match_ours/diff_total:0.0);
    printf("  MATCH-blocks (ours==ref):     %8ld\n", match_total);
    printf("    -> kernel==ref (KEPT):      %8ld  (%.2f%%)\n",
           match_match_ref, match_total? 100.0*match_match_ref/match_total:0.0);
    free(buf);
    return 0;
}
