//! abgen-serve — a *live-translate* asset-bundle proxy for unity-explorer.
//!
//! Where `abgen-corpus` builds an output tree ahead of time and the
//! `abgen-serve.py` script serves it statically, this binary serves the same
//! Decentraland AB-CDN routes but converts **just-in-time**: on a cache miss it
//! resolves the entity from a live catalyst, fetches its glb/image content,
//! runs the exact same spec-derivation + `build_bundle` pipeline `abgen-corpus`
//! uses, caches the result to disk, and serves it. Steady-state it is a static
//! file server; cold requests pay one conversion.
//!
//! Routes (match `abgen-serve.py`, verified against unity-explorer):
//!   GET  /<version>/<entityCID>/<assetHash>_<platform>   -> bundle (nested)
//!   GET  /<version>/<assetHash>_<platform>               -> bundle (flat)
//!   GET  /manifest/<entityCID>_<platform>.json           -> {version,date} (+ warms the entity)
//!   POST /entities/versions   {"pointers":[...]}          -> AB-registry version map
//!   OPTIONS *                                             -> CORS preflight
//!
//! Point the client with:
//!   --lsd-use-remote-ab --lsd-remote-ab-server http://127.0.0.1:<port>
//!
//! By default it builds *serving-correct* bundles (`--real-textures` +
//! `--v38-compat`); pass `--parity` to emit fork-byte-faithful bundles instead
//! (oversized textures become flat-color stubs — useful only for diffing).

use abgen::builder::{build_bundle, BuildOpts};
use abgen::catalyst::{CatalystClient, Scene, DEFAULT_CATALYST};
use abgen::glbscan::{scan_entity, EntityScan, UriCache};
use abgen::local_store::LocalContentStore;
use abgen::{anyhow, bail, naming, Context, Result};
use rayon::prelude::*;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CONVERTIBLE_EXTS: [&str; 5] = [".glb", ".gltf", ".png", ".jpg", ".jpeg"];

fn is_convertible(file: &str) -> (bool, bool) {
    let fl = file.to_lowercase();
    let is_glb = fl.ends_with(".glb") || fl.ends_with(".gltf");
    let is_image = fl.ends_with(".png") || fl.ends_with(".jpg") || fl.ends_with(".jpeg");
    (is_glb, is_image)
}

/// One-lock-per-key registry so two requests for the same entity/bundle do the
/// cold work once instead of racing.
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

/// Resolved + scanned entity, cached so repeat bundle builds skip the network
/// round-trip and the glb scan. Platform-independent (it is derived per build).
struct EntityCtx {
    scene: Scene,
    content_by_file: HashMap<String, String>,
    scan: EntityScan,
}

struct Proxy {
    catalyst: CatalystClient,
    local: Option<LocalContentStore>, // optional read-through snapshot fallback
    content: LocalContentStore,       // persistent content cache (sharded layout)
    bundle_dir: PathBuf,              // persistent built-bundle cache (flat by name)
    version: String,
    date: String,
    uri_cache: UriCache,

    entities: Mutex<HashMap<String, Arc<EntityCtx>>>,
    hash_index: Mutex<HashMap<String, String>>, // assetHash -> entityCID (flat-route lookup)
    entity_locks: KeyedLocks,
    bundle_locks: KeyedLocks,
}

impl Proxy {
    /// Make sure `hash`'s bytes are in the local content cache, pulling from the
    /// snapshot fallback or the live catalyst as needed.
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

    /// Resolve + scan an entity once, caching the result.
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

        // Warm the convertible content (models + textures) so the scan and any
        // bundle build read from disk. Best-effort: a missing texture is handled
        // downstream; only the specifically-requested bundle hard-requires bytes.
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

    /// Return a built bundle's bytes, building + caching it on first request.
    fn bundle(&self, cid: &str, bundle_name: &str) -> Result<Vec<u8>> {
        let cache_path = self.bundle_dir.join(bundle_name);
        if let Ok(b) = std::fs::read(&cache_path) {
            return Ok(b);
        }
        let lock = self.bundle_locks.get(bundle_name);
        let _g = lock.lock().unwrap();
        if let Ok(b) = std::fs::read(&cache_path) {
            return Ok(b);
        }

        let ctx = self.entity_ctx(cid)?;
        let data = self.build(&ctx, bundle_name)?;

        std::fs::create_dir_all(&self.bundle_dir).ok();
        let tmp = cache_path.with_extension(format!("tmp.{}", std::process::id()));
        std::fs::write(&tmp, &data).with_context(|| format!("write {}", tmp.display()))?;
        std::fs::rename(&tmp, &cache_path).ok();
        Ok(data)
    }

    /// The JIT conversion: mirrors `abgen-corpus`'s `from_entity_ids` per-bundle
    /// spec derivation and `build_one`, for a single (entity, bundle) pair.
    fn build(&self, ctx: &EntityCtx, bundle_name: &str) -> Result<Vec<u8>> {
        let (hash, platform) = bundle_name
            .rsplit_once('_')
            .ok_or_else(|| anyhow!("bundle name {bundle_name:?} has no _<platform> suffix"))?;

        // The explorer lowercases Qm (CIDv0) hashes in the bundle URL, but the
        // entity's content + the catalyst store key on the original case. Match
        // case-insensitively, then use the original-case hash for fetch/build/refs;
        // the bundle filename (bundle_name) stays exactly as requested.
        let item = ctx
            .scene
            .content
            .iter()
            .find(|c| c.hash.eq_ignore_ascii_case(hash))
            .ok_or_else(|| anyhow!("hash {hash} not in entity {}", ctx.scene.entity_id))?;
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
        let resolve_hash: Option<&dyn Fn(&str) -> Option<String>> = if !content_by_file.is_empty() {
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
        };
        let artifact = build_bundle(&glb, bundle_name, hash, &opts)?;
        Ok(artifact.data)
    }

    /// Find which entity owns a flat-route asset hash (populated as entities are
    /// resolved). `None` -> let the client fall back.
    fn entity_for_hash(&self, hash: &str) -> Option<String> {
        self.hash_index.lock().unwrap().get(&hash.to_lowercase()).cloned()
    }
}

// ---------------------------------------------------------------------------
// minimal HTTP/1.1 (std only; one thread per connection, Connection: close)
// ---------------------------------------------------------------------------

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_request(stream: &TcpStream) -> Result<Option<Request>> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None); // client closed
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

fn handle(proxy: &Proxy, stream: &mut TcpStream) {
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
        // Resolve each pointer to its entity + content and index every content
        // hash -> entity. The explorer POSTs this BEFORE requesting bundles via
        // the flat route (/v<n>/<hash>_mac, no entity in the path), so without
        // this the flat builds can't find the entity to JIT-convert (-> 404).
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
            "versions": {"assets": {"windows": ver, "mac": ver}},
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

    // GET /manifest/<entityCID>_<platform>.json  -> {version,date} (+ warm)
    if parts.len() == 2 && parts[0] == "manifest" && parts[1].ends_with(".json") {
        let stem = &parts[1][..parts[1].len() - 5];
        let entity = stem.rsplit_once('_').map(|(e, _)| e).unwrap_or(stem);
        match proxy.entity_ctx(entity) {
            Ok(_) => {
                let body = serde_json::json!({"version": proxy.version, "date": proxy.date});
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

    // GET /<version>/<entity>/<file>  or  /<version>/<file>
    let (cid, file): (Option<String>, &str) = match parts.as_slice() {
        [_v, entity, f] => (Some((*entity).to_string()), *f),
        [_v, f] => {
            let f: &str = *f;
            let hash = f.rsplit_once('_').map(|(h, _)| h).unwrap_or(f);
            (proxy.entity_for_hash(hash), f)
        }
        _ => return respond(stream, 404, "text/plain", b"not found", head),
    };

    let Some(cid) = cid else {
        eprintln!("GET {raw_path}: no entity known for hash (flat); client should fall back");
        return respond(stream, 404, "text/plain", b"unknown asset", head);
    };

    match proxy.bundle(&cid, file) {
        Ok(data) => {
            eprintln!("GET {cid}/{file} -> {} bytes", data.len());
            respond(stream, 200, "application/octet-stream", &data, head)
        }
        Err(e) => {
            eprintln!("build {cid}/{file}: {e:#}");
            respond(
                stream,
                500,
                "text/plain",
                format!("build failed: {e}").as_bytes(),
                head,
            )
        }
    }
}

// ---------------------------------------------------------------------------

/// Build identity = sha256 over this binary + the template bundles it serializes
/// against. Folded into the bundle-cache path so a rebuilt binary OR a
/// regenerated template never serves a stale conversion.
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

/// Deterministic ISO8601 derived from the build id, so the client's
/// (buildDate + hash) AB-cache key changes whenever the build does — the
/// explorer then refetches instead of reusing a stale download.
fn iso_from_build_id(id: &str) -> String {
    let n = u64::from_str_radix(id.get(..8).unwrap_or("0"), 16).unwrap_or(0);
    let base = 1_577_836_800u64; // 2020-01-01T00:00:00Z
    iso8601_utc(base + (n % 946_080_000)) // within ~30 years
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
            "-h" | "--help" => usage(),
            other => {
                eprintln!("unknown argument: {other}");
                usage();
            }
        }
        i += 1;
    }

    if !parity {
        // Serving-correct defaults: real textures + production v38 shape.
        std::env::set_var(BuildOpts::REAL_TEXTURES_ENV, "1");
        std::env::set_var(BuildOpts::V38_COMPAT_ENV, "1");
    }

    // Identity of this build (binary + templates). Keys the bundle cache so a
    // rebuilt binary or regenerated template can never serve a stale conversion,
    // and (via the derived buildDate) makes the client refetch too.
    let bid = build_id();
    let date = date.unwrap_or_else(|| iso_from_build_id(&bid));

    let cache_root = PathBuf::from(&cache);
    // Upstream content is immutable by CID -> shared across builds (no re-fetch).
    let content = LocalContentStore::new(cache_root.join("content"));
    // Built bundles are build-specific -> namespaced by build id.
    let bundle_dir = cache_root.join("bundles").join(&bid[..16.min(bid.len())]);
    if let Err(e) = std::fs::create_dir_all(&bundle_dir) {
        eprintln!("error: create cache {}: {e}", bundle_dir.display());
        std::process::exit(1);
    }

    let proxy = Arc::new(Proxy {
        catalyst: CatalystClient::new(&catalyst),
        local: local.map(LocalContentStore::new),
        content,
        bundle_dir: bundle_dir.clone(),
        version: version.clone(),
        date: date.clone(),
        uri_cache: UriCache::new(),
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
