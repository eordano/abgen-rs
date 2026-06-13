# Third-party notices

abgen-rs is licensed under the GNU Affero General Public License v3.0
or later (`AGPL-3.0-or-later`); see `LICENSE` at the crate root. This
file lists the third-party material it embeds or depends on.

Every vendored upstream license below (Apache-2.0, BSD-2-Clause, zlib,
public-domain) is one-way-compatible with AGPLv3: their terms continue
to apply to the ported files (clean-room reimplementations from public
specifications), while abgen-rs as a whole — including any derivative
work, network-deployed instance, or downstream redistribution — is
governed by AGPLv3. Section 13 (the network-use clause) applies to any
service that exposes abgen-rs functionality over a network.

## Ports of public codecs and reference algorithms

| Component | License | Source file | Upstream |
|---|---|---|---|
| bc7e (scalar port) | Apache-2.0 | `src/bc7_pure.rs` | github.com/GameTechDev/bc7e |
| LZ4 / LZ4-HC | BSD-2-Clause | `src/lz4.rs` | github.com/lz4/lz4 |
| INFLATE | zlib | `src/png.rs` | RFC 1951 |
| Adam7 deinterlace | (no license — algorithm spec) | `src/png.rs` | PNG spec / W3C REC-PNG |
| SpookyHash V2 | public domain | `src/hashes.rs` | burtleburtle.net/bob/hash/spooky.html |
| MD4 | public domain | `src/hashes.rs` | RFC 1320 |
| MD5 | public domain | `src/hashes.rs` | RFC 1321 |
| XXH64 | BSD-2-Clause | `src/hashes.rs` | github.com/Cyan4973/xxHash |
| CRC32 (table) | public domain | `src/hashes.rs` | IEEE 802.3 |
| MikkTSpace | (currently not vendored) | — | mikktspace.com (zlib license if added) |

Each port is a clean-room reimplementation from public specifications
and/or permissively-licensed reference code; no proprietary source was
consulted. Where the port reaches bit-exact equivalence with the
reference, fixtures under `tests/fixtures/` document the parity.

## Vendored upstream sources (compiled in via build.rs)

| Vendored at | License | Used for | Upstream |
|---|---|---|---|
| `third_party/draco_decoder/third_party/draco/` | Apache-2.0 | glTF Draco mesh decode | github.com/google/draco |
| `third_party/crunch/{crnlib,inc}/` | Public Domain (Unlicense-equivalent) | BC5 normal-map CRN encode | github.com/BinomialLLC/crunch |

The crunch tree is the BinomialLLC `crnlib` (v1.04) released into the
public domain on 2020-09-15 (see `third_party/crunch/license.txt`). We
compile a fixed subset of `crnlib/*.cpp` plus the bundled LZMA codec
into a single static lib via `third_party/crunch/build.rs`, exposed
through a thin C ABI (`cpp/crn_wrapper.cc`) and a Rust wrapper crate
(`crunch_ffi`). The encoder is the load-bearing piece —
`crn_compress(.. cCRNFmtDXN_XY ..)` — invoked from
`src/bc5_pure.rs::encode_bc5_normal_crn_mip_chain` when the
`bc5_normal_images` classifier fires.

## Rust crate dependencies

All Cargo dependencies are permissively licensed. The license families
present in the dependency graph (from `cargo metadata`):

| Count | License |
|---:|---|
| 129 | MIT OR Apache-2.0 |
| 33 | MIT |
| 18 | Unicode-3.0 |
| 14 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| 11 | Apache-2.0 OR MIT |
| 9 | MIT/Apache-2.0 |
| 7 | BSD-3-Clause |
| 4 | Zlib OR Apache-2.0 OR MIT |
| 4 | MIT OR Apache-2.0 OR Zlib |
| 3 | BSD-2-Clause |
| 3 | Unlicense OR MIT |
| 2 | BSD-3-Clause OR Apache-2.0 |
| 2 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| 2 | ISC |
| 2 | Apache-2.0 OR BSL-1.0 OR MIT |
| 2 | CDLA-Permissive-2.0 |
| 2 | BSD-2-Clause OR Apache-2.0 OR MIT |
| 1 each | 0BSD OR MIT OR Apache-2.0, Apache-2.0/MIT, BSD-3-Clause AND MIT, BSD-3-Clause/MIT, Zlib, CC0-1.0 OR Apache-2.0, (MIT OR Apache-2.0) AND NCSA, MIT OR Zlib OR Apache-2.0, Apache-2.0 AND ISC, Apache-2.0 OR ISC OR MIT, (MIT OR Apache-2.0) AND Unicode-3.0 |

No GPL-incompatible or commercial-only dependencies. The two packages
whose license set includes `LGPL-2.1-or-later` as one of several OR
alternatives are taken under MIT or Apache-2.0 (both AGPL-compatible);
the LGPL arm is not exercised. AGPLv3 itself is compatible with every
license family in the table above.

Run `cargo metadata --format-version 1` for the live list.

## Parity reference

abgen-rs's bit-for-bit parity work was performed against the byte
output of Decentraland's `asset-bundle-converter` tool (the
deterministic-guids fork). Every fix in `dev/fix_proposals/` is
documented as an observation of bundle bytes → algorithm hypothesis →
independent reimplementation. No closed-source converter code was
read or referenced.
