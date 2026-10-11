//! CLI: rowan FILE.py [--json] [--golden] [--facts] [--no-rows] [--no-summaries] [--no-entry] [--no-imports] [--value-depth N] [--search-path DIR]… [--follow PKG]… [--site-packages DIR]… | rowan serve
use std::io::Write;

use rowan::interp::Analyzer;
use rowan::report;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut file: Option<String> = None;
    let mut json = false;
    let mut golden = false;
    let mut facts = false;
    let mut show_rows = true;
    let mut show_summaries = true;
    let mut entry = true;
    let mut imports = true;
    let mut search: Vec<String> = Vec::new();
    let mut follow: Vec<String> = std::env::var("ROWAN_FOLLOW").ok().map(|v| v.split(',').filter(|s| !s.is_empty()).map(|s| s.trim().to_string()).collect()).unwrap_or_default();
    let mut site: Vec<String> = Vec::new();
    let mut serve = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "serve" => serve = true,
            "--follow" => match it.next() {
                Some(n) => follow.extend(n.split(',').filter(|s| !s.is_empty()).map(|s| s.trim().to_string())),
                None => {
                    eprintln!("--follow needs a package name");
                    std::process::exit(2);
                }
            },
            "--site-packages" => match it.next() {
                Some(d) => site.push(d.clone()),
                None => {
                    eprintln!("--site-packages needs a directory");
                    std::process::exit(2);
                }
            },
            "--search-path" => match it.next() {
                Some(d) => search.push(d.clone()),
                None => {
                    eprintln!("--search-path needs a directory");
                    std::process::exit(2);
                }
            },
            "--value-depth" => match it.next().and_then(|n| n.parse::<u32>().ok()) {
                Some(n) => rowan::report::VALUE_DEPTH.store(n, std::sync::atomic::Ordering::Relaxed),
                None => {
                    eprintln!("--value-depth needs a number");
                    std::process::exit(2);
                }
            },
            "--json" => json = true,
            "--golden" => golden = true,
            "--facts" => facts = true,
            "--no-rows" => show_rows = false,
            "--no-summaries" => show_summaries = false,
            "--no-entry" => entry = false,
            "--no-imports" => imports = false,
            other if other.starts_with("--") => {
                eprintln!("unknown option {}", other);
                std::process::exit(2);
            }
            other => file = Some(other.to_string()),
        }
    }
    if serve {
        let search2 = search.clone();
        let opts = Opts { search: search2, follow: follow.clone(), site: site.clone(), entry, imports };
        let out = std::thread::Builder::new()
            .stack_size(1 << 30)
            .spawn(move || serve_loop(opts))
            .unwrap()
            .join();
        if out.is_err() {
            eprintln!("rowan: server panicked");
            std::process::exit(1);
        }
        return;
    }
    let Some(file) = file else {
        eprintln!("usage: rowan FILE.py [--json|--golden|--facts] [--no-rows] [--no-summaries] [--no-entry] [--no-imports] [--value-depth N] [--search-path DIR]... [--follow PKG]... [--site-packages DIR]...  |  rowan serve [options]");
        std::process::exit(2);
    };
    let out = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || run(&file, Opts { search, follow, site, entry, imports }, json, golden, facts, show_rows, show_summaries))
        .unwrap()
        .join();
    match out {
        Ok(Ok(text)) => {
            let mut so = std::io::stdout().lock();
            let _ = so.write_all(text.as_bytes());
            let _ = so.write_all(b"\n");
        }
        Ok(Err(e)) => {
            eprintln!("rowan: {}", e);
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("rowan: analysis panicked");
            std::process::exit(1);
        }
    }
}

/// `rowan serve`: JSON lines over stdio. Requests: `{"cmd":"analyse","path":P}` (also
/// `"facts"`) returns the `--facts` document for `P`, from a cache when `P` and every module
/// it loaded are unchanged (content hashes); `{"cmd":"changed","paths":[…]}` drops the
/// cached results that depend on those files; `{"cmd":"shutdown"}` exits. Every response
/// echoes the request's `id` and carries `ok`; a result has `cached`, `ms`, `deps` and
/// `result`. Parsed trees are cached per (path, content hash) in the process.
/// Import options shared by the CLI and the server.
#[derive(Clone)]
struct Opts {
    search: Vec<String>,
    follow: Vec<String>,
    site: Vec<String>,
    entry: bool,
    imports: bool,
}

/// site-packages directories for `file`: the ones given, `$VIRTUAL_ENV`, and a `.venv`
/// under the file's directory or an ancestor (no Python interpreter is consulted).
fn site_packages_for(file: &str, given: &[String]) -> Vec<String> {
    let mut out: Vec<String> = given.to_vec();
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(v) = std::env::var("VIRTUAL_ENV") {
        candidates.push(std::path::PathBuf::from(v));
    }
    let mut dir = std::path::Path::new(file).parent().map(|p| p.to_path_buf());
    for _ in 0..10 {
        let Some(d) = dir.clone() else { break };
        candidates.push(d.join(".venv"));
        candidates.push(d.join("venv"));
        dir = d.parent().map(|p| p.to_path_buf());
    }
    for venv in candidates {
        let lib = venv.join("lib");
        if let Ok(rd) = std::fs::read_dir(&lib) {
            for e in rd.flatten() {
                let p = e.path().join("site-packages");
                if e.file_name().to_string_lossy().starts_with("python") && p.is_dir() {
                    let s = p.to_string_lossy().to_string();
                    if !out.contains(&s) {
                        out.push(s);
                    }
                }
            }
        }
    }
    out
}

fn serve_loop(opts: Opts) {
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    let mut cache: std::collections::HashMap<String, (Vec<(String, u64)>, String)> = std::collections::HashMap::new();
    let file_hash = |p: &str| -> Option<u64> { std::fs::read_to_string(p).ok().map(|s| rowan::frontend::content_hash(&s)) };
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let req: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(out, "{}", serde_json::json!({"ok": false, "error": format!("bad request: {}", e)}));
                let _ = out.flush();
                continue;
            }
        };
        let id = req.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let cmd = req.get("cmd").and_then(|c| c.as_str()).unwrap_or("").to_string();
        let reply = match cmd.as_str() {
            "analyse" | "facts" => {
                let path = req.get("path").and_then(|p| p.as_str()).unwrap_or("").to_string();
                let t0 = std::time::Instant::now();
                let fresh = cache.get(&path).filter(|(deps, _)| deps.iter().all(|(p, h)| file_hash(p) == Some(*h))).map(|(deps, r)| (deps.clone(), r.clone()));
                match fresh {
                    Some((deps, result)) => {
                        let dep_paths: Vec<&str> = deps.iter().map(|(p, _)| p.as_str()).collect();
                        format!("{{\"id\":{},\"ok\":true,\"path\":{},\"cached\":true,\"ms\":{},\"deps\":{},\"result\":{}}}", id, serde_json::json!(path), t0.elapsed().as_millis(), serde_json::json!(dep_paths), result)
                    }
                    None => match run_with_deps(&path, opts.clone(), false, false, true, true, true) {
                        Ok((result, deps)) => {
                            let hashed: Vec<(String, u64)> = deps.iter().map(|p| (p.clone(), file_hash(p).unwrap_or(0))).collect();
                            cache.insert(path.clone(), (hashed, result.clone()));
                            format!("{{\"id\":{},\"ok\":true,\"path\":{},\"cached\":false,\"ms\":{},\"deps\":{},\"result\":{}}}", id, serde_json::json!(path), t0.elapsed().as_millis(), serde_json::json!(deps), result)
                        }
                        Err(e) => serde_json::json!({"id": id, "ok": false, "path": path, "error": e}).to_string(),
                    },
                }
            }
            "changed" => {
                let paths: Vec<String> = req.get("paths").and_then(|p| p.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
                let before = cache.len();
                cache.retain(|_, (deps, _)| !deps.iter().any(|(p, _)| paths.contains(p)));
                serde_json::json!({"id": id, "ok": true, "dropped": before - cache.len()}).to_string()
            }
            "shutdown" => {
                let _ = writeln!(out, "{}", serde_json::json!({"id": id, "ok": true}));
                let _ = out.flush();
                break;
            }
            other => serde_json::json!({"id": id, "ok": false, "error": format!("unknown command {:?}", other)}).to_string(),
        };
        let _ = writeln!(out, "{}", reply);
        let _ = out.flush();
    }
}

#[allow(clippy::too_many_arguments)]
fn run(file: &str, opts: Opts, json: bool, golden: bool, facts: bool, show_rows: bool, show_summaries: bool) -> Result<String, String> {
    run_with_deps(file, opts, json, golden, facts, show_rows, show_summaries).map(|(s, _)| s)
}

/// `run`, also returning the paths of every module the analysis loaded (the entry first).
#[allow(clippy::too_many_arguments)]
fn run_with_deps(file: &str, opts: Opts, json: bool, golden: bool, facts: bool, show_rows: bool, show_summaries: bool) -> Result<(String, Vec<String>), String> {
    let Opts { search, follow, site, entry, imports } = opts;
    let src = std::fs::read_to_string(file).map_err(|e| format!("{}: {}", file, e))?;
    let dir = std::path::Path::new(file).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| ".".into());
    let dir = std::fs::canonicalize(&dir).map(|p| p.to_string_lossy().to_string()).unwrap_or(dir);
    // import roots: the file's directory, the directory above its outermost package (so
    // `import pkg.sub` works from inside `pkg`), then the --search-path directories
    let mut roots = vec![dir];
    if let Some(r) = rowan::interp::package_root_of_file(file) {
        if !roots.contains(&r) {
            roots.push(r);
        }
    }
    roots.extend(search);
    let mut an = Analyzer::new(src.clone(), file, roots, entry, imports, None)?;
    an.facts_on = facts;
    an.follow = follow;
    if !an.follow.is_empty() {
        an.site_paths = site_packages_for(file, &site);
    }
    if golden {
        // `#@row NAME:` / `#@value` / `#@shape` expectations of the main file and, in a package
        // case (a directory whose name shares the entry's prefix: `k94_pkg/k94_modules.py`), of
        // every module under that directory
        let mut files: Vec<(String, String)> = vec![(String::new(), src.clone())];
        let path = std::path::Path::new(file);
        if let (Some(dir), Some(stem)) = (path.parent(), path.file_name().map(|f| f.to_string_lossy().to_string())) {
            let prefix = stem.split('_').next().unwrap_or("").to_string();
            let dname = dir.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            if !prefix.is_empty() && dname.starts_with(&format!("{}_", prefix)) {
                let mut stack = vec![dir.to_path_buf()];
                while let Some(d) = stack.pop() {
                    let Ok(rd) = std::fs::read_dir(&d) else { continue };
                    let mut ents: Vec<_> = rd.flatten().map(|e| e.path()).collect();
                    ents.sort();
                    for e in ents {
                        if e.is_dir() {
                            stack.push(e);
                        } else if e.extension().map(|x| x == "py").unwrap_or(false) && e.as_path() != path {
                            if let (Ok(rel), Ok(text)) = (e.strip_prefix(dir), std::fs::read_to_string(&e)) {
                                files.push((rel.to_string_lossy().to_string(), text));
                            }
                        }
                    }
                }
            }
        }
        for (rel, text) in &files {
            golden_requests(&mut an, rel, text);
        }
    }
    an.run();
    let mut deps: Vec<String> = vec![file.to_string()];
    for (_, &m) in an.modules.iter() {
        if let Some(p) = &an.module_info(m).path {
            if !deps.contains(p) {
                deps.push(p.clone());
            }
        }
    }
    if std::env::var("ROWAN_DEBUG").is_ok() {
        eprintln!("work: {} (units {}, tabulations {}, objects {}, {} ms{}; budgeted tabulations {})", an.work, an.work_units(), an.tab_runs, an.objects.len(), an.started.elapsed().as_millis(), if an.over_deadline() { ", degraded after the deadline" } else { "" }, an.tab_count.values().sum::<usize>());
    }
    if facts {
        return Ok((serde_json::to_string(&report::facts_json(&an)).unwrap(), deps));
    }
    if golden {
        return Ok((serde_json::to_string(&report::golden_json(&an)).unwrap(), deps));
    }
    if json {
        let lines: Vec<String> = report::to_json(&an).into_iter().map(|j| serde_json::to_string(&j).unwrap()).collect();
        return Ok((lines.join("\n"), deps));
    }
    Ok((report::format_text(&an, show_rows, show_summaries), deps))
}

/// The row / value / shape checks a file's directives ask for (`#@row NAME:` is evaluated at
/// the statement on its line; a line holding only a directive comment applies to the code line
/// above it).
fn golden_requests(an: &mut Analyzer, rel: &str, src: &str) {
    let mut last_code = 0usize;
    for (i0, line) in src.lines().enumerate() {
        let st = line.trim();
        let i = if st.starts_with("#@") && last_code > 0 {
            last_code - 1
        } else {
            if !st.is_empty() && !st.starts_with('#') {
                last_code = i0 + 1;
            }
            i0
        };
        let push = |an: &mut Analyzer, name: String| {
            let req = (rel.to_string(), i as u32 + 1, name);
            if !req.2.is_empty() && !an.row_check_requests.contains(&req) {
                an.row_check_requests.push(req);
            }
        };
        // (`#@todo row NAME:` too: the harness checks those with ROWAN_TODO=check)
        let hit = line.find("#@row").map(|p| p + 5).or_else(|| line.find("#@todo row").map(|p| p + 10));
        if let Some(pos) = hit {
            let rest = &line[pos..];
            let name: String = rest.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            push(an, name);
        }
        // `#@value NAME: …; NAME2: …` (also under `#@todo value`), `#@shape[@Ln] NAME: …`,
        // `#@dim[@Ln] NAME: …`
        let shape_hit = ["#@shape", "#@dim", "#@todo shape", "#@todo dim"].iter().find_map(|d| {
            let p = line.find(d)? + d.len();
            let rest = &line[p..];
            if rest.starts_with('-') {
                return None;
            }
            // skip an `@L<n>` context filter
            let skip = if rest.starts_with("@L") { rest.find(char::is_whitespace).unwrap_or(rest.len()) } else { 0 };
            Some(p + skip)
        });
        let vhit = line.find("#@value").map(|p| p + 7).or_else(|| line.find("#@todo value").map(|p| p + 12)).or(shape_hit);
        if let Some(pos) = vhit {
            for clause in line[pos..].split(';') {
                let name: String = clause.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                push(an, name);
            }
        }
    }
}
