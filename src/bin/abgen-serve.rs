
use abgen::builder::{build_bundle, BuildOpts};
use abgen::catalyst::{CatalystClient, Scene, DEFAULT_CATALYST};
use abgen::glbscan::{scan_entity, EntityScan, UriCache};
use abgen::local_store::LocalContentStore;
use abgen::space::Space;
use abgen::{anyhow, bail, naming, Context, Result};
use rayon::prelude::*;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

const CONVERTIBLE_EXTS: [&str; 5] = [".glb", ".gltf", ".png", ".jpg", ".jpeg"];

struct BuildTelemetry<'a> {
    entity: &'a str,
    entity_type: &'a str,
    file: &'a str,
    platform: &'a str,
    hash: &'a str,
    ms: u64,
    out_bytes: usize,

    result: &'a str,
}

fn emit_build_telemetry(t: &BuildTelemetry) {

    let rec = serde_json::json!({
        "entity": t.entity,
        "entity_type": t.entity_type,
        "file": t.file,
        "platform": t.platform,
        "hash": t.hash,
        "build_ms": t.ms,
        "out_bytes": t.out_bytes,
        "result": t.result,
    });
    eprintln!("ABGEN_BUILD {rec}");
}

fn is_convertible(file: &str) -> (bool, bool) {
    let fl = file.to_lowercase();
    let is_glb = fl.ends_with(".glb") || fl.ends_with(".gltf");
    let is_image = fl.ends_with(".png") || fl.ends_with(".jpg") || fl.ends_with(".jpeg");
    (is_glb, is_image)
}

#[derive(Default)]
struct KeyedLocks {
    map: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl KeyedLocks {
    fn get(&self, key: &str) -> Arc<Mutex<()>> {
        let mut g = self.map.lock().unwrap();
        g.entry(key.to_string()).or_default().clone()
    }
}

struct EntityCtx {
    scene: Scene,
    content_by_file: HashMap<String, String>,
    scan: EntityScan,
}

struct Proxy {
    catalyst: CatalystClient,
    local: Option<LocalContentStore>,
    content: LocalContentStore,
    bundle_dir: PathBuf,
    version: String,
    date: String,
    uri_cache: UriCache,

    space: Option<Arc<Space>>,
    fallback_version: String,
    timeout: Duration,
    cache_cap: u64,

    entities: Mutex<HashMap<String, Arc<EntityCtx>>>,
    hash_index: Mutex<HashMap<String, String>>,
    entity_locks: KeyedLocks,
    bundle_locks: KeyedLocks,
}

impl Proxy {

    fn ensure_content(&self, hash: &str) -> Result<()> {
        if self.content.exists(hash) {
            return Ok(());
        }
        if let Some(local) = &self.local {
            if let Ok(b) = local.fetch(hash) {
                return self.content.write(hash, &b);
            }
        }
        let bytes = self
            .catalyst
            .fetch_content(hash)
            .with_context(|| format!("fetch content {hash}"))?;
        self.content.write(hash, &bytes)
    }

    fn entity_ctx(&self, cid: &str) -> Result<Arc<EntityCtx>> {
        if let Some(c) = self.entities.lock().unwrap().get(cid) {
            return Ok(c.clone());
        }
        let lock = self.entity_locks.get(cid);
        let _g = lock.lock().unwrap();
        if let Some(c) = self.entities.lock().unwrap().get(cid) {
            return Ok(c.clone());
        }

        let scene = self
            .catalyst
            .resolve_scene(cid)
            .with_context(|| format!("resolve entity {cid}"))?;

        for c in &scene.content {
            if CONVERTIBLE_EXTS
                .iter()
                .any(|e| c.file.to_lowercase().ends_with(*e))
            {
                if let Err(e) = self.ensure_content(&c.hash) {
                    eprintln!("warn: {cid}: content {} ({}): {e}", c.hash, c.file);
                }
            }
        }

        let content_by_file = scene.content_by_file();
        let scan = scan_entity(&self.content, &content_by_file, &self.uri_cache);

        {
            let mut idx = self.hash_index.lock().unwrap();
            for c in &scene.content {
                idx.entry(c.hash.to_lowercase())
                    .or_insert_with(|| cid.to_string());
            }
        }

        let ctx = Arc::new(EntityCtx {
            scene,
            content_by_file,
            scan,
        });
        self.entities
            .lock()
            .unwrap()
            .insert(cid.to_string(), ctx.clone());
        Ok(ctx)
    }

    fn bundle(&self, cid: &str, bundle_name: &str) -> Result<Vec<u8>> {
        // The disk cache is scoped per-entity. A GLB bundle bakes in its texture
        // dependencies *by content hash*, and a GLB shared verbatim across scenes
        // can reference textures that were deployed under different hashes in each
        // scene — so the bundle bytes (and the dep hashes a client then fetches)
        // are entity-specific. Keying only by bundle_name served one scene's baked
        // deps to every other scene, producing "hash not in entity" 500s and
        // breaking dependency loads. This mirrors the per-entity remote-space key
        // (see bundle_key / put_generated).
        let entity_dir = self.bundle_dir.join(cid);
        let cache_path = entity_dir.join(bundle_name);
        if let Ok(b) = std::fs::read(&cache_path) {
            return Ok(b);
        }
        let lock = self.bundle_locks.get(&format!("{cid}/{bundle_name}"));
        let _g = lock.lock().unwrap();
        if let Ok(b) = std::fs::read(&cache_path) {
            return Ok(b);
        }

        let ctx = self.entity_ctx(cid)?;
        let data = self.build(&ctx, bundle_name)?;

        std::fs::create_dir_all(&entity_dir).ok();
        let tmp = cache_path.with_extension(format!("tmp.{}", std::process::id()));
        std::fs::write(&tmp, &data).with_context(|| format!("write {}", tmp.display()))?;
        std::fs::rename(&tmp, &cache_path).ok();
        Ok(data)
    }

    fn build(&self, ctx: &EntityCtx, bundle_name: &str) -> Result<Vec<u8>> {
        let (hash, platform) = bundle_name
            .rsplit_once('_')
            .ok_or_else(|| anyhow!("bundle name {bundle_name:?} has no _<platform> suffix"))?;

        let item = match ctx
            .scene
            .content
            .iter()
            .find(|c| c.hash.eq_ignore_ascii_case(hash))
        {
            Some(it) => it,
            None => {
                // For v41 the client builds dependency-bundle URLs as
                // /v41/<parentScene>/<depHash> (HasHashInPath), and a dep can be a
                // cross-entity / shared texture whose true owner != the requesting
                // scene. Re-resolve to the owning entity (the same mechanism the
                // flat 2-part path already uses) and build the bundle in *its*
                // context, where the hash and its color-space/normal classification
                // live. The result is content-addressed by hash, so this returns
                // exactly the bundle the client asked for instead of hard-erroring.
                if let Some(owner) = self.entity_for_hash(hash) {
                    if !owner.eq_ignore_ascii_case(&ctx.scene.entity_id) {
                        let owner_ctx = self.entity_ctx(&owner)?;
                        return self.build(&owner_ctx, bundle_name);
                    }
                }
                bail!(
                    "hash {hash} not in entity {} (no owning entity indexed)",
                    ctx.scene.entity_id
                );
            }
        };
        let hash: &str = &item.hash;
        let file = item.file.clone();
        let (is_glb, is_image) = is_convertible(&file);
        if !is_glb && !is_image {
            bail!("content {file} (hash {hash}) is not a convertible glb/image");
        }

        self.ensure_content(hash)?;
        let glb = self.content.fetch(hash)?;

        let m_deps = if is_glb {
            ctx.scan
                .metadata_deps(&self.content, &file, hash, &ctx.content_by_file, platform)
        } else {
            Vec::new()
        };
        let model_ref = is_image && ctx.scan.model_refs.contains(hash);
        let standalone_color_space = if is_image {
            Some(if ctx.scan.linear_refs.contains(hash) {
                0
            } else {
                1
            })
        } else {
            None
        };
        let standalone_normal = is_image && ctx.scan.normal_refs.contains(hash);

        let content_by_file = &ctx.content_by_file;
        let store = &self.content;
        let sf_bytes = file.clone();
        let resolve_fn = |uri: &str| -> Option<Vec<u8>> {
            let key = naming::resolve_uri_to_content_file(uri, &sf_bytes)
                .ok()?
                .to_lowercase();
            let h = content_by_file.get(&key)?;
            store.fetch(h).ok()
        };
        let resolve: abgen::gltf::Resolve = if !content_by_file.is_empty() {
            Some(&resolve_fn)
        } else {
            None
        };
        let sf_hash = file.clone();
        let resolve_hash_fn = |uri: &str| -> Option<String> {
            let key = naming::resolve_uri_to_content_file(uri, &sf_hash)
                .ok()?
                .to_lowercase();
            content_by_file.get(&key).cloned()
        };
        type HashResolver<'a> = &'a dyn Fn(&str) -> Option<String>;
        let resolve_hash: Option<HashResolver<'_>> = if !content_by_file.is_empty() {
            Some(&resolve_hash_fn)
        } else {
            None
        };

        let entity_type = ctx.scene.entity_type.clone();
        let opts = BuildOpts {
            keep_forward_plus: true,
            source_file: Some(&file),
            entity_type: if entity_type.is_empty() {
                None
            } else {
                Some(entity_type.as_str())
            },
            resolve,
            resolve_hash,
            model_referenced: model_ref,
            metadata_dependencies: &m_deps,
            expect_hash: None,
            standalone_color_space,
            standalone_normal,
            force_default_material: false,
            magenta_missing: std::env::var("ABGEN_MAGENTA_MISSING").is_ok(),
        };

        let started = std::time::Instant::now();
        let outcome = abgen::regen::guard(|| build_bundle(&glb, bundle_name, hash, &opts));
        let ms = started.elapsed().as_millis() as u64;

        let (result_label, out_bytes) = match &outcome {
            Ok(a) => ("ok", a.data.len()),
            Err(e) => {
                if e.to_string().starts_with("panic:") {
                    ("panic-recovered", 0usize)
                } else {
                    ("error", 0usize)
                }
            }
        };
        emit_build_telemetry(&BuildTelemetry {
            entity: &ctx.scene.entity_id,
            entity_type: &entity_type,
            file: &file,
            platform,
            hash,
            ms,
            out_bytes,
            result: result_label,
        });

        let artifact = outcome?;
        Ok(artifact.data)
    }

    fn entity_for_hash(&self, hash: &str) -> Option<String> {
        self.hash_index.lock().unwrap().get(&hash.to_lowercase()).cloned()
    }

    fn bundle_key(version: &str, cid: &str, file: &str) -> String {
        format!("{version}/{cid}/{file}")
    }

    fn fetch_fallback(&self, cid: &str, file: &str) -> Option<Vec<u8>> {
        let space = self.space.as_ref()?;
        let key = Self::bundle_key(&self.fallback_version, cid, file);
        match space.get(&key) {
            Ok(Some(b)) => Some(b),
            Ok(None) => None,
            Err(e) => {
                eprintln!("fallback {key}: {e}");
                None
            }
        }
    }

    fn put_generated(&self, cid: &str, file: &str, bytes: &[u8]) {
        let Some(space) = self.space.as_ref() else { return };
        let key = Self::bundle_key(&self.version, cid, file);
        match space.put(&key, bytes, "application/octet-stream") {
            Ok(()) => eprintln!("space put {key} ({} bytes)", bytes.len()),
            Err(e) => eprintln!("put {key}: {e}"),
        }
        if let Some((_, platform)) = file.rsplit_once('_') {
            let mkey = format!("manifest/{cid}_{platform}.json");
            let body =
                serde_json::json!({"version": self.version, "date": self.date}).to_string();
            if let Err(e) = space.put(&mkey, body.as_bytes(), "application/json") {
                eprintln!("put {mkey}: {e}");
            }
        }
    }

    fn enforce_lru(&self) {
        if self.cache_cap == 0 {
            return;
        }
        // Bundles live one level deep now: <bundle_dir>/<entity>/<bundle_name>.
        let Ok(entities) = std::fs::read_dir(&self.bundle_dir) else {
            return;
        };
        let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = Vec::new();
        for ent in entities.filter_map(|e| e.ok()) {
            let Ok(rd) = std::fs::read_dir(ent.path()) else {
                continue;
            };
            for e in rd.filter_map(|e| e.ok()) {
                let Ok(m) = e.metadata() else { continue };
                if !m.is_file() {
                    continue;
                }
                files.push((
                    m.modified().unwrap_or(std::time::UNIX_EPOCH),
                    m.len(),
                    e.path(),
                ));
            }
        }
        let mut total: u64 = files.iter().map(|(_, l, _)| *l).sum();
        if total <= self.cache_cap {
            return;
        }
        files.sort_by_key(|(t, _, _)| *t);
        for (_, len, path) in files {
            if total <= self.cache_cap {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(len);
            }
        }
    }

    fn serve_or_fallback(self: &Arc<Self>, cid: &str, file: &str) -> (u16, Vec<u8>, &'static str) {
        if let Ok(b) = std::fs::read(self.bundle_dir.join(cid).join(file)) {
            return (200, b, "cache");
        }
        let (tx, rx) = mpsc::channel();
        let me = self.clone();
        let cid_t = cid.to_string();
        let file_t = file.to_string();
        std::thread::spawn(move || {
            let r = me.bundle(&cid_t, &file_t);
            if let Ok(bytes) = &r {
                me.put_generated(&cid_t, &file_t, bytes);
                me.enforce_lru();
            }
            let _ = tx.send(r.map_err(|e| format!("{e:#}")));
        });
        match rx.recv_timeout(self.timeout) {
            Ok(Ok(bytes)) => (200, bytes, "fresh"),
            Ok(Err(e)) => match self.fetch_fallback(cid, file) {
                Some(b) => (200, b, "fallback(build-failed)"),
                None => (500, format!("build failed: {e}").into_bytes(), "error"),
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(b) = self.fetch_fallback(cid, file) {
                    (200, b, "fallback")
                } else {
                    match rx.recv() {
                        Ok(Ok(b)) => (200, b, "fresh-slow"),
                        Ok(Err(e)) => (500, format!("build failed: {e}").into_bytes(), "error"),
                        Err(_) => (500, b"build channel closed".to_vec(), "error"),
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                (500, b"build worker died".to_vec(), "error")
            }
        }
    }
}

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_request(stream: &TcpStream) -> Result<Option<Request>> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let mut it = line.split_whitespace();
    let method = it.next().unwrap_or("").to_string();
    let path = it.next().unwrap_or("/").to_string();
    if method.is_empty() {
        return Ok(None);
    }

    let mut content_length = 0usize;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 {
            break;
        }
        let t = h.trim_end();
        if t.is_empty() {
            break;
        }
        if let Some((k, v)) = t.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                content_length = v.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }
    Ok(Some(Request { method, path, body }))
}

fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        204 => "No Content",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

fn respond(stream: &mut TcpStream, code: u16, ctype: &str, body: &[u8], head: bool) {
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        code,
        reason(code),
        ctype,
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    if !head {
        let _ = stream.write_all(body);
    }
    let _ = stream.flush();
}

fn handle(proxy: &Arc<Proxy>, stream: &mut TcpStream) {
    stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
    let req = match read_request(stream) {
        Ok(Some(r)) => r,
        Ok(None) => return,
        Err(_) => return,
    };
    let head = req.method == "HEAD";
    let raw_path = req.path.split('?').next().unwrap_or("/");
    let parts: Vec<&str> = raw_path.trim_matches('/').split('/').collect();

    if req.method == "OPTIONS" {
        return respond(stream, 204, "text/plain", b"", head);
    }

    if req.method == "POST" && raw_path.trim_matches('/').ends_with("entities/versions") {
        let pointers: Vec<String> = serde_json::from_slice::<serde_json::Value>(&req.body)
            .ok()
            .and_then(|v| {
                v.get("pointers").and_then(|p| p.as_array()).map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
            })
            .unwrap_or_default();

        let pairs: Vec<(String, String)> = pointers
            .par_iter()
            .flat_map(|p| {
                proxy
                    .catalyst
                    .resolve_scene(p)
                    .map(|s| {
                        s.content
                            .iter()
                            .map(|c| (c.hash.to_lowercase(), s.entity_id.clone()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect();
        let indexed = {
            let mut idx = proxy.hash_index.lock().unwrap();
            let before = idx.len();
            for (h, e) in pairs {
                idx.entry(h).or_insert(e);
            }
            idx.len() - before
        };
        let ver = serde_json::json!({"version": proxy.version, "buildDate": proxy.date});
        let mut assets = serde_json::Map::new();
        for p in &pointers {
            assets.insert(p.clone(), ver.clone());
        }
        let resp = serde_json::json!({
            "pointers": pointers,
            "versions": {"assets": {"windows": ver, "mac": ver, "linux": ver, "webgl": ver}},
            "bundles": {"assets": serde_json::Value::Object(assets)},
        });
        eprintln!(
            "POST entities/versions ({} pointers, +{indexed} hashes indexed)",
            pointers.len()
        );
        return respond(
            stream,
            200,
            "application/json",
            resp.to_string().as_bytes(),
            head,
        );
    }

    if req.method != "GET" && !head {
        return respond(stream, 404, "text/plain", b"not found", head);
    }

    if parts.len() == 2 && parts[0] == "manifest" && parts[1].ends_with(".json") {
        let stem = &parts[1][..parts[1].len() - 5];
        let entity = stem.rsplit_once('_').map(|(e, _)| e).unwrap_or(stem);
        match proxy.entity_ctx(entity) {
            Ok(_) => {
                let body = serde_json::json!({"version": proxy.version, "date": proxy.date});

                if let Some(space) = proxy.space.as_ref() {
                    let key = format!("manifest/{stem}.json");
                    let _ = space.put(&key, body.to_string().as_bytes(), "application/json");
                }
                eprintln!("manifest {entity} -> warmed");
                respond(
                    stream,
                    200,
                    "application/json",
                    body.to_string().as_bytes(),
                    head,
                )
            }
            Err(e) => {

                if let Some(space) = proxy.space.as_ref() {
                    let key = format!("manifest/{stem}.json");
                    if let Ok(Some(b)) = space.get(&key) {
                        eprintln!("manifest {entity} -> fallback from space");
                        return respond(stream, 200, "application/json", &b, head);
                    }
                }
                eprintln!("manifest {entity}: {e:#}");
                respond(
                    stream,
                    404,
                    "application/json",
                    b"{\"error\":\"unknown entity\"}",
                    head,
                )
            }
        }
        return;
    }

    let (cid, file): (Option<String>, &str) = match parts.as_slice() {
        [_v, entity, f] => (Some((*entity).to_string()), *f),
        [_v, f] => {
            let f: &str = f;
            let hash = f.rsplit_once('_').map(|(h, _)| h).unwrap_or(f);
            (proxy.entity_for_hash(hash), f)
        }
        _ => return respond(stream, 404, "text/plain", b"not found", head),
    };

    let Some(cid) = cid else {
        eprintln!("GET {raw_path}: no entity known for hash (flat); client should fall back");
        return respond(stream, 404, "text/plain", b"unknown asset", head);
    };

    let (code, data, src) = proxy.serve_or_fallback(&cid, file);

    eprintln!("GET {cid}/{file} -> {code} [{src}] {} bytes", data.len());
    let platform = file.rsplit_once('_').map(|(_, p)| p).unwrap_or("");
    eprintln!(
        "ABGEN_SERVE {}",
        serde_json::json!({
            "entity": cid,
            "file": file,
            "platform": platform,
            "code": code,
            "src": src,
            "out_bytes": data.len(),
        })
    );
    let ctype = if code == 200 {
        "application/octet-stream"
    } else {
        "text/plain"
    };
    respond(stream, code, ctype, &data, head);
}

fn build_id() -> String {
    let mut buf: Vec<u8> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(b) = std::fs::read(&exe) {
            buf.extend_from_slice(&b);
        }
    }
    if let Ok(rd) = std::fs::read_dir(abgen::builder::template_dir()) {
        let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.sort();
        for f in files {
            if let Ok(b) = std::fs::read(&f) {
                buf.extend_from_slice(f.to_string_lossy().as_bytes());
                buf.extend_from_slice(&b);
            }
        }
    }
    abgen::hashes::sha256_hex(&buf)
}

fn iso_from_build_id(id: &str) -> String {
    let n = u64::from_str_radix(id.get(..8).unwrap_or("0"), 16).unwrap_or(0);
    let base = 1_577_836_800u64;
    iso8601_utc(base + (n % 946_080_000))
}

fn iso8601_utc(total_secs: u64) -> String {
    let days = (total_secs / 86_400) as i64;
    let sod = (total_secs % 86_400) as i64;
    let (h, mi, s) = (sod / 3600, (sod % 3600) / 60, sod % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.000Z")
}

const fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn usage() -> ! {
    eprintln!(
        "usage: abgen-serve [--catalyst <url>] [--local <snapshot-root>] [--cache <dir>]\n       \
         [--host <ip>] [--port <n>] [--version v<int>] [--date <iso>] [--parity]\n\n\
         Live-translate asset-bundle proxy for unity-explorer. On a cache miss it\n  \
         resolves the entity from --catalyst, fetches content, converts with abgen,\n  \
         caches under --cache, and serves the DCL AB-CDN routes.\n\n  \
         --catalyst  content server base (default {DEFAULT_CATALYST})\n  \
         --local     optional local snapshot read first before the network\n  \
         --cache     on-disk cache root (default ./abgen-serve-cache)\n  \
         --port      listen port (default 5185)\n  \
         --parity    emit fork-byte-faithful bundles (texture stubs) instead of\n              \
                     the default serving-correct --real-textures/--v38-compat\n\n\
         point the client: --lsd-use-remote-ab --lsd-remote-ab-server http://<host>:<port>"
    );
    std::process::exit(2);
}

fn main() {
    let mut catalyst = DEFAULT_CATALYST.to_string();
    let mut local: Option<String> = None;
    let mut cache = "./abgen-serve-cache".to_string();
    let mut host = "127.0.0.1".to_string();
    let mut port: u16 = 5185;
    let mut version = "v41".to_string();
    let mut date: Option<String> = None;
    let mut parity = false;
    let mut fallback_version = "v41".to_string();
    let mut use_space = false;
    let mut timeout_ms: u64 = 1000;
    let mut cache_cap_gb: f64 = 0.0;
    let mut magenta_missing = false;

    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let next = |i: &mut usize| -> String {
            *i += 1;
            argv.get(*i).cloned().unwrap_or_else(|| usage())
        };
        match argv[i].as_str() {
            "--catalyst" => catalyst = next(&mut i),
            "--local" => local = Some(next(&mut i)),
            "--cache" => cache = next(&mut i),
            "--host" => host = next(&mut i),
            "--port" => port = next(&mut i).parse().unwrap_or_else(|_| usage()),
            "--version" => version = next(&mut i),
            "--date" => date = Some(next(&mut i)),
            "--parity" => parity = true,
            "--space" => use_space = true,
            "--magenta-missing" => magenta_missing = true,
            "--fallback-version" => fallback_version = next(&mut i),
            "--timeout-ms" => timeout_ms = next(&mut i).parse().unwrap_or_else(|_| usage()),
            "--cache-cap-gb" => cache_cap_gb = next(&mut i).parse().unwrap_or_else(|_| usage()),
            "-h" | "--help" => usage(),
            other => {
                eprintln!("unknown argument: {other}");
                usage();
            }
        }
        i += 1;
    }

    if !parity {

        std::env::set_var(BuildOpts::REAL_TEXTURES_ENV, "1");
        std::env::set_var(BuildOpts::V38_COMPAT_ENV, "1");
    }
    // --magenta-missing: render broken/missing textures as magenta placeholders
    // (read back per-build in the opts below). Off by default → live serve
    // behaviour is unchanged unless the flag is passed.
    if magenta_missing {
        std::env::set_var("ABGEN_MAGENTA_MISSING", "1");
    }

    let bid = build_id();
    let date = date.unwrap_or_else(|| iso_from_build_id(&bid));

    let turbojpeg_ok = abgen::ffi::turbojpeg_available();
    eprintln!(
        "ABGEN_CAP {}",
        serde_json::json!({
            "turbojpeg": turbojpeg_ok,
            "build": &bid[..16.min(bid.len())],
            "version": version,
        })
    );
    if !turbojpeg_ok {
        eprintln!(
            "warn: libturbojpeg NOT loadable -> JPEG decode falls back (parity/quality \
             degradation for ~300 textured bundles). Run in an FHS env or set TURBOJPEG_LIB."
        );
    }

    let cache_root = PathBuf::from(&cache);

    let content = LocalContentStore::new(cache_root.join("content"));

    let bundle_dir = cache_root.join("bundles").join(&bid[..16.min(bid.len())]);
    if let Err(e) = std::fs::create_dir_all(&bundle_dir) {
        eprintln!("error: create cache {}: {e}", bundle_dir.display());
        std::process::exit(1);
    }

    let space = if use_space {
        match Space::from_env() {
            Some(s) => {
                eprintln!("space: {} (gen={version} fallback={fallback_version})", s.host);
                Some(Arc::new(s))
            }
            None => {
                eprintln!("warning: --space set but S3 credentials missing (ABGEN_S3_ACCESS_KEY/SECRET_KEY); disabled");
                None
            }
        }
    } else {
        None
    };

    let proxy = Arc::new(Proxy {
        catalyst: CatalystClient::new(&catalyst),
        local: local.map(LocalContentStore::new),
        content,
        bundle_dir: bundle_dir.clone(),
        version: version.clone(),
        date: date.clone(),
        uri_cache: UriCache::new(),
        space,
        fallback_version: fallback_version.clone(),
        timeout: Duration::from_millis(timeout_ms),
        cache_cap: (cache_cap_gb * 1e9) as u64,
        entities: Mutex::new(HashMap::new()),
        hash_index: Mutex::new(HashMap::new()),
        entity_locks: KeyedLocks::default(),
        bundle_locks: KeyedLocks::default(),
    });

    let addr = format!("{host}:{port}");
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    let base = format!("http://{addr}");
    eprintln!(
        "abgen-serve: live-translate proxy on {base}\n  catalyst={catalyst} cache={cache} \
         version={version} mode={}\n  build={} buildDate={date}\n  bundles={}",
        if parity {
            "parity"
        } else {
            "serving (real-textures + v38)"
        },
        &bid[..16.min(bid.len())],
        bundle_dir.display(),
    );
    eprintln!("point unity-explorer: --lsd-use-remote-ab --lsd-remote-ab-server {base}");

    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let proxy = proxy.clone();
        std::thread::spawn(move || handle(&proxy, &mut stream));
    }
}
