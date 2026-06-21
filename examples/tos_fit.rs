use abgen::hashes::crc32;
use abgen::pathids;

fn fnv1a32(s: &[u8]) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for &b in s {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}
fn fnv1_32(s: &[u8]) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for &b in s {
        h = h.wrapping_mul(16777619);
        h ^= b as u32;
    }
    h
}
fn fnv1a64(s: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in s {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
fn djb2(s: &[u8]) -> u32 {
    let mut h: u32 = 5381;
    for &b in s {
        h = h.wrapping_mul(33).wrapping_add(b as u32);
    }
    h
}
fn djb2x(s: &[u8]) -> u32 {
    let mut h: u32 = 5381;
    for &b in s {
        h = h.wrapping_mul(33) ^ (b as u32);
    }
    h
}
fn sdbm(s: &[u8]) -> u32 {
    let mut h: u32 = 0;
    for &b in s {
        h = (b as u32)
            .wrapping_add(h << 6)
            .wrapping_add(h << 16)
            .wrapping_sub(h);
    }
    h
}
fn java31(s: &[u8]) -> u32 {
    let mut h: u32 = 0;
    for &b in s {
        h = h.wrapping_mul(31).wrapping_add(b as u32);
    }
    h
}
fn jenkins_oaat(s: &[u8]) -> u32 {
    let mut h: u32 = 0;
    for &b in s {
        h = h.wrapping_add(b as u32);
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h.wrapping_add(h << 15)
}
fn murmur3_32(s: &[u8], seed: u32) -> u32 {
    let c1: u32 = 0xcc9e2d51;
    let c2: u32 = 0x1b873593;
    let mut h = seed;
    let n = s.len() / 4;
    for i in 0..n {
        let mut k = u32::from_le_bytes(s[i * 4..i * 4 + 4].try_into().unwrap());
        k = k.wrapping_mul(c1);
        k = k.rotate_left(15);
        k = k.wrapping_mul(c2);
        h ^= k;
        h = h.rotate_left(13);
        h = h.wrapping_mul(5).wrapping_add(0xe6546b64);
    }
    let mut k: u32 = 0;
    let tail = &s[n * 4..];
    for (i, &b) in tail.iter().enumerate() {
        k ^= (b as u32) << (8 * i);
    }
    if !tail.is_empty() {
        k = k.wrapping_mul(c1);
        k = k.rotate_left(15);
        k = k.wrapping_mul(c2);
        h ^= k;
    }
    h ^= s.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85ebca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2ae35);
    h ^ (h >> 16)
}
fn xxh32(s: &[u8], seed: u32) -> u32 {
    const P1: u32 = 2654435761;
    const P2: u32 = 2246822519;
    const P3: u32 = 3266489917;
    const P4: u32 = 668265263;
    const P5: u32 = 374761393;
    let mut i = 0usize;
    let len = s.len();
    let mut h: u32;
    let rd = |i: usize| u32::from_le_bytes(s[i..i + 4].try_into().unwrap());
    if len >= 16 {
        let mut v1 = seed.wrapping_add(P1).wrapping_add(P2);
        let mut v2 = seed.wrapping_add(P2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(P1);
        while i + 16 <= len {
            v1 = v1
                .wrapping_add(rd(i).wrapping_mul(P2))
                .rotate_left(13)
                .wrapping_mul(P1);
            v2 = v2
                .wrapping_add(rd(i + 4).wrapping_mul(P2))
                .rotate_left(13)
                .wrapping_mul(P1);
            v3 = v3
                .wrapping_add(rd(i + 8).wrapping_mul(P2))
                .rotate_left(13)
                .wrapping_mul(P1);
            v4 = v4
                .wrapping_add(rd(i + 12).wrapping_mul(P2))
                .rotate_left(13)
                .wrapping_mul(P1);
            i += 16;
        }
        h = v1
            .rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
    } else {
        h = seed.wrapping_add(P5);
    }
    h = h.wrapping_add(len as u32);
    while i + 4 <= len {
        h = h
            .wrapping_add(rd(i).wrapping_mul(P3))
            .rotate_left(17)
            .wrapping_mul(P4);
        i += 4;
    }
    while i < len {
        h = h
            .wrapping_add((s[i] as u32).wrapping_mul(P5))
            .rotate_left(11)
            .wrapping_mul(P1);
        i += 1;
    }
    h ^= h >> 15;
    h = h.wrapping_mul(P2);
    h ^= h >> 13;
    h = h.wrapping_mul(P3);
    h ^ (h >> 16)
}

fn load(dir: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().map(|x| x != "tsv").unwrap_or(true) {
            continue;
        }
        let txt = std::fs::read_to_string(&p).unwrap();
        let names: Vec<String> = txt
            .lines()
            .filter_map(|l| l.split_once('\t').map(|(_, n)| n.to_string()))
            .collect();
        if names.len() > 2 {
            out.push((p.file_name().unwrap().to_string_lossy().to_string(), names));
        }
    }
    out
}

fn grouped_ok(buckets: &[u64]) -> bool {
    let mut wraps = 0;
    for i in 1..buckets.len() {
        if buckets[i] < buckets[i - 1] {
            wraps += 1;
            if wraps > 1 {
                return false;
            }
        }
    }
    if wraps == 1 {
        let first = buckets[0];
        let mut seen_wrap = false;
        for i in 1..buckets.len() {
            if buckets[i] < buckets[i - 1] {
                seen_wrap = true;
            }
            if seen_wrap && buckets[i] >= first {
                return false;
            }
        }
    }
    true
}

fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let sets = load(&dir);
    eprintln!("loaded {} orderings", sets.len());
    type HF = (&'static str, fn(&[u8]) -> u64);
    let fams: Vec<HF> = vec![
        ("crc32", |s| crc32(s) as u64),
        ("fnv1a32", |s| fnv1a32(s) as u64),
        ("fnv1_32", |s| fnv1_32(s) as u64),
        ("fnv1a64", |s| fnv1a64(s)),
        ("djb2", |s| djb2(s) as u64),
        ("djb2x", |s| djb2x(s) as u64),
        ("sdbm", |s| sdbm(s) as u64),
        ("java31", |s| java31(s) as u64),
        ("jenkins", |s| jenkins_oaat(s) as u64),
        ("murmur3_0", |s| murmur3_32(s, 0) as u64),
        ("murmur3_42", |s| murmur3_32(s, 42) as u64),
        ("xxh32_0", |s| xxh32(s, 0) as u64),
        ("spooky_lo", |s| {
            let (a, _) = pathids::spooky_short(s, 0, 0);
            a
        }),
        ("spooky_lo32", |s| {
            let (a, _) = pathids::spooky_short(s, 0, 0);
            a & 0xffff_ffff
        }),
        ("crc32_rev", |s| (crc32(s).reverse_bits()) as u64),
        ("crc32_bswap", |s| crc32(s).swap_bytes() as u64),
    ];
    for (fname, f) in fams.iter() {
        for dirn in [false, true] {
            let ok = sets.iter().all(|(_, names)| {
                let h: Vec<u64> = names.iter().map(|n| f(n.as_bytes())).collect();
                let mut s = h.clone();
                if dirn {
                    s.sort_by(|a, b| b.cmp(a));
                } else {
                    s.sort();
                }
                s == h
            });
            if ok {
                println!("FULLSORT {} desc={}", fname, dirn);
            }
        }

        for m in 2u64..=1024 {
            for dirn in [false, true] {
                for top in [false, true] {
                    let ok = sets.iter().all(|(_, names)| {
                        let b: Vec<u64> = names
                            .iter()
                            .map(|n| {
                                let h = f(n.as_bytes());
                                let v = if top {
                                    ((h & 0xffff_ffff) * m) >> 32
                                } else {
                                    h % m
                                };
                                if dirn {
                                    m - 1 - v
                                } else {
                                    v
                                }
                            })
                            .collect();
                        grouped_ok(&b)
                    });
                    if ok {
                        println!("GROUPED {} M={} desc={} top={}", fname, m, dirn, top);
                    }
                }
            }
        }
    }
    eprintln!("done");
}
