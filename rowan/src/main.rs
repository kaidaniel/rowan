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
    let mut hovers: Option<(usize, usize)> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "serve" => serve = true,
            // (a measurement: every function's run on demand, timed; `--part I/N`: every Nth from the Ith)
            "hovers" => hovers = Some((0, 1)),
            "--part" => match it.next().and_then(|p| p.split_once('/')).and_then(|(i, n)| Some((i.parse().ok()?, n.parse().ok()?))) {
                Some(p) => hovers = Some(p),
                None => {
                    eprintln!("--part needs I/N");
                    std::process::exit(2);
                }
            },
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
    let opts = Opts { search, follow, site, entry, imports };
    if serve {
        on_big_stack(move || Ok(serve_loop(opts)));
        return;
    }
    let Some(file) = file else {
        eprintln!("usage: rowan FILE.py [--json|--golden|--facts] [--no-rows] [--no-summaries] [--no-entry] [--no-imports] [--value-depth N] [--search-path DIR]... [--follow PKG]... [--site-packages DIR]...  |  rowan serve [options]");
        std::process::exit(2);
    };
    if let Some(part) = hovers {
        on_big_stack(move || time_hovers(&file, opts, part));
        return;
    }
    let text = on_big_stack(move || run(&file, opts, json, golden, facts, show_rows, show_summaries));
    let mut so = std::io::stdout().lock();
    let _ = so.write_all(text.as_bytes());
    let _ = so.write_all(b"\n");
}

/// `f` on a thread with a 1 GiB stack (deep recursion of the interpreter); its error or panic
/// ends the process.
fn on_big_stack<R: Send + 'static>(f: impl FnOnce() -> Result<R, String> + Send + 'static) -> R {
    match std::thread::Builder::new().stack_size(1 << 30).spawn(f).unwrap().join() {
        Ok(Ok(r)) => r,
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

/// `rowan serve`: JSON lines over stdio. Requests:
/// * `{"cmd":"analyse","path":P}` (also `"facts"`) returns the `--facts` document for `P`, from
///   a cache when `P` and every module it loaded are unchanged (content hashes). When the
///   deadline cuts the whole program's analysis, the result is the main module's own (its module
///   code: `on_demand: true`) and its functions are analysed one at a time when asked (`hover`).
/// * `{"cmd":"hover","path":P,"line":L}` returns the `--facts` document of the function of `P`
///   defined over line L, from its run for any call (open parameters as annotated, from the
///   state the module ended in), cached per function until a file it depends on changes; it
///   sets `P` up for that (its module code alone) if no analysis of `P` is kept.
/// * `{"cmd":"changed","paths":[…]}` drops the cached results that depend on those files;
///   `{"cmd":"shutdown"}` exits.
///
/// Every response echoes the request's `id` and carries `ok`; a result has `cached`, `ms`,
/// `deps` and `result`. Each analysis runs on a thread of its own (the engine's tables are
/// per thread); an analysis kept for `hover` keeps its thread (at most `SESSIONS`).
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

/// Objects the runs on demand of one analysis may add to its heap before it is set up again.
const REBUILD_OBJECTS: usize = 500_000;

/// Analyses kept for `hover` requests at most (each holds its whole heap).
const SESSIONS: usize = 2;

/// A request to an analysis kept on its thread: the function at a line, with where to reply.
type HoverReq = (u32, std::sync::mpsc::Sender<Result<HoverReply, String>>);

/// A `hover` request's answer: the facts document of the function's lines, whether it was
/// cached, the function's first and last lines.
struct HoverReply {
    result: String,
    cached: bool,
    span: (u32, u32),
}

/// An analysis kept for `hover` requests: its file, the files it loaded with their hashes,
/// and its thread's request channel.
struct Session {
    path: String,
    deps: Vec<(String, u64)>,
    tx: std::sync::mpsc::Sender<HoverReq>,
}

/// What a worker thread reports first: the analysis's facts document, the files it loaded,
/// and whether it went on demand (its thread then waits for `hover` requests).
type First = Result<(String, Vec<String>, bool), String>;

/// Analyse `path` on a thread of its own: the whole program (`whole`; on demand if the deadline
/// cuts it), or its module code alone, ready for `hover` requests. Returns the first report and,
/// when on demand, the thread's request channel.
fn spawn_analysis(path: String, opts: Opts, whole: bool) -> (First, Option<std::sync::mpsc::Sender<HoverReq>>) {
    let (first_tx, first_rx) = std::sync::mpsc::channel::<First>();
    let (req_tx, req_rx) = std::sync::mpsc::channel::<HoverReq>();
    let spawned = std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let mut an = match analyser(&path, Opts { entry: opts.entry && whole, ..opts.clone() }, true) {
            Ok(an) => an,
            Err(e) => {
                let _ = first_tx.send(Err(e));
                return;
            }
        };
        an.run();
        let deps = deps_of(&an, &path);
        let demand = !whole || an.was_cut();
        if !demand {
            let _ = first_tx.send(Ok((serde_json::to_string(&report::facts_json(&an)).unwrap(), deps, false)));
            return;
        }
        an.on_demand();
        let mut setup_objects = an.work_counts()[2];
        let mut doc = report::facts_json(&an);
        doc["on_demand"] = serde_json::json!(true);
        let _ = first_tx.send(Ok((serde_json::to_string(&doc).unwrap(), deps, true)));
        // (the documents of the functions run so far, by their first and last lines)
        let mut done: Vec<((u32, u32), String)> = Vec::new();
        for (line, reply) in req_rx {
            if let Some((span, r)) = done.iter().find(|((a, b), _)| *a <= line && line <= *b) {
                let _ = reply.send(Ok(HoverReply { result: r.clone(), cached: true, span: *span }));
                continue;
            }
            let before = an.work_counts();
            let Some(span) = an.general_run_at(line) else {
                let _ = reply.send(Err(format!("no function defined at line {}", line)));
                continue;
            };
            let mut doc = report::facts_json(&an);
            for list in ["facts", "verdicts"] {
                if let Some(a) = doc[list].as_array_mut() {
                    a.retain(|f| f["file"].as_str().is_none_or(|f| f.is_empty()) && f["line"].as_u64().is_some_and(|l| span.0 as u64 <= l && l <= span.1 as u64));
                }
            }
            doc["on_demand"] = serde_json::json!(true);
            doc["span"] = serde_json::json!([span.0, span.1]);
            let after = an.work_counts();
            doc["work"] = serde_json::json!({"statements": after[0] - before[0], "runs": after[1] - before[1], "objects": after[2] - before[2], "havoc": after[3] - before[3]});
            let r = serde_json::to_string(&doc).unwrap();
            done.push((span, r.clone()));
            let _ = reply.send(Ok(HoverReply { result: r, cached: false, span }));
            // (what the runs made stays in the heap: past `REBUILD_OBJECTS` more objects than
            // the module's own code made, the module is set up again — the documents stay)
            if an.work_counts()[2] > setup_objects + REBUILD_OBJECTS {
                let Ok(fresh) = analyser(&path, Opts { entry: false, ..opts.clone() }, true) else { break };
                an = fresh;
                an.run();
                an.on_demand();
                setup_objects = an.work_counts()[2];
            }
        }
    });
    if let Err(e) = spawned {
        return (Err(format!("cannot start an analysis thread: {}", e)), None);
    }
    match first_rx.recv() {
        Ok(Ok((r, deps, true))) => (Ok((r, deps, true)), Some(req_tx)),
        Ok(other) => (other, None),
        Err(_) => (Err("analysis panicked".to_string()), None),
    }
}

fn serve_loop(opts: Opts) {
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    let mut cache: std::collections::HashMap<String, (Vec<(String, u64)>, String)> = std::collections::HashMap::new();
    let mut sessions: Vec<Session> = Vec::new();
    let file_hash = |p: &str| -> Option<u64> { std::fs::read_to_string(p).ok().map(|s| rowan::frontend::content_hash(&s)) };
    let fresh = |deps: &[(String, u64)]| deps.iter().all(|(p, h)| file_hash(p) == Some(*h));
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
        let path = req.get("path").and_then(|p| p.as_str()).unwrap_or("").to_string();
        // (an analysis kept for `hover` whose files changed is dropped: its thread ends)
        sessions.retain(|s| fresh(&s.deps));
        let reply = match cmd.as_str() {
            "analyse" | "facts" => {
                let t0 = std::time::Instant::now();
                let kept = cache.get(&path).filter(|(deps, _)| fresh(deps)).map(|(deps, r)| (deps.clone(), r.clone()));
                match kept {
                    Some((deps, result)) => {
                        let dep_paths: Vec<&str> = deps.iter().map(|(p, _)| p.as_str()).collect();
                        format!("{{\"id\":{},\"ok\":true,\"path\":{},\"cached\":true,\"ms\":{},\"deps\":{},\"result\":{}}}", id, serde_json::json!(path), t0.elapsed().as_millis(), serde_json::json!(dep_paths), result)
                    }
                    None => {
                        sessions.retain(|s| s.path != path);
                        match spawn_analysis(path.clone(), opts.clone(), true) {
                            (Ok((result, deps, demand)), tx) => {
                                let hashed: Vec<(String, u64)> = deps.iter().map(|p| (p.clone(), file_hash(p).unwrap_or(0))).collect();
                                // (on demand, the module's own document is kept too: asked again, it is
                                // answered at once while the analysis waits for hovers)
                                if let (Some(tx), true) = (tx, demand) {
                                    keep_session(&mut sessions, Session { path: path.clone(), deps: hashed.clone(), tx });
                                }
                                cache.insert(path.clone(), (hashed, result.clone()));
                                format!("{{\"id\":{},\"ok\":true,\"path\":{},\"cached\":false,\"ms\":{},\"deps\":{},\"result\":{}}}", id, serde_json::json!(path), t0.elapsed().as_millis(), serde_json::json!(deps), result)
                            }
                            (Err(e), _) => serde_json::json!({"id": id, "ok": false, "path": path, "error": e}).to_string(),
                        }
                    }
                }
            }
            "hover" => {
                let line_no = req.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
                hover_request(&mut sessions, &opts, &id, &path, line_no)
            }
            "changed" => {
                let paths: Vec<String> = req.get("paths").and_then(|p| p.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
                let before = cache.len() + sessions.len();
                cache.retain(|_, (deps, _)| !deps.iter().any(|(p, _)| paths.contains(p)));
                sessions.retain(|s| !s.deps.iter().any(|(p, _)| paths.contains(p)));
                serde_json::json!({"id": id, "ok": true, "dropped": before - cache.len() - sessions.len()}).to_string()
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

/// `rowan hovers FILE [--part I/N]` (a measurement, `bench/hovers.py`): `FILE`'s module code
/// once, then the run on demand of every function of the program's own modules (of part I of
/// N), each timed: JSON lines, the setup first.
fn time_hovers(file: &str, opts: Opts, (part, parts): (usize, usize)) -> Result<(), String> {
    use std::io::Write;
    let t0 = std::time::Instant::now();
    let setup = |opts: &Opts| -> Result<Analyzer, String> {
        let mut an = analyser(file, Opts { entry: false, ..opts.clone() }, true)?;
        an.run();
        an.on_demand();
        Ok(an)
    };
    let mut an = setup(&opts)?;
    let setup_objects = an.work_counts()[2];
    let mut fns = an.outermost_functions(|m| an.is_local_module(m));
    let mut so = std::io::stdout().lock();
    let report = serde_json::json!({"setup_ms": t0.elapsed().as_millis(), "parse_ms": rowan::frontend::parse_time().as_millis(), "modules": an.modules().len(), "objects": an.work_counts()[2], "statements": an.work_counts()[0], "functions": fns.len()});
    let _ = writeln!(so, "{}", report);
    let mut k = part;
    while let Some(&(oid, first, _)) = fns.get(k) {
        k += parts.max(1);
        let fi = an.heap.func_info(oid);
        let file = an.rel_file(an.heap.module_info(fi.module).idx);
        let before = an.work_counts();
        let t = std::time::Instant::now();
        an.general_run_of(oid);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let after = an.work_counts();
        let w: Vec<usize> = (0..4).map(|i| after[i] - before[i]).collect();
        let _ = writeln!(so, "{}", serde_json::json!({"function": fi.py_qualname(), "file": file, "line": first, "ms": (ms * 10.0).round() / 10.0, "statements": w[0], "runs": w[1], "objects": w[2], "havoc": w[3], "cut": an.was_cut()}));
        // (as `rowan serve` does: set up again past `REBUILD_OBJECTS`, the functions' objects
        // the same — the module's code makes them alike)
        if after[2] > setup_objects + REBUILD_OBJECTS {
            let t = std::time::Instant::now();
            an = setup(&opts)?;
            fns = an.outermost_functions(|m| an.is_local_module(m));
            let _ = writeln!(so, "{}", serde_json::json!({"rebuild_ms": t.elapsed().as_millis()}));
        }
    }
    Ok(())
}

/// A `hover` request: the function of `path` at `line`, from the analysis kept for `path` (set
/// up first, its module code alone, if there is none).
fn hover_request(sessions: &mut Vec<Session>, opts: &Opts, id: &serde_json::Value, path: &str, line: u32) -> String {
    let t0 = std::time::Instant::now();
    let hash = |p: &str| std::fs::read_to_string(p).ok().map(|s| rowan::frontend::content_hash(&s)).unwrap_or(0);
    let mut setup = Ok(None);
    if !sessions.iter().any(|s| s.path == path) {
        setup = match spawn_analysis(path.to_string(), opts.clone(), false) {
            (Ok((_, deps, _)), Some(tx)) => {
                let deps = deps.iter().map(|p| (p.clone(), hash(p))).collect();
                keep_session(sessions, Session { path: path.to_string(), deps, tx });
                Ok(Some(t0.elapsed().as_millis()))
            }
            (Err(e), _) => Err(e),
            (Ok(_), None) => Err("the file could not be analysed".to_string()),
        };
    }
    let t1 = std::time::Instant::now();
    let answer = setup.and_then(|setup_ms| {
        let s = sessions.iter().find(|s| s.path == path).ok_or("the file could not be analysed")?;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = s.tx.send((line, tx));
        let h = rx.recv().unwrap_or_else(|_| Err("analysis panicked".to_string()))?;
        Ok((h, setup_ms))
    });
    match answer {
        Ok((h, setup_ms)) => format!(
            "{{\"id\":{},\"ok\":true,\"path\":{},\"cached\":{},\"ms\":{},\"setup_ms\":{},\"span\":[{},{}],\"result\":{}}}",
            id, serde_json::json!(path), h.cached, t1.elapsed().as_millis(), serde_json::json!(setup_ms), h.span.0, h.span.1, h.result
        ),
        Err(e) => serde_json::json!({"id": id, "ok": false, "path": path, "error": e}).to_string(),
    }
}

/// Keep `s` for `hover` requests, dropping the oldest past `SESSIONS` (its thread ends).
fn keep_session(sessions: &mut Vec<Session>, s: Session) {
    sessions.retain(|o| o.path != s.path);
    sessions.push(s);
    while sessions.len() > SESSIONS {
        sessions.remove(0);
    }
}

#[allow(clippy::too_many_arguments)]
fn run(file: &str, opts: Opts, json: bool, golden: bool, facts: bool, show_rows: bool, show_summaries: bool) -> Result<String, String> {
    run_with_deps(file, opts, json, golden, facts, show_rows, show_summaries).map(|(s, _)| s)
}

/// `run`, also returning the paths of every module the analysis loaded (the entry first).
#[allow(clippy::too_many_arguments)]
fn run_with_deps(file: &str, opts: Opts, json: bool, golden: bool, facts: bool, show_rows: bool, show_summaries: bool) -> Result<(String, Vec<String>), String> {
    let mut an = analyser(file, opts, facts)?;
    let src = std::fs::read_to_string(file).map_err(|e| format!("{}: {}", file, e))?;
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
    let deps = deps_of(&an, file);
    if std::env::var("ROWAN_DEBUG").is_ok() {
        eprintln!("{}", an.work_line());
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

/// The analyser of `file` with these import options (hover facts collected with `facts`).
fn analyser(file: &str, opts: Opts, facts: bool) -> Result<Analyzer, String> {
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
    let mut an = Analyzer::new(src, file, roots, entry, imports)?;
    if facts {
        an.collect_facts();
    }
    an.follow_packages(follow, || site_packages_for(file, &site));
    Ok(an)
}

/// The paths of every module the analysis of `file` loaded (the entry first).
fn deps_of(an: &Analyzer, file: &str) -> Vec<String> {
    let mut deps: Vec<String> = vec![file.to_string()];
    for (_, &m) in an.modules().iter() {
        if let Some(p) = &an.heap.module_info(m).path {
            // (a namespace package's directory has no text to hash)
            if !deps.contains(p) && !std::path::Path::new(p).is_dir() {
                deps.push(p.clone());
            }
        }
    }
    deps
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
            an.request_row_check((rel.to_string(), i as u32 + 1, name));
        };
        // (`#@todo row NAME:` too: the harness checks those with ROWAN_TODO=check)
        let hit = line.find("#@row").map(|p| p + 5).or_else(|| line.find("#@todo row").map(|p| p + 10));
        if let Some(pos) = hit {
            let rest = &line[pos..];
            let name: String = rest.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if let Some(ks) = rest.split("absent={").nth(1).and_then(|r| r.split('}').next()) {
                for k in ks.split(',').map(str::trim).filter(|k| !k.is_empty()) {
                    let key = match k.parse::<i64>() {
                        Ok(n) => rowan::intern::key_int(n),
                        Err(_) => rowan::intern::key_str_lit(k.trim_matches(|c| c == '"' || c == '\'')),
                    };
                    an.request_absent_probe((rel.to_string(), i as u32 + 1, name.clone(), key));
                }
            }
            push(an, name);
        }
        // `#@value NAME: …; NAME2: …`, `#@general …` (also under `#@todo`), `#@shape[@Ln] NAME: …`,
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
        let vhit = ["#@value", "#@todo value", "#@general", "#@todo general"].iter().find_map(|d| line.find(d).map(|p| p + d.len())).or(shape_hit);
        if let Some(pos) = vhit {
            for clause in line[pos..].split(';') {
                let name: String = clause.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                push(an, name);
            }
        }
    }
}

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
