//! `rowan serve`: JSON lines over stdio. Requests:
//! * `{"cmd":"analyse","path":P}` (also `"facts"`) returns the `--facts` document for `P`, from
//!   a cache when `P` and every module it loaded are unchanged (content hashes). The whole
//!   program's analysis and one of the main module's own code start together: unless the whole
//!   program's completes within `EARLY`, the result is the module code's (`on_demand: true`;
//!   its functions are analysed one at a time when asked, `hover`), with `pending: true` while
//!   the whole program's goes on; when that completes (not cut by its deadline) its document
//!   follows as an event line `{"event":"analysed","path":P,"ms":…,"deps":…,"result":…}`; cut,
//!   `{"event":"cut","path":P,"whole":…}` (how far it got, `analysis_json`; a module code's
//!   document answered after the cut carries it as `whole`).
//! * Following installed packages (`--follow`): all of the above runs without following, so the
//!   first answer comes as fast as without. When they saw the program import a package it would
//!   follow (by the answer, or when the whole program's analysis completes), a whole program's
//!   analysis following them starts (`pending`, `following`); when it completes (not cut by its
//!   deadline) its document replaces the answer, `{"event":"analysed","followed":true,…}` (later
//!   events of the analyses without following say nothing). A newer `analyse`, `changed` or
//!   `cancel` cuts it.
//! * `{"cmd":"hover","path":P,"line":L}` returns the `--facts` document of the function of `P`
//!   defined over line L, from its run for any call (open parameters as annotated, from the
//!   state the module ended in), cached per function until a file it depends on changes; it
//!   sets `P` up for that (its module code alone) if no analysis of `P` is kept.
//! * `{"cmd":"changed","paths":[…]}` drops the cached results that depend on those files and
//!   ends the analyses running for them (as their deadline would); `{"cmd":"cancel","paths":[…]}`
//!   only ends those (answered at once, before requests still running); `{"cmd":"shutdown"}` exits.
//!
//! Every response echoes the request's `id` and carries `ok`; a result has `cached`, `ms`,
//! `deps` and `result` (responses come in order, a cancel's aside; an event has no `id`).
//! Each analysis runs on a thread of its own (the engine's tables are per thread); an
//! analysis kept for `hover` keeps its thread (at most `SESSIONS`).

use rowan::report;

use crate::{analyser, deps_of, Opts};

/// Objects the runs on demand of one analysis may add to its heap before it is set up again.
pub(crate) const REBUILD_OBJECTS: usize = 500_000;

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

/// What an analysis thread runs (`start_analysis`).
#[derive(Clone, Copy, PartialEq)]
enum Start {
    /// the whole program, ending if the deadline cuts it (an analysis of the module code
    /// answers then)
    Whole,
    /// its module code alone, then its functions on demand
    Module,
}

/// How long `analyse` waits for the whole program's analysis before it answers from the
/// module code's (its functions then run when hovered while the whole program's goes on).
const EARLY: std::time::Duration = std::time::Duration::from_millis(1000);

/// Analyse `path` on a thread of its own (`start`): the channel its first report comes on and
/// the thread's request channel (for `hover` requests, when it goes on demand; dropping it
/// ends the thread).
fn start_analysis(path: String, opts: Opts, start: Start, cancel: Cancel) -> (std::sync::mpsc::Receiver<First>, std::sync::mpsc::Sender<HoverReq>) {
    let (first_tx, first_rx) = std::sync::mpsc::channel::<First>();
    let (req_tx, req_rx) = std::sync::mpsc::channel::<HoverReq>();
    let whole = start != Start::Module;
    let spawned = std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let mut an = match analyser(&path, Opts { entry: opts.entry && whole, ..opts.clone() }, true) {
            Ok(an) => an,
            Err(e) => {
                let _ = first_tx.send(Err(e));
                return;
            }
        };
        an.set_cancel(cancel.clone());
        if !whole {
            an.main_block_unrun();
        }
        an.run();
        if let Some((follow, flag)) = &opts.probe {
            if followable(&an.external_imports(), follow, &path, &opts.site) {
                flag.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        if std::env::var("ROWAN_DEBUG").is_ok() {
            eprintln!("{} ({} run of {})", an.work_line(), if whole { "whole" } else { "module" }, path);
        }
        let deps = deps_of(&an, &path);
        let demand = !whole || an.was_cut();
        if !demand {
            let _ = first_tx.send(Ok((serde_json::to_string(&report::facts_json(&an)).unwrap(), deps, false)));
            return;
        }
        // (cut at its deadline: how far it got, for the module code's document to say)
        if start == Start::Whole {
            let _ = first_tx.send(Ok((an.analysis_json().to_string(), deps, true)));
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
            // (a cancel ends the run it finds, not the next ones)
            cancel.store(false, std::sync::atomic::Ordering::Relaxed);
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
            // (the function's run: its time, and whether its deadline cut it)
            doc["analysis"]["statements"] = serde_json::json!(after[0] - before[0]);
            let r = serde_json::to_string(&doc).unwrap();
            done.push((span, r.clone()));
            let _ = reply.send(Ok(HoverReply { result: r, cached: false, span }));
            // (what the runs made stays in the heap: past `REBUILD_OBJECTS` more objects than
            // the module's own code made, the module is set up again — the documents stay)
            if an.work_counts()[2] > setup_objects + REBUILD_OBJECTS {
                let Ok(fresh) = analyser(&path, Opts { entry: false, ..opts.clone() }, true) else { break };
                an = fresh;
                an.set_cancel(cancel.clone());
                an.main_block_unrun();
                an.run();
                an.on_demand();
                setup_objects = an.work_counts()[2];
            }
        }
    });
    if let Err(e) = spawned {
        let (tx, rx) = std::sync::mpsc::channel::<First>();
        let _ = tx.send(Err(format!("cannot start an analysis thread: {}", e)));
        return (rx, req_tx);
    }
    (first_rx, req_tx)
}

/// `start_analysis`, waiting for its first report; the request channel when it went on demand.
fn spawn_analysis(path: String, opts: Opts, start: Start, cancel: Cancel) -> (First, Option<std::sync::mpsc::Sender<HoverReq>>) {
    let (first_rx, req_tx) = start_analysis(path, opts, start, cancel);
    match first_rx.recv() {
        Ok(Ok((r, deps, true))) => (Ok((r, deps, true)), Some(req_tx)),
        Ok(other) => (other, None),
        Err(_) => (Err("analysis panicked".to_string()), None),
    }
}

/// The server's state shared with the threads that wait for a whole program's analysis: the
/// documents kept per file (with the files they depend on and their hashes), the analyses kept
/// for `hover`, and per file the number of its latest `analyse` (a late answer to an earlier
/// one is dropped), the flags that cancel the analyses running for a file, and the `analyse`
/// a followed analysis answered.
#[derive(Default)]
struct Shared {
    cache: std::collections::HashMap<String, (Vec<(String, u64)>, String)>,
    sessions: Vec<Session>,
    latest: std::collections::HashMap<String, u64>,
    running: Vec<(String, Cancel)>,
    /// per file, the latest `analyse` whose analysis following installed packages answered
    /// (`follow_followed`): the analyses without following say nothing after it
    followed: std::collections::HashMap<String, u64>,
}

/// Set from the server's reader: the analysis holding it is stale (its file changed, a newer
/// `analyse` of it came, its request timed out) and ends as its deadline would end it.
type Cancel = std::sync::Arc<std::sync::atomic::AtomicBool>;

impl Shared {
    /// A flag for an analysis of `path` starting now.
    fn cancel_flag(&mut self, path: &str) -> Cancel {
        // (flags no thread holds any more: their analyses ended)
        self.running.retain(|(_, c)| std::sync::Arc::strong_count(c) > 1);
        let c = Cancel::default();
        self.running.push((path.to_string(), c.clone()));
        c
    }

    /// Cancel the analyses running for `paths` (a whole program's, the module code's and its
    /// runs on demand).
    fn cancel(&mut self, paths: &[String]) {
        for (_, c) in self.running.iter().filter(|(p, _)| paths.contains(p)) {
            c.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

type Out = std::sync::Arc<std::sync::Mutex<std::io::Stdout>>;

fn send_line(out: &Out, line: &str) {
    use std::io::Write;
    let mut o = out.lock().unwrap();
    let _ = writeln!(o, "{}", line);
    let _ = o.flush();
}

fn file_hash(p: &str) -> Option<u64> {
    std::fs::read_to_string(p).ok().map(|s| rowan::frontend::content_hash(&s))
}

fn hashed(deps: &[String]) -> Vec<(String, u64)> {
    deps.iter().map(|p| (p.clone(), file_hash(p).unwrap_or(0))).collect()
}

/// The server's loop: requests read on a thread of their own, answered in order.
pub(crate) fn serve_loop(opts: Opts) {
    let out: Out = std::sync::Arc::new(std::sync::Mutex::new(std::io::stdout()));
    let shared = std::sync::Arc::new(std::sync::Mutex::new(Shared::default()));
    let fresh = |deps: &[(String, u64)]| deps.iter().all(|(p, h)| file_hash(p) == Some(*h));
    let requests = read_requests(shared.clone(), out.clone());
    for req in requests {
        let id = req.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let cmd = req.get("cmd").and_then(|c| c.as_str()).unwrap_or("").to_string();
        let path = req.get("path").and_then(|p| p.as_str()).unwrap_or("").to_string();
        // (an analysis kept for `hover` whose files changed is dropped: its thread ends)
        shared.lock().unwrap().sessions.retain(|s| fresh(&s.deps));
        let reply = match cmd.as_str() {
            "analyse" | "facts" => {
                let t0 = std::time::Instant::now();
                let kept = shared.lock().unwrap().cache.get(&path).filter(|(deps, _)| fresh(deps)).map(|(deps, r)| (deps.clone(), r.clone()));
                match kept {
                    Some((deps, result)) => {
                        let dep_paths: Vec<&str> = deps.iter().map(|(p, _)| p.as_str()).collect();
                        format!("{{\"id\":{},\"ok\":true,\"path\":{},\"cached\":true,\"ms\":{},\"deps\":{},\"result\":{}}}", id, serde_json::json!(path), t0.elapsed().as_millis(), serde_json::json!(dep_paths), result)
                    }
                    None => analyse_request(&shared, &out, &opts, &id, &path),
                }
            }
            "hover" => {
                let line_no = req.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
                hover_request(&shared, &opts, &id, &path, line_no)
            }
            "changed" => {
                let paths: Vec<String> = req.get("paths").and_then(|p| p.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
                let mut sh = shared.lock().unwrap();
                let before = sh.cache.len() + sh.sessions.len();
                sh.cache.retain(|_, (deps, _)| !deps.iter().any(|(p, _)| paths.contains(p)));
                sh.sessions.retain(|s| !s.deps.iter().any(|(p, _)| paths.contains(p)));
                let dropped = before - sh.cache.len() - sh.sessions.len();
                // (a whole program's analysis still running for a changed file answers nothing)
                for p in &paths {
                    *sh.latest.entry(p.clone()).or_default() += 1;
                }
                serde_json::json!({"id": id, "ok": true, "dropped": dropped}).to_string()
            }
            "shutdown" => {
                send_line(&out, &serde_json::json!({"id": id, "ok": true}).to_string());
                break;
            }
            other => serde_json::json!({"id": id, "ok": false, "error": format!("unknown command {:?}", other)}).to_string(),
        };
        send_line(&out, &reply);
    }
}

/// The requests on stdin, read on a thread of their own so that a `cancel` (and the
/// cancelling part of `changed`) acts while an earlier request still runs: `{"cmd":"cancel",
/// "paths":[…]}` ends the analyses running for those files (answered at once).
fn read_requests(shared: std::sync::Arc<std::sync::Mutex<Shared>>, out: Out) -> std::sync::mpsc::Receiver<serde_json::Value> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            let req: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(e) => {
                    send_line(&out, &serde_json::json!({"ok": false, "error": format!("bad request: {}", e)}).to_string());
                    continue;
                }
            };
            let cmd = req.get("cmd").and_then(|c| c.as_str()).unwrap_or("");
            if matches!(cmd, "cancel" | "changed") {
                let paths: Vec<String> = req.get("paths").and_then(|p| p.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
                shared.lock().unwrap().cancel(&paths);
                if cmd == "cancel" {
                    send_line(&out, &serde_json::json!({"id": req.get("id"), "ok": true}).to_string());
                    continue;
                }
            }
            let stop = cmd == "shutdown";
            if tx.send(req).is_err() || stop {
                break;
            }
        }
    });
    rx
}

/// An `analyse` request with nothing kept: the whole program's analysis and its module code's
/// alone start together. The whole program's document answers if it comes within `EARLY`;
/// else the module code's, once it is set up (`on_demand`, `pending`: its functions run when
/// hovered), and when the whole program's analysis completes later (not cut by its deadline)
/// its document follows as an event, `{"event":"analysed","path":P,…,"result":…}`.
fn analyse_request(shared: &std::sync::Arc<std::sync::Mutex<Shared>>, out: &Out, opts: &Opts, id: &serde_json::Value, path: &str) -> String {
    let t0 = std::time::Instant::now();
    let generation = {
        let mut sh = shared.lock().unwrap();
        sh.sessions.retain(|s| s.path != path);
        // (an earlier analysis of the file still running is stale)
        sh.cancel(&[path.to_string()]);
        let g = sh.latest.entry(path.to_string()).or_default();
        *g += 1;
        *g
    };
    let (whole_cancel, module_cancel) = {
        let mut sh = shared.lock().unwrap();
        (sh.cancel_flag(path), sh.cancel_flag(path))
    };
    // (following installed packages: the answer comes from analyses without; when they see the
    // program import one it would follow, an analysis following them starts, `follow_later`)
    let later = FollowLater { opts: opts.clone(), probe: Default::default(), spawned: Default::default() };
    let opts = &Opts { probe: (!opts.follow.is_empty()).then(|| (opts.follow.clone(), later.probe.clone())), ..unfollowed(opts) };
    let (whole_rx, _whole_tx) = start_analysis(path.to_string(), opts.clone(), Start::Whole, whole_cancel);
    let (module_rx, module_tx) = start_analysis(path.to_string(), opts.clone(), Start::Module, module_cancel);
    let answer = |res: Result<(String, Vec<String>), String>, pending: bool| -> String {
        let mut following = later.start(shared, out, path, generation, t0);
        let mut pending = pending || following;
        match res {
            Ok((mut doc, mut deps)) => {
                let mut sh = shared.lock().unwrap();
                // (the analysis following installed packages answered first: its document)
                match sh.cache.get(path).filter(|_| sh.followed.get(path) == Some(&generation)) {
                    Some((d, r)) => (doc, deps, pending, following) = (r.clone(), d.iter().map(|(p, _)| p.clone()).collect(), false, false),
                    None => drop(sh.cache.insert(path.to_string(), (hashed(&deps), doc.clone()))),
                }
                format!("{{\"id\":{},\"ok\":true,\"path\":{},\"cached\":false,\"pending\":{},\"following\":{},\"ms\":{},\"deps\":{},\"result\":{}}}", id, serde_json::json!(path), pending, following, t0.elapsed().as_millis(), serde_json::json!(deps), doc)
            }
            Err(e) => serde_json::json!({"id": id, "ok": false, "path": path, "error": e}).to_string(),
        }
    };
    // (the whole program's answer, if it comes early)
    let mut whole_cut: Option<String> = None;
    let early = match whole_rx.recv_timeout(EARLY) {
        Ok(r) => {
            whole_cut = cut_of(&r);
            whole_doc(r)
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
        Err(_) => Some(Err("analysis panicked".to_string())),
    };
    if let Some(r) = early {
        return answer(r, false);
    }
    let module = module_rx.recv().unwrap_or_else(|_| Err("analysis panicked".to_string()));
    // (done meanwhile: the whole program's)
    if whole_cut.is_none() {
        if let Ok(r) = whole_rx.try_recv() {
            whole_cut = cut_of(&r);
            if let Some(r) = whole_doc(r) {
                return answer(r, false);
            }
        }
    }
    // (cut at its deadline: the module code's analysis answers, saying so; nothing follows)
    if let Some(info) = &whole_cut {
        return match module {
            Ok((doc, deps, _)) => {
                keep_session(&mut shared.lock().unwrap().sessions, Session { path: path.to_string(), deps: hashed(&deps), tx: module_tx });
                answer(Ok((with_whole(doc, info), deps)), false)
            }
            Err(e) => answer(Err(e), false),
        };
    }
    let (doc, deps) = match module {
        Ok((doc, deps, _)) => (doc, deps),
        // (the module code cannot be analysed alone: wait for the whole program)
        Err(_) => return match whole_rx.recv().map(whole_doc) {
            Ok(Some(r)) => answer(r, false),
            _ => answer(Err("the file could not be analysed".to_string()), false),
        },
    };
    keep_session(&mut shared.lock().unwrap().sessions, Session { path: path.to_string(), deps: hashed(&deps), tx: module_tx });
    // (the whole program's document, when its analysis completes, replaces this one)
    let waiter = std::thread::Builder::new().spawn({
        let (shared, out, path, module_doc, later) = (shared.clone(), out.clone(), path.to_string(), doc.clone(), later.clone());
        move || follow_whole(&shared, &out, &path, generation, whole_rx, _whole_tx, module_doc, t0, later)
    });
    let pending = waiter.is_ok();
    answer(Ok((doc, deps)), pending)
}

/// A whole program's document, when its analysis completed (not cut by its deadline).
fn whole_doc(r: First) -> Option<Result<(String, Vec<String>), String>> {
    match r {
        Ok((doc, deps, false)) => Some(Ok((doc, deps))),
        Ok((_, _, true)) => None,
        Err(e) => Some(Err(e)),
    }
}

/// The whole program's analysis cut at its deadline: how far it got (`analysis_json`).
fn cut_of(r: &First) -> Option<String> {
    match r {
        Ok((info, _, true)) => Some(info.clone()),
        _ => None,
    }
}

/// A module code's document, with what cut the whole program's analysis (`whole`).
fn with_whole(doc: String, info: &str) -> String {
    format!("{{\"whole\":{},{}", info, &doc[1..])
}

/// After a module code's document answered an `analyse` (`generation`, `module_doc`): the whole
/// program's analysis, when it completes, as an `analysed` event (its document replaces the
/// module code's); cut at its deadline, a `cut` event (the module code's document stays, saying
/// so). Nothing when a newer `analyse` came, or a file it loaded changed.
#[allow(clippy::too_many_arguments)]
fn follow_whole(shared: &std::sync::Arc<std::sync::Mutex<Shared>>, out: &Out, path: &str, generation: u64, whole_rx: std::sync::mpsc::Receiver<First>, _whole_tx: std::sync::mpsc::Sender<HoverReq>, module_doc: String, t0: std::time::Instant, later: FollowLater) {
    let Ok(r) = whole_rx.recv() else { return };
    // (an import the module code's analysis did not see, in a function: following starts now)
    later.start(shared, out, path, generation, t0);
    let mut sh = shared.lock().unwrap();
    if sh.latest.get(path) != Some(&generation) || sh.followed.get(path) == Some(&generation) {
        return;
    }
    if let Some(info) = cut_of(&r) {
        if let Some(c) = sh.cache.get_mut(path) {
            c.1 = with_whole(module_doc, &info);
        }
        drop(sh);
        send_line(out, &format!("{{\"event\":\"cut\",\"ok\":true,\"path\":{},\"whole\":{}}}", serde_json::json!(path), info));
        return;
    }
    let Some(Ok((doc, deps))) = whole_doc(r) else { return };
    let deps_h = hashed(&deps);
    if !deps_h.iter().all(|(p, h)| file_hash(p) == Some(*h)) {
        return;
    }
    sh.cache.insert(path.to_string(), (deps_h, doc.clone()));
    sh.sessions.retain(|s| s.path != path);
    drop(sh);
    send_line(out, &format!("{{\"event\":\"analysed\",\"ok\":true,\"path\":{},\"ms\":{},\"deps\":{},\"result\":{}}}", serde_json::json!(path), t0.elapsed().as_millis(), serde_json::json!(deps), doc));
}

/// The options without following installed packages.
fn unfollowed(opts: &Opts) -> Opts {
    Opts { follow: Vec::new(), probe: None, ..opts.clone() }
}

/// Does the program import a package `follow` names (`*`: any) that rowan has no model of and
/// that is installed in the site-packages it would read?
fn followable(imports: &[String], follow: &[String], path: &str, site: &[String]) -> bool {
    let wanted: Vec<&String> = imports.iter().filter(|n| follow.iter().any(|f| f == *n || f == "*") && !rowan::models::has_module_models(n)).collect();
    if wanted.is_empty() {
        return false;
    }
    let dirs = crate::site_packages_for(path, site);
    wanted.iter().any(|n| dirs.iter().any(|d| std::path::Path::new(d).join(n.as_str()).is_dir() || std::path::Path::new(d).join(format!("{}.py", n)).exists()))
}

/// An `analyse` request's analysis following installed packages, started once an analysis
/// without following saw the program import one (`probe`): at its answer, or when its whole
/// program's analysis completes later.
#[derive(Clone)]
struct FollowLater {
    opts: Opts,
    probe: std::sync::Arc<std::sync::atomic::AtomicBool>,
    spawned: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl FollowLater {
    /// Start it if it is due and not started; is it running (or done)?
    fn start(&self, shared: &std::sync::Arc<std::sync::Mutex<Shared>>, out: &Out, path: &str, generation: u64, t0: std::time::Instant) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        if !self.spawned.load(Relaxed) && self.probe.load(Relaxed) && shared.lock().unwrap().latest.get(path) == Some(&generation) {
            self.spawned.store(spawn_followed(shared, out, &self.opts, path, generation, t0), Relaxed);
        }
        self.spawned.load(Relaxed)
    }
}

/// For `analyse` request `generation` of `path`: the whole program's analysis following
/// installed packages, on a thread of its own; when it completes (not cut by its deadline) its
/// document replaces the answer as an `analysed` event (`followed`: true) and in the cache.
/// Nothing when a newer `analyse` came or a file it loaded changed. False if it cannot start.
fn spawn_followed(shared: &std::sync::Arc<std::sync::Mutex<Shared>>, out: &Out, opts: &Opts, path: &str, generation: u64, t0: std::time::Instant) -> bool {
    let cancel = shared.lock().unwrap().cancel_flag(path);
    let (rx, tx) = start_analysis(path.to_string(), opts.clone(), Start::Whole, cancel);
    let (shared, out, path) = (shared.clone(), out.clone(), path.to_string());
    std::thread::Builder::new()
        .spawn(move || {
            let _tx = tx;
            let Ok(Some(Ok((doc, deps)))) = rx.recv().map(whole_doc) else { return };
            let deps_h = hashed(&deps);
            let mut sh = shared.lock().unwrap();
            if sh.latest.get(&path) != Some(&generation) || !deps_h.iter().all(|(p, h)| file_hash(p) == Some(*h)) {
                return;
            }
            sh.followed.insert(path.clone(), generation);
            sh.cache.insert(path.clone(), (deps_h, doc.clone()));
            sh.sessions.retain(|s| s.path != path);
            drop(sh);
            send_line(&out, &format!("{{\"event\":\"analysed\",\"followed\":true,\"ok\":true,\"path\":{},\"ms\":{},\"deps\":{},\"result\":{}}}", serde_json::json!(path), t0.elapsed().as_millis(), serde_json::json!(deps), doc));
        })
        .is_ok()
}

/// A `hover` request: the function of `path` at `line`, from the analysis kept for `path` (set
/// up first, its module code alone, if there is none).
fn hover_request(shared: &std::sync::Arc<std::sync::Mutex<Shared>>, opts: &Opts, id: &serde_json::Value, path: &str, line: u32) -> String {
    let t0 = std::time::Instant::now();
    let mut setup = Ok(None);
    let has = shared.lock().unwrap().sessions.iter().any(|s| s.path == path);
    if !has {
        let cancel = shared.lock().unwrap().cancel_flag(path);
        setup = match spawn_analysis(path.to_string(), unfollowed(opts), Start::Module, cancel) {
            (Ok((_, deps, _)), Some(tx)) => {
                keep_session(&mut shared.lock().unwrap().sessions, Session { path: path.to_string(), deps: hashed(&deps), tx });
                Ok(Some(t0.elapsed().as_millis()))
            }
            (Err(e), _) => Err(e),
            (Ok(_), None) => Err("the file could not be analysed".to_string()),
        };
    }
    let t1 = std::time::Instant::now();
    let answer = setup.and_then(|setup_ms| {
        let tx = shared.lock().unwrap().sessions.iter().find(|s| s.path == path).map(|s| s.tx.clone()).ok_or("the file could not be analysed")?;
        let (rtx, rx) = std::sync::mpsc::channel();
        let _ = tx.send((line, rtx));
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
