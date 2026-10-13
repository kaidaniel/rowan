//! What user code may change beyond what it is given, for a call that does not run its body
//! (past its function's budget, or cut at the deadline): the module globals and closure cells
//! its code (and the code it can reach) names in a position where they may change or escape.

use ruff_python_ast::visitor::{walk_expr, walk_stmt, Visitor};
use ruff_python_ast::{Expr, Stmt};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::domain::{Ns, ObjId, Value};
use crate::intern::{self, key_str_lit, s, str_get, KeyId, StrId};
use crate::state::{Kind, State};

use super::{Analyzer, FuncInfo, Payload};

/// A name the code reads that is not its own local: (name, the attribute read on it, whether
/// what it holds may change or escape — stored into, deleted from, given to a call that may
/// keep or change it, returned, aliased —, whether the code rebinds it).
pub struct Use {
    name: StrId,
    attr: Option<StrId>,
    changes: bool,
    rebinds: bool,
}

/// The names a function's code (its nested functions, lambdas and comprehensions included)
/// reads beyond its locals, the modules its own imports bind, and the attribute names it may
/// change on any object.
pub struct CodeUses {
    uses: Vec<Use>,
    imports: FxHashMap<StrId, String>,
    attrs: FxHashSet<StrId>,
    /// the attribute names it reads at all (a method of a class runs only when its name is
    /// read, or for a dunder)
    reads: FxHashSet<StrId>,
}

/// A slot: (object, namespace, key).
type Loc = (ObjId, Ns, KeyId);

/// What a piece of code may change beyond what it is given (`code_closure`).
#[derive(Default)]
pub struct Closure {
    /// the code it reaches
    pub code: FxHashSet<ObjId>,
    pub locs: Vec<Loc>,
    pub rebound: Vec<Loc>,
    pub values: Vec<Value>,
}

/// Method names that neither change their receiver nor keep their arguments.
const PURE_METHODS: &[&str] = &[
    "get", "keys", "values", "items", "copy", "count", "index", "startswith", "endswith", "format", "join", "split", "rsplit", "strip", "lstrip", "rstrip", "lower",
    "upper", "replace", "find", "rfind", "encode", "decode", "isdigit", "isalpha", "isspace", "splitlines", "partition", "rpartition", "casefold", "title",
];
/// Builtins that neither change nor keep their arguments.
const PURE_BUILTINS: &[&str] = &["len", "isinstance", "issubclass", "print", "str", "repr", "int", "float", "bool", "hash", "id", "type", "hasattr", "abs", "round", "format"];

struct Collect<'a> {
    locals: &'a dyn Fn(&str) -> bool,
    esc: bool,
    out: CodeUses,
    pkg: String,
}

impl Collect<'_> {
    fn name(&mut self, n: &str, attr: Option<&str>, changes: bool, rebinds: bool) {
        if !(self.locals)(n) || self.out.imports.contains_key(&s(n)) {
            self.out.uses.push(Use { name: s(n), attr: attr.map(s), changes, rebinds });
        }
    }

    fn expr(&mut self, e: &Expr, esc: bool) {
        match e {
            Expr::Name(n) => self.name(n.id.as_str(), None, esc, false),
            Expr::Attribute(a) => {
                self.out.reads.insert(s(a.attr.as_str()));
                if esc {
                    self.attr(a.attr.as_str());
                }
                match &*a.value {
                    Expr::Name(n) => self.name(n.id.as_str(), Some(a.attr.as_str()), esc, false),
                    v => self.expr(v, esc),
                }
            }
            Expr::Subscript(x) => {
                self.expr(&x.value, esc);
                self.expr(&x.slice, false);
            }
            Expr::Call(c) => {
                let pure = match &*c.func {
                    Expr::Attribute(a) => {
                        self.out.reads.insert(s(a.attr.as_str()));
                        let pure = PURE_METHODS.contains(&a.attr.as_str());
                        match &*a.value {
                            Expr::Name(n) => self.name(n.id.as_str(), Some(a.attr.as_str()), esc || !pure, false),
                            v => self.expr(v, esc || !pure),
                        }
                        pure
                    }
                    Expr::Name(n) => {
                        self.name(n.id.as_str(), None, true, false);
                        // (an attribute by a name in the text: that one; else any — read,
                        // and changed where the value escapes or is set)
                        let f = n.id.as_str();
                        if matches!(f, "setattr" | "delattr" | "getattr" | "hasattr" | "vars") {
                            let changes = matches!(f, "setattr" | "delattr") || (esc && matches!(f, "getattr" | "vars"));
                            match c.arguments.args.get(1) {
                                Some(Expr::StringLiteral(l)) if f != "vars" => {
                                    self.out.reads.insert(s(l.value.to_str()));
                                    if changes {
                                        self.attr(l.value.to_str());
                                    }
                                }
                                // (an attribute by a computed name: not followed)
                                _ => {}
                            }
                        }
                        PURE_BUILTINS.contains(&n.id.as_str())
                    }
                    f => {
                        self.expr(f, true);
                        false
                    }
                };
                for a in c.arguments.args.iter().chain(c.arguments.keywords.iter().map(|k| &k.value)) {
                    self.expr(a, !pure);
                }
            }
            Expr::Compare(c) => {
                self.expr(&c.left, false);
                c.comparators.iter().for_each(|x| self.expr(x, false));
            }
            Expr::UnaryOp(u) => self.expr(&u.operand, false),
            Expr::If(i) => {
                self.expr(&i.test, false);
                self.expr(&i.body, esc);
                self.expr(&i.orelse, esc);
            }
            Expr::FString(_) => self.walk(e, false),
            Expr::Yield(_) | Expr::YieldFrom(_) | Expr::Await(_) | Expr::Named(_) | Expr::Lambda(_) => self.walk(e, true),
            _ => self.walk(e, esc),
        }
    }

    fn walk(&mut self, e: &Expr, esc: bool) {
        let saved = std::mem::replace(&mut self.esc, esc);
        walk_expr(self, e);
        self.esc = saved;
    }

    /// An assignment / deletion / loop target.
    fn target(&mut self, t: &Expr) {
        match t {
            Expr::Name(n) => self.name(n.id.as_str(), None, false, true),
            Expr::Attribute(a) => {
                self.out.reads.insert(s(a.attr.as_str()));
                self.attr(a.attr.as_str());
                match &*a.value {
                    Expr::Name(n) => self.name(n.id.as_str(), Some(a.attr.as_str()), true, true),
                    v => self.expr(v, true),
                }
            }
            Expr::Subscript(x) => {
                self.expr(&x.value, true);
                self.expr(&x.slice, false);
            }
            Expr::Tuple(t) => t.elts.iter().for_each(|x| self.target(x)),
            Expr::List(t) => t.elts.iter().for_each(|x| self.target(x)),
            Expr::Starred(x) => self.target(&x.value),
            e => self.expr(e, true),
        }
    }

    fn attr(&mut self, a: &str) {
        self.out.attrs.insert(s(a));
    }

    fn import(&mut self, local: &str, module: String) {
        self.out.imports.insert(s(local), module);
    }
}

impl<'a> Visitor<'a> for Collect<'_> {
    fn visit_expr(&mut self, e: &'a Expr) {
        let esc = self.esc;
        self.expr(e, esc);
    }

    fn visit_stmt(&mut self, st: &'a Stmt) {
        self.esc = true;
        match st {
            Stmt::Expr(x) => self.expr(&x.value, false),
            Stmt::Assign(a) => {
                self.expr(&a.value, true);
                a.targets.iter().for_each(|t| self.target(t));
            }
            Stmt::AugAssign(a) => {
                self.expr(&a.value, true);
                self.expr(&a.target, true);
                self.target(&a.target);
            }
            Stmt::AnnAssign(a) => {
                if let Some(v) = &a.value {
                    self.expr(v, true);
                    self.target(&a.target);
                }
            }
            Stmt::Delete(d) => d.targets.iter().for_each(|t| self.target(t)),
            Stmt::For(f) => {
                self.expr(&f.iter, true);
                self.target(&f.target);
                self.visit_body(&f.body);
                self.visit_body(&f.orelse);
            }
            Stmt::If(i) => {
                self.expr(&i.test, false);
                self.visit_body(&i.body);
                for c in &i.elif_else_clauses {
                    if let Some(t) = &c.test {
                        self.expr(t, false);
                    }
                    self.visit_body(&c.body);
                }
            }
            Stmt::While(w) => {
                self.expr(&w.test, false);
                self.visit_body(&w.body);
                self.visit_body(&w.orelse);
            }
            Stmt::Assert(a) => {
                self.expr(&a.test, false);
                if let Some(m) = &a.msg {
                    self.expr(m, false);
                }
            }
            Stmt::With(w) => {
                for it in &w.items {
                    self.expr(&it.context_expr, true);
                    if let Some(v) = &it.optional_vars {
                        self.target(v);
                    }
                }
                self.visit_body(&w.body);
            }
            Stmt::Import(i) => {
                for a in &i.names {
                    let full = a.name.id.as_str();
                    match &a.asname {
                        Some(x) => self.import(x.as_str(), full.to_string()),
                        None => self.import(full.split('.').next().unwrap_or(full), full.split('.').next().unwrap_or(full).to_string()),
                    }
                }
            }
            Stmt::ImportFrom(i) => {
                let mut base: Vec<String> = if i.level > 0 { self.pkg.split('.').filter(|x| !x.is_empty()).map(String::from).collect() } else { Vec::new() };
                for _ in 1..i.level {
                    base.pop();
                }
                base.extend(i.module.iter().map(|m| m.id.to_string()));
                for a in i.names.iter().filter(|a| a.name.as_str() != "*") {
                    let local = a.asname.as_ref().unwrap_or(&a.name).as_str();
                    self.import(local, format!("{}.{}", base.join("."), a.name.as_str()));
                }
            }
            _ => walk_stmt(self, st),
        }
    }
}

impl Analyzer {
    /// The names a function's code reads beyond its locals (computed once per `def`).
    fn code_uses(&mut self, fi: &FuncInfo) -> std::rc::Rc<CodeUses> {
        let addr = fi.node.addr();
        if let Some(u) = self.code_uses.get(&addr) {
            return u.clone();
        }
        let mi = self.module_info(fi.module);
        let scope = mi.scopes.get(fi.scope);
        let locals = |n: &str| scope.is_local(n);
        let mut c = Collect { locals: &locals, esc: true, out: CodeUses { uses: Vec::new(), imports: FxHashMap::default(), attrs: FxHashSet::default(), reads: FxHashSet::default() }, pkg: mi.pkg.clone() };
        // (imports first: a use before the import statement in the text binds the same name)
        for st in super::access::walk_stmts(fi.node.body()) {
            if matches!(st, Stmt::Import(_) | Stmt::ImportFrom(_)) {
                c.visit_stmt(st);
            }
        }
        match fi.node {
            super::FuncNode::Def(f) => {
                for d in f.parameters.iter_non_variadic_params().filter_map(|p| p.default.as_deref()) {
                    c.expr(d, true);
                }
                c.visit_body(&f.body);
            }
            super::FuncNode::Lambda(l) => c.expr(&l.body, true),
        }
        let u = std::rc::Rc::new(c.out);
        self.code_uses.insert(addr, u.clone());
        u
    }

    /// Where `name` (read in the code of `fi`) is bound — a cell, a module's global — or the
    /// module it names (an import of a whole module in the function).
    fn resolve_use(&self, fi: &FuncInfo, uses: &CodeUses, name: StrId) -> Result<Loc, ObjId> {
        if let Some(&cell) = fi.closure.get(&name) {
            return Ok((cell, Ns::Cell, super::funcs::cell_key()));
        }
        if let Some(full) = uses.imports.get(&name) {
            if let Some(&m) = self.modules.get(full) {
                return Err(m);
            }
            if let Some((m, a)) = full.rsplit_once('.').and_then(|(m, a)| self.modules.get(m).map(|mo| (*mo, a))) {
                return Ok((m, Ns::Attr, key_str_lit(a)));
            }
        }
        Ok((fi.module, Ns::Attr, key_str_lit(&str_get(name))))
    }

    /// What the code `roots` hold (functions, bound methods, classes, instances' classes) may
    /// change when it runs, through the code it can reach by the names it reads (functions,
    /// classes, the methods of those classes it reads, dunders): the slots whose values may
    /// change (globals, cells, class attributes by name) or be rebound, and other values it may
    /// change (mutable defaults, bound receivers). Computed once per set of roots: the code a
    /// name holds is taken as fixed (rebinding a function or class is monkey-patching), and
    /// methods called by a computed name are not followed; the code in the data those values
    /// hold is followed where the call is (`skipped_call`).
    pub fn code_closure(&mut self, roots: &[ObjId], state: &State) -> std::rc::Rc<Closure> {
        if let Some(c) = self.closures.get(roots) {
            return c.clone();
        }
        let mut out = Closure::default();
        let mut work: Vec<ObjId> = roots.to_vec();
        let (mut attrs, mut reads): (FxHashSet<StrId>, FxHashSet<StrId>) = Default::default();
        let mut classes: Vec<ObjId> = Vec::new();
        let (mut done, mut done_attrs): (FxHashSet<(ObjId, KeyId)>, FxHashSet<(ObjId, KeyId)>) = Default::default();
        let code = |an: &Analyzer, v: &Value, work: &mut Vec<ObjId>| work.extend(v.pts.iter().filter(|x| matches!(an.kind(*x), Kind::Function | Kind::Class | Kind::Bound | Kind::Instance | Kind::Partial | Kind::Module)));
        loop {
            while let Some(o) = work.pop() {
                if !out.code.insert(o) {
                    continue;
                }
                match (self.kind(o), &self.obj(o).payload) {
                    (Kind::Function, Payload::Func(fi)) => {
                        let fi = fi.clone();
                        let uses = self.code_uses(&fi);
                        attrs.extend(uses.attrs.iter().copied());
                        reads.extend(uses.reads.iter().copied());
                        // (a mutable default is the function's own state)
                        for v in fi.defaults.values() {
                            out.values.push(v.clone());
                            code(self, v, &mut work);
                        }
                        for u in &uses.uses {
                            // (`m.x` of a module m: its global x)
                            let (loc, in_module) = match (self.resolve_use(&fi, &uses, u.name), u.attr) {
                                (Ok(l), None) => (l, false),
                                (Ok(l), Some(a)) => match state.slot(l.0, l.1, l.2).single().filter(|m| self.kind(*m) == Kind::Module) {
                                    Some(m) => ((m, Ns::Attr, key_str_lit(&str_get(a))), true),
                                    None => (l, false),
                                },
                                (Err(m), Some(a)) => ((m, Ns::Attr, key_str_lit(&str_get(a))), true),
                                (Err(m), None) => {
                                    work.push(m);
                                    continue;
                                }
                            };
                            if u.rebinds && (u.attr.is_none() || in_module) {
                                out.rebound.push(loc);
                            }
                            if u.changes {
                                out.locs.push(loc);
                            }
                            code(self, &state.slot(loc.0, loc.1, loc.2), &mut work);
                        }
                    }
                    (Kind::Module, _) => {
                        // (a module used as a value: any of its globals)
                        for (k, v) in state.obj_slots(o, Ns::Attr) {
                            out.locs.push((o, Ns::Attr, k));
                            code(self, &v, &mut work);
                        }
                    }
                    (Kind::Class, _) => {
                        classes.push(o);
                        if let Some(ci) = self.class_info_opt(o) {
                            work.extend(ci.bases.iter().copied());
                        }
                    }
                    (Kind::Instance, Payload::Cls(c)) => work.push(*c),
                    (Kind::Bound, Payload::Bound(f, recv)) => {
                        work.push(*f);
                        out.values.push(recv.clone());
                    }
                    (Kind::Partial, Payload::Partial(f, args, kw)) => {
                        code(self, f, &mut work);
                        out.values.extend(args.iter().chain(kw.values()).cloned());
                    }
                    _ => {}
                }
            }
            // the methods of the classes reached that the code reads (dunders run implicitly),
            // and their attributes it may change
            for &c in &classes {
                for (k, v) in state.obj_slots(c, Ns::Attr) {
                    let name = intern::key_as_str(k).map(|n| s(&n));
                    let has = |set: &FxHashSet<StrId>| name.is_some_and(|n| set.contains(&n));
                    if (has(&reads) || name.is_some_and(|n| str_get(n).starts_with("__"))) && done.insert((c, k)) {
                        code(self, &v, &mut work);
                    }
                    if has(&attrs) && done_attrs.insert((c, k)) {
                        out.locs.push((c, Ns::Attr, k));
                    }
                }
            }
            if work.is_empty() {
                out.locs.sort_unstable();
                out.locs.dedup();
                let c = std::rc::Rc::new(out);
                self.closures.insert(roots.to_vec(), c.clone());
                return c;
            }
        }
    }
}
