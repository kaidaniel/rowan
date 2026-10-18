//! The command line: `rowan FILE.py [--json|--golden|--facts] [--no-rows] [--no-summaries]
//! [--no-entry] [--no-imports] [--value-depth N] [--search-path DIR]… [--follow PKG]…
//! [--site-packages DIR]…` analyses one program and prints its report; `rowan serve` is the
//! editor's server (`serve.rs`); `rowan hovers FILE [--part I/N] [--only NAME]` times every
//! function's run on demand (`bench/hovers.py`).

mod serve;

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
    let mut only: Option<String> = None;
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
            // (with `hovers`: only the functions whose qualified name contains this)
            "--only" => only = it.next().cloned(),
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
        on_big_stack(move || Ok(serve::serve_loop(opts)));
        return;
    }
    let Some(file) = file else {
        eprintln!("usage: rowan FILE.py [--json|--golden|--facts] [--no-rows] [--no-summaries] [--no-entry] [--no-imports] [--value-depth N] [--search-path DIR]... [--follow PKG]... [--site-packages DIR]...  |  rowan serve [options]");
        std::process::exit(2);
    };
    if let Some(part) = hovers {
        on_big_stack(move || time_hovers(&file, opts, part, only));
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
    let abs = std::fs::canonicalize(file).unwrap_or_else(|_| std::path::PathBuf::from(file));
    let mut dir = abs.parent().map(|p| p.to_path_buf());
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

/// `rowan hovers FILE [--part I/N] [--only NAME]` (a measurement, `bench/hovers.py`): `FILE`'s module code
/// once, then the run on demand of every function of the program's own modules (of part I of
/// N), each timed: JSON lines, the setup first.
fn time_hovers(file: &str, opts: Opts, (part, parts): (usize, usize), only: Option<String>) -> Result<(), String> {
    use std::io::Write;
    let t0 = std::time::Instant::now();
    let setup = |opts: &Opts| -> Result<Analyzer, String> {
        let mut an = analyser(file, Opts { entry: false, ..opts.clone() }, true)?;
        an.main_block_unrun();
        an.run();
        an.on_demand();
        Ok(an)
    };
    let mut an = setup(&opts)?;
    let setup_objects = an.work_counts()[2];
    let pick = |an: &Analyzer| -> Vec<(u32, u32, u32)> {
        let fns = an.outermost_functions(|m| an.is_local_module(m));
        fns.into_iter().filter(|(o, _, _)| only.as_ref().is_none_or(|q| an.heap.func_info(*o).py_qualname().contains(q.as_str()))).collect()
    };
    let mut fns = pick(&an);
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
        let _ = writeln!(so, "{}", serde_json::json!({"function": fi.py_qualname(), "file": file, "line": first, "ms": (ms * 10.0).round() / 10.0, "statements": w[0], "runs": w[1], "objects": w[2], "havoc": w[3], "cut": an.was_cut(), "callees_cut": !an.analysis_json()["callees_cut"].is_null()}));
        // (as `rowan serve` does: set up again past `REBUILD_OBJECTS`, the functions' objects
        // the same — the module's code makes them alike)
        if after[2] > setup_objects + serve::REBUILD_OBJECTS {
            let t = std::time::Instant::now();
            an = setup(&opts)?;
            fns = pick(&an);
            let _ = writeln!(so, "{}", serde_json::json!({"rebuild_ms": t.elapsed().as_millis()}));
        }
    }
    Ok(())
}

fn run(file: &str, opts: Opts, json: bool, golden: bool, facts: bool, show_rows: bool, show_summaries: bool) -> Result<String, String> {
    run_with_deps(file, opts, json, golden, facts, show_rows, show_summaries).map(|(s, _)| s)
}

/// `run`, also returning the paths of every module the analysis loaded (the entry first).
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
        for l in an.tally_lines() {
            eprintln!("{}", l);
        }
        let [m, l, sm, sl] = rowan::frontend::loaded_counts();
        eprintln!("loaded: {} modules, {} lines (site-packages: {} modules, {} lines), parsing {} ms", m, l, sm, sl, rowan::frontend::parse_time().as_millis());
        eprintln!("library entries (run, undone, refused): {:?}, work {}", an.library_counts().0, an.library_counts().1);
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
        if line.contains("#@general") || line.contains("#@todo general") {
            an.request_general_checks();
        }
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
