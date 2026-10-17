//! The program: the main file, where imports are searched, the library models, and the
//! modules imported so far (an import runs the module once, as Python does; `TYPE_CHECKING`
//! is False, so its imports are only noted for annotations, `type_only`). Also the classes
//! the program defines (`user_classes`) and file names relative to the main file's directory.
//!
//! For a call memo: `modules`, `type_only` and `user_classes` grow as code runs (a run that
//! imports a module or defines a class changes them); the rest is fixed at the start.

use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;
use ruff_python_ast as ast;
use rustc_hash::FxHashMap;

use super::{Analyzer, ModuleInfo, Payload};
use crate::domain::{Ns, ObjId, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s, StrId};
use crate::models::Registry;
use crate::scopes as scopemod;
use crate::state::{Kind, State};

/// The program being analysed (see the module doc).
#[derive(Default)]
pub struct Program {
    /// the main file, as given
    pub filename: String,
    /// the main module's frontend index
    pub main_idx: u32,
    pub main_module: ObjId,
    /// where imports are looked for: the main file's directory, its package root, `--search-path`
    pub search_paths: Vec<String>,
    /// installed packages to follow into site-packages (top-level names; `*` for all)
    pub follow: Vec<String>,
    /// site-packages directories searched for followed packages
    pub site_paths: Vec<String>,
    /// run the functions the program does not call (`--no-entry`: not)
    pub analyse_entry_points: bool,
    /// the library models, by qualified name
    pub models: Registry,
    /// the modules imported so far, by name
    pub modules: IndexMap<String, ObjId>,
    /// names imported under `if TYPE_CHECKING:` (module, name) -> (module imported, name in it)
    pub type_only: FxHashMap<(ObjId, StrId), (String, Option<String>)>,
    /// the classes the program defines, in definition order
    pub user_classes: Vec<ObjId>,
    /// module index -> its path relative to the main file's directory (`rel_file`)
    rel_files: RefCell<FxHashMap<u32, String>>,
}

impl Program {
    pub fn new(filename: &str, main_idx: u32, search_paths: Vec<String>, analyse_entry_points: bool) -> Program {
        Program { filename: filename.to_string(), main_idx, search_paths, analyse_entry_points, ..Default::default() }
    }

    /// The path of module `idx` relative to the main file's directory ("" for the main
    /// module; the file name when it lies elsewhere, the module name when it has no file).
    pub fn rel_file(&self, idx: u32) -> String {
        if idx == self.main_idx {
            return String::new();
        }
        if let Some(f) = self.rel_files.borrow().get(&idx) {
            return f.clone();
        }
        let f = match frontend::module_path(idx) {
            Some(p) => self.rel_to_main(&p).unwrap_or_else(|| std::path::Path::new(&p).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or(p)),
            None => frontend::module_name(idx),
        };
        self.rel_files.borrow_mut().insert(idx, f.clone());
        f
    }

    /// Path `p` relative to the main file's directory, when it lies there or below.
    fn rel_to_main(&self, p: &str) -> Option<String> {
        let canon = |p: &str| std::fs::canonicalize(p).map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string());
        let dir = std::path::Path::new(&canon(&self.filename)).parent()?.to_path_buf();
        std::path::Path::new(&canon(p)).strip_prefix(&dir).ok().map(|r| r.to_string_lossy().to_string())
    }

    /// Is module `idx` (not the main one) the program's own: in the main file's directory or
    /// below, not in site-packages?
    pub fn is_local_idx(&self, idx: u32) -> bool {
        frontend::module_path(idx).map(|p| !p.contains("site-packages") && self.rel_to_main(&p).is_some()).unwrap_or(false)
    }
}

impl Analyzer {
    /// A module of the program itself: the main file's directory or below.
    pub fn is_local_module(&self, m: ObjId) -> bool {
        if m == self.program.main_module {
            return true;
        }
        self.program.is_local_idx(self.heap.module_info(m).idx)
    }

    /// The path of the running module relative to the main file's directory ("" for the main
    /// module).
    pub fn file_of_frame(&self) -> String {
        let m = self.frame().module;
        if m == self.program.main_module {
            return String::new();
        }
        let idx = self.heap.module_info(m).idx;
        self.program.rel_file(idx)
    }

    // ------------------------------------------------------------------ setup
    /// A module object (registered under `name`) for a parsed tree; `path` is its file, a
    /// namespace package's directory, or None (a module made by a model).
    pub fn make_module(&mut self, name: &str, path: Option<&str>, idx: u32, tree: &'static ast::ModModule) -> ObjId {
        let pkg = match path {
            Some(p) if name == "__main__" => package_of_file(p),
            Some(p) if p.ends_with("__init__.py") || std::path::Path::new(p).is_dir() => name.to_string(),
            _ => name.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_default(),
        };
        let info = ModuleInfo { name: name.to_string(), path: path.map(|p| p.to_string()), pkg, idx, tree, scopes: scopemod::analyse(tree) };
        let oid = self.alloc_global(Kind::Module, &format!("module:{}", name), &format!("module {}", name), Payload::Module(Rc::new(info)));
        self.program.modules.insert(name.to_string(), oid);
        oid
    }

    /// The module `from M import …` names (relative imports resolve against the current
    /// module's package).
    pub(super) fn import_from_module(&self, i: &ast::StmtImportFrom) -> String {
        if i.level > 0 {
            let pkg = self.heap.module_info(self.frame().module).pkg.clone();
            let mut base: Vec<&str> = pkg.split('.').filter(|x| !x.is_empty()).collect();
            for _ in 1..i.level {
                base.pop();
            }
            let mut full = base.join(".");
            if let Some(m) = &i.module {
                if !full.is_empty() {
                    full.push('.');
                }
                full.push_str(m.id.as_str());
            }
            full
        } else {
            i.module.as_ref().map(|m| m.id.to_string()).unwrap_or_default()
        }
    }

    /// The names an `if TYPE_CHECKING:` block imports (it does not run): for annotations only,
    /// resolved once the module they name is loaded (`type_only_name`).
    pub(super) fn note_type_only_imports(&mut self, body: &'static [ast::Stmt]) {
        let m = self.frame().module;
        for st in body {
            match st {
                ast::Stmt::ImportFrom(i) => {
                    let modname = self.import_from_module(i);
                    for a in &i.names {
                        let bound = a.asname.as_ref().unwrap_or(&a.name).id.as_str();
                        self.program.type_only.insert((m, s(bound)), (modname.clone(), Some(a.name.id.to_string())));
                    }
                }
                ast::Stmt::Import(i) => {
                    for a in i.names.iter().filter(|a| a.asname.is_some() || !a.name.id.contains('.')) {
                        let bound = a.asname.as_ref().unwrap_or(&a.name).id.as_str();
                        self.program.type_only.insert((m, s(bound)), (a.name.id.to_string(), None));
                    }
                }
                _ => {}
            }
        }
    }

    /// A name an `if TYPE_CHECKING:` block of the running module imports, once its module is
    /// loaded (nothing is imported for it).
    pub fn type_only_name(&self, name: &str, state: &State) -> Option<Value> {
        let (modname, attr) = self.program.type_only.get(&(self.frame().module, s(name)))?;
        let mo = *self.program.modules.get(modname)?;
        match attr {
            Some(a) => state.slot_opt(mo, Ns::Attr, key_str_lit(a)),
            None => Some(Value::obj(mo)),
        }
    }

    /// `from M import *`: the names in M's `__all__` (a submodule of a package for a name it
    /// does not define), or without a known one every name of M not starting with `_`.
    pub(super) fn import_star(&mut self, mod_v: &Value, modname: &str, n: &Node, state: &mut State) {
        let Some(mo) = mod_v.single().filter(|o| self.heap.kind(*o) == Kind::Module) else { return };
        let all = state.slot_opt(mo, Ns::Attr, key_str_lit("__all__")).map(|v| self.iterate(&v, n, state).items);
        let names: Vec<String> = match all.flatten().and_then(|items| items.iter().map(|v| v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_str)).collect::<Option<Vec<_>>>()) {
            Some(ns) => ns.iter().map(|x| x.to_string()).collect(),
            None => state.obj_slots(mo, Ns::Attr).into_iter().filter_map(|(k, _)| intern::key_as_str(k)).filter(|x| !x.starts_with('_')).map(|x| x.to_string()).collect(),
        };
        for name in names {
            let mut v = state.slot_opt(mo, Ns::Attr, key_str_lit(&name)).unwrap_or_default();
            if v.pts.is_empty() && v.kv.is_bot() {
                v = self.import_module(&format!("{}.{}", modname, name), n, state);
            }
            if v.pts.is_empty() && v.kv.is_bot() {
                v = self.load_attr(mod_v, &name, n, state, false);
            }
            self.store_name(s(&name), v, state);
        }
    }

    /// `import a.b.c`: the parents are imported first and each submodule becomes an attribute
    /// of its package; a module that is not found is an external object.
    pub fn import_module(&mut self, name: &str, node: &Node, state: &mut State) -> Value {
        if let Some(&m) = self.program.modules.get(name) {
            return Value::obj(m);
        }
        if let Some((parent, leaf)) = name.rsplit_once('.') {
            let pv = self.import_module(parent, node, state);
            let mut found: Option<Value> = None;
            if let Some(po) = pv.single() {
                if self.heap.kind(po) == Kind::Module {
                    // (a package's directory: its `__init__.py`'s, or a namespace package's own)
                    let ppath = self.heap.module_info(po).path.clone().map(std::path::PathBuf::from);
                    let dir = ppath.and_then(|p| if p.ends_with("__init__.py") { p.parent().map(|d| d.to_path_buf()) } else if p.is_dir() { Some(p) } else { None });
                    if let Some(dir) = dir {
                        found = self.load_module_file(name, &dir, leaf, state).or_else(|| self.load_namespace(name, &dir, leaf));
                    }
                    if let Some(v) = &found {
                        state.set_slot(po, Ns::Attr, key_str_lit(leaf), v.clone(), true);
                    }
                }
            }
            if let Some(v) = found {
                return v;
            }
        } else {
            for base in self.program.search_paths.clone() {
                let dir = std::path::PathBuf::from(&base);
                if let Some(v) = self.load_module_file(name, &dir, name, state) {
                    return v;
                }
            }
            // a directory without `__init__.py` is a namespace package (PEP 420) when no module
            // or regular package of the name is found (a modelled library's name is not taken)
            if !crate::models::has_module_models(name) {
                for base in self.program.search_paths.clone() {
                    if let Some(v) = self.load_namespace(name, std::path::Path::new(&base), name) {
                        return v;
                    }
                }
            }
            // an installed package on the allowlist is analysed from site-packages, unless a
            // model covers it (the model knows more than its source would yield)
            let followed = self.program.follow.iter().any(|f| f == name || f == "*");
            if followed && !crate::models::has_module_models(name) {
                for base in self.program.site_paths.clone() {
                    let dir = std::path::PathBuf::from(&base);
                    if let Some(v) = self.load_module_file(name, &dir, name, state) {
                        return v;
                    }
                }
            }
        }
        let _ = node;
        if name == "torch" || name.starts_with("torch.") {
            self.m.torch_on = true;
            self.m.torch_real = true;
            let m = self.frame().module;
            self.entry.torch_modules.insert(m);
        }
        // numpy arrays are tensors to the shape model
        if name == "numpy" || name.starts_with("numpy.") {
            self.m.torch_on = true;
            let m = self.frame().module;
            self.entry.torch_modules.insert(m);
        }
        let oid = self.heap.alloc_raw(Kind::External, &format!("ext:{}", name), "", true, name, Payload::Str(name.to_string()));
        Value::obj(oid)
    }

    /// Load `dir/leaf/__init__.py` or `dir/leaf.py` as module `name` (a file already loaded
    /// under another name is shared).
    pub(super) fn load_module_file(&mut self, name: &str, dir: &std::path::Path, leaf: &str, state: &mut State) -> Option<Value> {
        let cands = [dir.join(leaf).join("__init__.py"), dir.join(format!("{}.py", leaf))];
        for p in cands.iter() {
            if !p.exists() {
                continue;
            }
            let real = std::fs::canonicalize(p).map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string_lossy().to_string());
            for (_, &moid) in self.program.modules.clone().iter() {
                let mi = self.heap.module_info(moid);
                if let Some(mp) = &mi.path {
                    let mreal = std::fs::canonicalize(mp).map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|_| mp.clone());
                    if mreal == real {
                        self.program.modules.insert(name.to_string(), moid);
                        return Some(Value::obj(moid));
                    }
                }
            }
            let src = match std::fs::read_to_string(p) {
                Ok(s) => s,
                Err(_) => return None,
            };
            let ps = p.to_string_lossy().to_string();
            return match frontend::parse(name, Some(&ps), src) {
                Ok((idx, tree)) => {
                    let moid = self.make_module(name, Some(&ps), idx, tree);
                    self.run_module(moid, state);
                    Some(Value::obj(moid))
                }
                Err(e) => {
                    self.findings.note(format!("could not parse {}: {}", ps, e));
                    None
                }
            };
        }
        None
    }

    /// `dir/leaf`, a directory without `__init__.py`, as namespace package `name`: an empty
    /// module whose path is the directory (its submodules are found there).
    pub(super) fn load_namespace(&mut self, name: &str, dir: &std::path::Path, leaf: &str) -> Option<Value> {
        let d = dir.join(leaf);
        if !d.is_dir() || d.join("__init__.py").exists() {
            return None;
        }
        let ps = d.to_string_lossy().to_string();
        let (idx, tree) = frontend::parse(name, Some(&ps), String::new()).ok()?;
        Some(Value::obj(self.make_module(name, Some(&ps), idx, tree)))
    }
}

/// The dotted package a file belongs to: the chain of enclosing directories that have an
/// `__init__.py` (empty for a file outside any package).
pub fn package_of_file(path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut dir = std::path::Path::new(path).parent().map(|p| p.to_path_buf());
    while let Some(d) = dir {
        if !d.join("__init__.py").exists() {
            break;
        }
        match d.file_name() {
            Some(n) => parts.push(n.to_string_lossy().to_string()),
            None => break,
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }
    parts.reverse();
    parts.join(".")
}

/// The directory that holds the outermost package of `path` (the import root for `import
/// pkg.sub` from inside the package), when the file is inside a package.
pub fn package_root_of_file(path: &str) -> Option<String> {
    let mut dir = std::path::Path::new(path).parent().map(|p| p.to_path_buf())?;
    if !dir.join("__init__.py").exists() {
        return None;
    }
    while dir.join("__init__.py").exists() {
        dir = dir.parent()?.to_path_buf();
    }
    Some(dir.to_string_lossy().to_string())
}
