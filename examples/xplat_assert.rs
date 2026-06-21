use abgen::unity::bundle_file::Bundle;
use abgen::unity::serialized_file::SerializedFile;
use abgen::value::Value;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const TP_WINDOWS: i32 = 19;
const TP_MAC: i32 = 2;
const CLASS_ASSETBUNDLE: i32 = 142;

#[derive(Default)]
struct Stats {
    entities: usize,
    pairs_checked: usize,
    pairs_passed: usize,
    pairs_skipped: usize,
    failures: Vec<String>,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "usage: xplat_assert <windows_root> <mac_root>\n\
             roots are abgen cdn-layout / flat outputs; pairs <hash>_windows <-> <hash>_mac per entity"
        );
        std::process::exit(2);
    }
    let win_root = PathBuf::from(&args[1]);
    let mac_root = PathBuf::from(&args[2]);

    let mut stats = Stats::default();

    let win_bundles = collect_bundles(&win_root, "_windows");
    for (rel_key, win_path) in &win_bundles {

        let mac_path = sibling_mac(&mac_root, rel_key, win_path);
        match mac_path {
            Some(mp) => check_pair(rel_key, win_path, &mp, &mut stats),
            None => {
                stats.failures.push(format!(
                    "{rel_key}: no mac sibling found under {}",
                    mac_root.display()
                ));
            }
        }
    }

    let mut ent_dirs = std::collections::BTreeSet::new();
    for (k, _) in &win_bundles {
        if let Some((ent, _)) = k.rsplit_once('/') {
            ent_dirs.insert(ent.to_string());
        } else {
            ent_dirs.insert(String::new());
        }
    }
    stats.entities = ent_dirs.len();

    println!("=== xplat_assert ===");
    println!("windows root: {}", win_root.display());
    println!("mac root:     {}", mac_root.display());
    println!(
        "entities: {}  pairs: {}  passed: {}  skipped(shader): {}  FAILED: {}",
        stats.entities,
        stats.pairs_checked,
        stats.pairs_passed,
        stats.pairs_skipped,
        stats.failures.len()
    );
    if !stats.failures.is_empty() {
        println!("\n--- FAILURES ---");
        for f in &stats.failures {
            println!("FAIL {f}");
        }
        std::process::exit(1);
    }
    println!("\nALL DIFFERENCES EXPLAINED BY THE KNOWN PLATFORM TRANSFORM. PASS.");
}

fn collect_bundles(root: &Path, suffix: &str) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    walk(root, root, suffix, &mut out);
    out.sort();
    out
}

fn walk(base: &Path, dir: &Path, suffix: &str, out: &mut Vec<(String, PathBuf)>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(base, &p, suffix, out);
        } else if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
            if let Some(stem) = name.strip_suffix(suffix) {
                let rel = p.strip_prefix(base).unwrap_or(&p);
                let rel_dir = rel.parent().map(|d| d.to_string_lossy().into_owned());
                let key = match rel_dir {
                    Some(d) if !d.is_empty() => format!("{d}/{stem}"),
                    _ => stem.to_string(),
                };
                out.push((key, p.clone()));
            }
        }
    }
}

fn sibling_mac(mac_root: &Path, rel_key: &str, win_path: &Path) -> Option<PathBuf> {

    let cand = mac_root.join(format!("{rel_key}_mac"));
    if cand.is_file() {
        return Some(cand);
    }

    if let Some(d) = win_path.parent() {
        let stem = rel_key.rsplit('/').next().unwrap_or(rel_key);
        let cand2 = d.join(format!("{stem}_mac"));
        if cand2.is_file() {
            return Some(cand2);
        }
    }
    None
}

fn is_shader_bundle(rel_key: &str) -> bool {
    rel_key.contains("scene_ignore")
}

fn check_pair(rel_key: &str, win_path: &Path, mac_path: &Path, stats: &mut Stats) {
    if is_shader_bundle(rel_key) {

        stats.pairs_skipped += 1;
        return;
    }
    stats.pairs_checked += 1;

    let win = match Bundle::load(win_path) {
        Ok(b) => b,
        Err(e) => {
            stats
                .failures
                .push(format!("{rel_key}: load windows: {e}"));
            return;
        }
    };
    let mac = match Bundle::load(mac_path) {
        Ok(b) => b,
        Err(e) => {
            stats.failures.push(format!("{rel_key}: load mac: {e}"));
            return;
        }
    };

    let wsf = match win.serialized() {
        Some(s) => s,
        None => {
            stats
                .failures
                .push(format!("{rel_key}: windows has no SerializedFile"));
            return;
        }
    };
    let msf = match mac.serialized() {
        Some(s) => s,
        None => {
            stats
                .failures
                .push(format!("{rel_key}: mac has no SerializedFile"));
            return;
        }
    };

    if let Err(reason) = compare_serialized(wsf, msf) {
        stats.failures.push(format!("{rel_key}: {reason}"));
        return;
    }
    stats.pairs_passed += 1;
}

fn norm_str(s: &str) -> String {
    let s = s.replace("_windows", "").replace("_mac", "");

    collapse_cab_tokens(&s)
}

fn collapse_cab_tokens(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        let lower_match = i + 4 <= bytes.len() && bytes[i..i + 4].eq_ignore_ascii_case(b"CAB-");
        if lower_match {
            let mut j = i + 4;
            while j < bytes.len() && bytes[j].is_ascii_hexdigit() {
                j += 1;
            }
            if j - (i + 4) >= 16 {
                out.push_str("CAB-#");
                i = j;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn normalize_cab_bytes(data: &[u8]) -> Vec<u8> {
    const PLACEHOLDER: &[u8; 36] = b"CAB-############################ABCD";
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if i + 36 <= data.len()
            && data[i..i + 4].eq_ignore_ascii_case(b"CAB-")
            && data[i + 4..i + 36].iter().all(|b| b.is_ascii_hexdigit())
        {
            out.extend_from_slice(PLACEHOLDER);
            i += 36;
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    out
}

fn values_equal_normalized(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Str(x), Value::Str(y)) => norm_str(x) == norm_str(y),
        (Value::Bytes(x), Value::Bytes(y)) => normalize_cab_bytes(x) == normalize_cab_bytes(y),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|(p, q)| values_equal_normalized(p, q))
        }
        (Value::Map(x), Value::Map(y)) => {
            x.0.len() == y.0.len()
                && x.0.iter().zip(y.0.iter()).all(|((kx, vx), (ky, vy))| {
                    kx.as_str() == ky.as_str() && values_equal_normalized(vx, vy)
                })
        }
        _ => a == b,
    }
}

fn cab_for(sf: &SerializedFile, file_id: i64) -> String {
    if file_id == 0 {
        return "INTERNAL".to_string();
    }
    let idx = (file_id - 1) as usize;
    match sf.externals.get(idx) {
        Some(fi) => norm_str(&fi.path),
        None => format!("EXT#{file_id}"),
    }
}

fn compare_serialized(w: &SerializedFile, m: &SerializedFile) -> Result<(), String> {

    if w.target_platform != TP_WINDOWS {
        return Err(format!(
            "windows target_platform={} (expected {})",
            w.target_platform, TP_WINDOWS
        ));
    }
    if m.target_platform != TP_MAC {
        return Err(format!(
            "mac target_platform={} (expected {})",
            m.target_platform, TP_MAC
        ));
    }
    if w.unity_version != m.unity_version {
        return Err(format!(
            "unityVersion skew: win={} mac={}",
            w.unity_version, m.unity_version
        ));
    }

    let mut we: Vec<String> = w.externals.iter().map(|e| norm_str(&e.path)).collect();
    let mut me: Vec<String> = m.externals.iter().map(|e| norm_str(&e.path)).collect();
    we.sort();
    me.sort();
    if we != me {
        return Err(format!(
            "externals multiset differs (normalized): win={we:?} mac={me:?}"
        ));
    }

    let wmap: BTreeMap<i64, &abgen::unity::serialized_file::Object> =
        w.objects.iter().map(|o| (o.path_id, o)).collect();
    let mmap: BTreeMap<i64, &abgen::unity::serialized_file::Object> =
        m.objects.iter().map(|o| (o.path_id, o)).collect();

    let wkeys: Vec<i64> = wmap.keys().copied().collect();
    let mkeys: Vec<i64> = mmap.keys().copied().collect();
    if wkeys != mkeys {
        return Err(format!(
            "object path_id set differs: win has {} objs, mac has {} objs",
            wkeys.len(),
            mkeys.len()
        ));
    }

    for (&pid, wo) in &wmap {
        let mo = mmap[&pid];
        if wo.class_id != mo.class_id {
            return Err(format!(
                "object {pid}: class_id win={} mac={}",
                wo.class_id, mo.class_id
            ));
        }
        if wo.class_id == CLASS_ASSETBUNDLE {

            continue;
        }

        let wn = normalize_cab_bytes(&wo.data);
        let mn = normalize_cab_bytes(&mo.data);
        if wn != mn {

            let wv = w.read_typetree(wo);
            let mv = m.read_typetree(mo);
            let explained = match (wv, mv) {
                (Ok(wv), Ok(mv)) => values_equal_normalized(&wv, &mv),
                _ => false,
            };
            if !explained {
                let off = wn.iter().zip(mn.iter()).position(|(a, b)| a != b);
                return Err(format!(
                    "object {pid} ({}) bytes DIFFER (win {}B / mac {}B, first diff offset {:?}) \
                     -- UNEXPLAINED platform divergence",
                    wo.type_name,
                    wo.data.len(),
                    mo.data.len(),
                    off
                ));
            }
        }
    }

    let wab = wmap
        .values()
        .find(|o| o.class_id == CLASS_ASSETBUNDLE)
        .copied();
    let mab = mmap
        .values()
        .find(|o| o.class_id == CLASS_ASSETBUNDLE)
        .copied();
    match (wab, mab) {
        (Some(wo), Some(mo)) => {
            let wv = w
                .read_typetree(wo)
                .map_err(|e| format!("read windows AssetBundle typetree: {e}"))?;
            let mv = m
                .read_typetree(mo)
                .map_err(|e| format!("read mac AssetBundle typetree: {e}"))?;
            compare_assetbundle(&wv, &mv, w, m)?;
        }
        (None, None) => {}
        _ => return Err("AssetBundle asset present on only one platform".to_string()),
    }

    Ok(())
}

fn pptr_key(sf: &SerializedFile, v: &Value) -> (String, i64) {
    let fid = v.get("m_FileID").and_then(|x| x.as_i64()).unwrap_or(0);
    let pid = v.get("m_PathID").and_then(|x| x.as_i64()).unwrap_or(0);
    (cab_for(sf, fid), pid)
}

fn compare_assetbundle(
    wv: &Value,
    mv: &Value,
    w: &SerializedFile,
    m: &SerializedFile,
) -> Result<(), String> {

    for key in ["m_Name", "m_AssetBundleName"] {
        let ws = wv.get(key).and_then(|x| x.as_str()).map(norm_str);
        let ms = mv.get(key).and_then(|x| x.as_str()).map(norm_str);
        if ws != ms {
            return Err(format!(
                "AssetBundle {key} differs after normalize: win={ws:?} mac={ms:?}"
            ));
        }
    }

    let mut wpre = preload_multiset(wv, w)?;
    let mut mpre = preload_multiset(mv, m)?;
    wpre.sort();
    mpre.sort();
    if wpre != mpre {
        let only_w: Vec<_> = wpre.iter().filter(|x| !mpre.contains(x)).collect();
        let only_m: Vec<_> = mpre.iter().filter(|x| !wpre.contains(x)).collect();
        return Err(format!(
            "m_PreloadTable multiset differs: only-in-win={only_w:?} only-in-mac={only_m:?}"
        ));
    }

    let mut wdep = str_set(wv, "m_Dependencies");
    let mut mdep = str_set(mv, "m_Dependencies");
    wdep.sort();
    mdep.sort();
    if wdep != mdep {
        return Err(format!(
            "m_Dependencies differ (normalized): win={wdep:?} mac={mdep:?}"
        ));
    }

    let wcont = container_map(wv, w)?;
    let mcont = container_map(mv, m)?;
    if wcont != mcont {
        let mut diffs = Vec::new();
        for (k, v) in &wcont {
            match mcont.get(k) {
                Some(mvv) if mvv == v => {}
                Some(mvv) => diffs.push(format!("{k}: win={v:?} mac={mvv:?}")),
                None => diffs.push(format!("{k}: only in win")),
            }
        }
        for k in mcont.keys() {
            if !wcont.contains_key(k) {
                diffs.push(format!("{k}: only in mac"));
            }
        }
        return Err(format!("m_Container differs: {}", diffs.join("; ")));
    }

    Ok(())
}

fn preload_multiset(v: &Value, sf: &SerializedFile) -> Result<Vec<(String, i64)>, String> {
    let arr = v
        .get("m_PreloadTable")
        .and_then(|x| x.as_array())
        .ok_or("AssetBundle missing m_PreloadTable array")?;
    Ok(arr.iter().map(|p| pptr_key(sf, p)).collect())
}

fn str_set(v: &Value, key: &str) -> Vec<String> {
    match v.get(key).and_then(|x| x.as_array()) {
        Some(arr) => arr
            .iter()
            .filter_map(|x| x.as_str())
            .map(norm_str)
            .collect(),
        None => Vec::new(),
    }
}

fn container_map(
    v: &Value,
    sf: &SerializedFile,
) -> Result<BTreeMap<String, (i64, String, i64)>, String> {
    let arr = v
        .get("m_Container")
        .and_then(|x| x.as_array())
        .ok_or("AssetBundle missing m_Container array")?;
    let mut out = BTreeMap::new();
    for e in arr {
        let pair = e.as_array().ok_or("m_Container entry not a pair")?;
        if pair.len() != 2 {
            return Err("m_Container entry not length 2".to_string());
        }
        let name = norm_str(pair[0].as_str().unwrap_or(""));
        let slot = &pair[1];
        let size = slot
            .get("preloadSize")
            .and_then(|x| x.as_i64())
            .unwrap_or(-1);
        let asset = slot.get("asset");
        let (acab, apid) = match asset {
            Some(a) => pptr_key(sf, a),
            None => ("NONE".to_string(), 0),
        };
        out.insert(name, (size, acab, apid));
    }
    Ok(out)
}
