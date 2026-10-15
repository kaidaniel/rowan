//! What user code may change beyond what it is given, for a call that does not run its body
//! (past its function's budget, or cut at the deadline): the module globals and closure cells
//! its code (and the code it can reach) names in a position where they may change or escape.

use ruff_python_ast::visitor::{walk_expr, walk_stmt, Visitor};
use ruff_python_ast::{Expr, Stmt};
use rustc_hash::{FxHashMap, FxHashSet};

use indexmap::IndexMap;

use crate::domain::{Ns, ObjId, Value};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, s, str_get, KeyId, StrId};
use crate::state::{Kind, State};

use super::{Analyzer, FuncInfo, Payload};

/// What the cache of code closures (`code_closure`) may hold; past it the cache starts over.
pub const CLOSURE_CACHE: usize = 1 << 18;

/// Caches of what code may change when it does not run. `code_uses` is a function of the
/// program text; a closure is computed once per set of roots from the state of its first
/// computation (the code a name holds is taken as fixed), so it is flow-insensitive state, not
/// only a cache.
#[derive(Default)]
pub struct EffectsCache {
    /// per function code (`FuncNode::addr`): the names it reads beyond its locals
    code_uses: FxHashMap<usize, std::rc::Rc<CodeUses>>,
    /// per code object: what it may change when a call of it does not run (`code_closure`)
    closures: FxHashMap<Vec<ObjId>, std::rc::Rc<Closure>>,
    /// what `closures` holds (roots, code, slots, values), bounded by `CLOSURE_CACHE`
    closures_size: usize,
}

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
    /// the attribute names it assigns (`x.a = …`, `setattr(x, "a", …)`) and deletes
    stored: FxHashSet<StrId>,
    deleted: FxHashSet<StrId>,
    /// the attribute names it reads at all (a method of a class runs only when its name is
    /// read, or for a dunder)
    reads: FxHashSet<StrId>,
    /// how it uses its first parameter (a method's receiver), None when it has none
    recv: Option<RecvUse>,
}

/// How a method's own code uses its receiver: the attribute names it reads, stores, deletes and
/// calls on it, the methods it calls through `super()`, or that the receiver escapes (passed
/// on, aliased, compared, used by a computed name, captured by a nested function).
#[derive(Default)]
pub struct RecvUse {
    escapes: bool,
    loads: FxHashSet<StrId>,
    stores: FxHashSet<StrId>,
    deletes: FxHashSet<StrId>,
    calls: FxHashSet<StrId>,
    supers: FxHashSet<StrId>,
    /// attribute names built from a template (`f"{name}_dynamic"`, as (start, end)) it reads,
    /// stores and deletes by `getattr` / `hasattr`, `setattr` and `delattr`
    loads_like: Vec<Like>,
    stores_like: Vec<Like>,
    deletes_like: Vec<Like>,
}

/// A name made by a template: its literal parts and its holes (a hole that is a name: the
/// name). Not knowing what the holes hold, it is any name that starts and ends as it does.
#[derive(Clone, PartialEq)]
pub struct Like(Vec<Result<String, Option<String>>>);

impl Like {
    pub fn matches(&self, k: KeyId) -> bool {
        let start: String = self.0.iter().map_while(|p| p.as_ref().ok()).cloned().collect();
        let end: String = self.0.iter().rev().map_while(|p| p.as_ref().ok()).cloned().collect::<Vec<String>>().into_iter().rev().collect();
        intern::key_as_str(k).is_some_and(|n| n.len() >= start.len() + end.len() && n.starts_with(&start) && n.ends_with(&end))
    }

    /// The one name it makes when each hole is a name `bind` knows the text of.
    pub fn resolve(&self, bind: &dyn Fn(&str) -> Option<String>) -> Option<String> {
        self.0.iter().map(|p| match p {
            Ok(l) => Some(l.clone()),
            Err(n) => bind(n.as_deref()?),
        }).collect()
    }
}

/// The scan behind `RecvUse` (`me`: the receiver's parameter name; `named`: locals bound to a
/// template, None when bound to anything else too).
struct RecvScan<'a> {
    me: &'a str,
    out: RecvUse,
    named: FxHashMap<String, Option<Like>>,
}

/// The template an f-string with holes is (`f"{a}_dynamic"`: "", "_dynamic").
fn template(e: &Expr) -> Option<Like> {
    let Expr::FString(f) = e else { return None };
    let mut parts: Vec<Result<String, Option<String>>> = Vec::new();
    for p in f.value.iter() {
        match p {
            ruff_python_ast::FStringPart::Literal(sl) => parts.push(Ok(sl.value.to_string())),
            ruff_python_ast::FStringPart::FString(fs) => {
                for el in fs.elements.iter() {
                    match el {
                        ruff_python_ast::InterpolatedStringElement::Literal(l) => parts.push(Ok(l.value.to_string())),
                        // (`{name}` without conversion or format: the name's text)
                        ruff_python_ast::InterpolatedStringElement::Interpolation(i) => parts.push(Err(match (&*i.expression, i.conversion, &i.format_spec) {
                            (Expr::Name(n), ruff_python_ast::ConversionFlag::None, None) => Some(n.id.to_string()),
                            _ => None,
                        })),
                    }
                }
            }
        }
    }
    parts.iter().any(|p| p.is_err()).then_some(Like(parts))
}

fn mentions(me: &str, st: Option<&Stmt>, e: Option<&Expr>) -> bool {
    struct M<'a>(&'a str, bool);
    impl<'b> Visitor<'b> for M<'_> {
        fn visit_expr(&mut self, e: &'b Expr) {
            match e {
                Expr::Name(n) if n.id.as_str() == self.0 => self.1 = true,
                _ => walk_expr(self, e),
            }
        }
    }
    let mut m = M(me, false);
    if let Some(s) = st {
        m.visit_stmt(s);
    }
    if let Some(e) = e {
        m.visit_expr(e);
    }
    m.1
}

impl RecvScan<'_> {
    /// `self.<name>` of an expression, if it is one.
    fn attr_of_me<'e>(&self, e: &'e Expr) -> Option<&'e str> {
        match e {
            Expr::Attribute(a) if matches!(&*a.value, Expr::Name(n) if n.id.as_str() == self.me) => Some(a.attr.as_str()),
            _ => None,
        }
    }

    fn target(&mut self, t: &Expr) {
        match t {
            _ if self.attr_of_me(t).is_some() => drop(self.out.stores.insert(s(self.attr_of_me(t).unwrap()))),
            Expr::Tuple(x) => x.elts.iter().for_each(|y| self.target(y)),
            Expr::List(x) => x.elts.iter().for_each(|y| self.target(y)),
            Expr::Starred(x) => self.target(&x.value),
            Expr::Name(n) if n.id.as_str() == self.me => self.out.escapes = true,
            // (a name bound again: no longer one template)
            Expr::Name(n) => drop(self.named.insert(n.id.to_string(), None)),
            _ => self.visit_expr(t),
        }
    }

    /// A builtin applied to the receiver that reads (or by a literal or template name stores /
    /// deletes) one of its attributes and keeps nothing.
    fn builtin_on_me(&mut self, f: &str, args: &[Expr]) -> bool {
        let lit = match args.get(1) {
            Some(Expr::StringLiteral(l)) => Some(l.value.to_str().to_string()),
            _ => None,
        };
        let like = match args.get(1) {
            Some(Expr::Name(n)) => self.named.get(n.id.as_str()).cloned().flatten(),
            Some(e) => template(e),
            None => None,
        };
        match (f, lit, like) {
            ("isinstance" | "issubclass" | "id" | "callable", _, _) => {}
            // (`type(self)` is `self.__class__`)
            ("type", _, _) => drop(self.out.loads.insert(s("__class__"))),
            ("getattr" | "hasattr", Some(n), _) => drop(self.out.loads.insert(s(&n))),
            ("setattr", Some(n), _) => drop(self.out.stores.insert(s(&n))),
            ("delattr", Some(n), _) => drop(self.out.deletes.insert(s(&n))),
            ("getattr" | "hasattr", _, Some(l)) => self.out.loads_like.push(l),
            ("setattr", _, Some(l)) => self.out.stores_like.push(l),
            ("delattr", _, Some(l)) => self.out.deletes_like.push(l),
            _ => return false,
        }
        args.iter().skip(1).for_each(|a| self.visit_expr(a));
        true
    }
}

impl<'a> Visitor<'a> for RecvScan<'_> {
    fn visit_stmt(&mut self, st: &'a Stmt) {
        match st {
            // (`return self`: the caller gets it back; no other code gets to change it)
            Stmt::Return(r) if matches!(r.value.as_deref(), Some(Expr::Name(n)) if n.id.as_str() == self.me) => {}
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => self.out.escapes |= mentions(self.me, Some(st), None),
            Stmt::Assign(a) => {
                for t in &a.targets {
                    let v = matches!(t, Expr::Name(n) if !self.named.contains_key(n.id.as_str())).then(|| template(&a.value)).flatten();
                    self.target(t);
                    if let Expr::Name(n) = t {
                        self.named.insert(n.id.to_string(), v);
                    }
                }
                self.visit_expr(&a.value);
            }
            Stmt::AugAssign(a) => {
                if let Some(n) = self.attr_of_me(&a.target) {
                    self.out.loads.insert(s(n));
                }
                self.target(&a.target);
                self.visit_expr(&a.value);
            }
            Stmt::AnnAssign(a) => {
                if let Some(v) = &a.value {
                    self.target(&a.target);
                    self.visit_expr(v);
                }
            }
            Stmt::Delete(d) => {
                for t in &d.targets {
                    match self.attr_of_me(t) {
                        Some(n) => drop(self.out.deletes.insert(s(n))),
                        None => self.visit_expr(t),
                    }
                }
            }
            Stmt::For(f) => {
                self.target(&f.target);
                self.visit_expr(&f.iter);
                self.visit_body(&f.body);
                self.visit_body(&f.orelse);
            }
            Stmt::With(w) => {
                for it in &w.items {
                    self.visit_expr(&it.context_expr);
                    if let Some(v) = &it.optional_vars {
                        self.target(v);
                    }
                }
                self.visit_body(&w.body);
            }
            _ => walk_stmt(self, st),
        }
    }

    fn visit_expr(&mut self, e: &'a Expr) {
        match e {
            Expr::Name(n) if n.id.as_str() == self.me => self.out.escapes = true,
            Expr::Lambda(_) => self.out.escapes |= mentions(self.me, None, Some(e)),
            _ if self.attr_of_me(e).is_some() => {
                let n = self.attr_of_me(e).unwrap();
                self.out.escapes |= n == "__dict__";
                self.out.loads.insert(s(n));
            }
            Expr::Call(c) => {
                let args = || c.arguments.args.iter().chain(c.arguments.keywords.iter().map(|k| &k.value));
                if let Some(m) = self.attr_of_me(&c.func) {
                    self.out.calls.insert(s(m));
                    args().for_each(|a| self.visit_expr(a));
                    return;
                }
                if let Expr::Attribute(a) = &*c.func {
                    if let Expr::Call(sc) = &*a.value {
                        let bare = sc.arguments.args.is_empty() || matches!(sc.arguments.args.get(1), Some(Expr::Name(n)) if n.id.as_str() == self.me);
                        if matches!(&*sc.func, Expr::Name(f) if f.id.as_str() == "super") && bare {
                            self.out.supers.insert(s(a.attr.as_str()));
                            args().for_each(|x| self.visit_expr(x));
                            return;
                        }
                    }
                }
                if let (Expr::Name(f), Some(Expr::Name(first))) = (&*c.func, c.arguments.args.first()) {
                    if first.id.as_str() == self.me && c.arguments.keywords.is_empty() && self.builtin_on_me(f.id.as_str(), &c.arguments.args) {
                        return;
                    }
                }
                walk_expr(self, e)
            }
            _ => walk_expr(self, e),
        }
    }
}

/// What a skipped method may do to its receiver (`receiver_effects`): the attribute names it
/// may store and delete, those it reads (what it reaches through the receiver), and the methods
/// that run with the receiver.
pub struct RecvEffects {
    pub stores: Vec<KeyId>,
    pub deletes: Vec<KeyId>,
    pub used: Vec<KeyId>,
    pub code: Vec<ObjId>,
    /// the templates of names it may store (new attributes too)
    pub stores_like: Vec<Like>,
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
                        let pure = super::PURE_METHODS.contains(&a.attr.as_str());
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
                                    match f {
                                        "delattr" => drop(self.out.deleted.insert(s(l.value.to_str()))),
                                        "setattr" => drop(self.out.stored.insert(s(l.value.to_str()))),
                                        _ => {}
                                    }
                                    if changes {
                                        self.attr(l.value.to_str());
                                    }
                                }
                                // (an attribute by a computed name: not followed)
                                _ => {}
                            }
                        }
                        super::PURE_BUILTINS.contains(&n.id.as_str())
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
                self.out.stored.insert(s(a.attr.as_str()));
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
            Stmt::Delete(d) => {
                for t in &d.targets {
                    if let Expr::Attribute(a) = t {
                        self.out.deleted.insert(s(a.attr.as_str()));
                    }
                    self.target(t);
                }
            }
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
        if let Some(u) = self.effects.code_uses.get(&addr) {
            return u.clone();
        }
        let mi = self.heap.module_info(fi.module);
        let scope = mi.scopes.get(fi.scope);
        let locals = |n: &str| scope.is_local(n);
        let mut c = Collect { locals: &locals, esc: true, out: CodeUses { uses: Vec::new(), imports: FxHashMap::default(), attrs: FxHashSet::default(), stored: FxHashSet::default(), deleted: FxHashSet::default(), reads: FxHashSet::default(), recv: None }, pkg: mi.pkg.clone() };
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
        if let (super::FuncNode::Def(f), Some(p)) = (fi.node, fi.node.params().and_then(|p| p.posonlyargs.iter().chain(p.args.iter()).next())) {
            let mut r = RecvScan { me: p.parameter.name.id.as_str(), out: RecvUse::default(), named: FxHashMap::default() };
            r.visit_body(&f.body);
            c.out.recv = Some(r.out);
        }
        let u = std::rc::Rc::new(c.out);
        self.effects.code_uses.insert(addr, u.clone());
        u
    }

    /// The attribute names the methods of `cls` (and its bases) other than `__init__` store on
    /// some object, and those they delete.
    pub(in crate::interp) fn attrs_set_by_methods(&mut self, cls: ObjId, state: &State) -> (Vec<KeyId>, Vec<KeyId>) {
        let (mut out, mut del): (FxHashSet<StrId>, FxHashSet<StrId>) = Default::default();
        for c in self.heap.mro(cls) {
            for (k, v) in state.obj_slots(c, Ns::Attr) {
                if intern::key_as_str(k).as_deref() == Some("__init__") {
                    continue;
                }
                for f in v.pts.iter() {
                    if let Payload::Func(fi) = &self.heap.obj(f).payload {
                        let fi = fi.clone();
                        let u = self.code_uses(&fi);
                        out.extend(u.stored.iter().copied());
                        del.extend(u.deleted.iter().copied());
                    }
                }
            }
        }
        let keys = |set: FxHashSet<StrId>| {
            let mut ks: Vec<KeyId> = set.into_iter().map(|a| key_str_lit(&str_get(a))).collect();
            ks.sort_unstable();
            ks
        };
        (keys(out), keys(del))
    }

    /// What a call of method `foid` that does not run may do to its receiver `recv` itself, by
    /// its own code and the methods and properties of the receiver's class that run with it:
    /// the attribute names it may store and delete, and the names it reads on it (what it can
    /// reach through the receiver is under those). None: anything (the receiver escapes,
    /// the class hooks attribute access or has a base rowan does not see, a descriptor).
    pub(in crate::interp) fn receiver_effects(&mut self, foid: ObjId, recv: ObjId, state: &State, bind: &dyn Fn(&str) -> Option<String>) -> Option<RecvEffects> {
        let cls = self.heap.instance_class(recv)?;
        let mro = self.heap.mro(cls);
        if mro.iter().any(|c| self.heap.class_info_opt(*c).is_none_or(|ci| ci.open_base)) || ["__getattr__", "__getattribute__", "__setattr__", "__delattr__"].iter().any(|h| self.class_lookup(cls, h).is_some()) {
            return None;
        }
        let (mut stores, mut deletes, mut used): (FxHashSet<StrId>, FxHashSet<StrId>, FxHashSet<StrId>) = Default::default();
        let (mut loads_like, mut stores_like, mut deletes_like): (Vec<Like>, Vec<Like>, Vec<Like>) = Default::default();
        let mut work = vec![foid];
        let mut done: FxHashSet<ObjId> = FxHashSet::default();
        let runs_with_recv = |an: &Analyzer, g: ObjId| an.heap.kind(g) == Kind::Function && !an.heap.func_info(g).is_static.get() && !an.heap.func_info(g).is_classmethod.get();
        while let Some(f) = work.pop() {
            if !done.insert(f) {
                continue;
            }
            let Payload::Func(fi) = &self.heap.obj(f).payload else { return None };
            let fi = fi.clone();
            let uses = self.code_uses(&fi);
            let u = uses.recv.as_ref().filter(|u| !u.escapes)?;
            stores.extend(u.stores.iter().copied());
            deletes.extend(u.deletes.iter().copied());
            // (the called method's own templates: the names its arguments make)
            let named = |an: &Analyzer, likes: &[Like], to: &mut FxHashSet<StrId>, rest: &mut Vec<Like>| -> Option<()> {
                for l in likes {
                    match l.resolve(bind).filter(|_| f == foid) {
                        Some(n) if an.class_lookup(cls, &n).is_some() => return None,
                        Some(n) => drop(to.insert(s(&n))),
                        None => rest.push(l.clone()),
                    }
                }
                Some(())
            };
            named(self, &u.loads_like, &mut used, &mut loads_like)?;
            named(self, &u.stores_like, &mut stores, &mut stores_like)?;
            named(self, &u.deletes_like, &mut deletes, &mut deletes_like)?;
            for &n in u.loads.iter().chain(u.stores.iter()).chain(u.deletes.iter()).chain(u.calls.iter()) {
                // (what a name only stored into or deleted held before is not the code's to reach)
                if u.loads.contains(&n) || u.calls.contains(&n) {
                    used.insert(n);
                }
                // (a name the class does not define is the instance's own attribute)
                let k = key_str_lit(&str_get(n));
                let Some(owner) = self.class_lookup(cls, &str_get(n)) else { continue };
                for g in self.heap.cls_slots.get(&(owner, Ns::Attr, k)).cloned().unwrap_or_default().pts.iter() {
                    match self.heap.kind(g) {
                        // (a method read without a call: a bound method holding the receiver)
                        Kind::Function if runs_with_recv(self, g) && u.loads.contains(&n) && !self.heap.func_info(g).is_property.get() => return None,
                        Kind::Function if runs_with_recv(self, g) => work.push(g),
                        // (a descriptor's code runs with the receiver)
                        Kind::Instance if self.heap.instance_class(g).is_some_and(|c| ["__get__", "__set__", "__delete__"].iter().any(|h| self.class_lookup(c, h).is_some())) => return None,
                        _ => {}
                    }
                }
                if u.stores.contains(&n) {
                    work.extend(self.heap.prop_setters.get(&(owner, k)).copied());
                }
            }
            for &n in u.supers.iter() {
                let k = key_str_lit(&str_get(n));
                for c in &mro {
                    let fv = self.heap.cls_slots.get(&(*c, Ns::Attr, k)).cloned().unwrap_or_default();
                    work.extend(fv.pts.iter().filter(|g| runs_with_recv(self, *g)));
                }
            }
        }
        // (names by a template: the receiver's attributes they name now — not one of the
        // class's, which may be a method bound to the receiver or a descriptor —, and new ones)
        let any_like = |likes: &[Like], k: KeyId| likes.iter().any(|l| l.matches(k));
        let all_like: Vec<Like> = loads_like.iter().chain(&stores_like).chain(&deletes_like).cloned().collect();
        if !all_like.is_empty() && mro.iter().any(|c| state.obj_slots(*c, Ns::Attr).iter().any(|(k, _)| any_like(&all_like, *k))) {
            return None;
        }
        let as_str = |k: KeyId| s(&intern::key_as_str(k).unwrap_or_default());
        for k in state.row(recv, Ns::Attr).present.iter() {
            if any_like(&loads_like, k) {
                used.insert(as_str(k));
            }
            if any_like(&stores_like, k) {
                stores.insert(as_str(k));
            }
            if any_like(&deletes_like, k) {
                deletes.insert(as_str(k));
            }
        }
        let keys = |set: FxHashSet<StrId>| {
            let mut ks: Vec<KeyId> = set.into_iter().map(|a| key_str_lit(&str_get(a))).collect();
            ks.sort_unstable();
            ks
        };
        let mut code: Vec<ObjId> = done.into_iter().collect();
        code.sort_unstable();
        // (a deleted name stored again: as a store, the row opened for both)
        stores_like.extend(deletes_like);
        Some(RecvEffects { stores: keys(stores), deletes: keys(deletes), used: keys(used), code, stores_like })
    }

    /// The attribute names a method's own code stores on its receiver on every way it
    /// completes: the top-level `self.x = …` statements before any statement that may return.
    pub(in crate::interp) fn must_stores(&self, foid: ObjId) -> Vec<KeyId> {
        let Payload::Func(fi) = &self.heap.obj(foid).payload else { return Vec::new() };
        let (super::FuncNode::Def(f), Some(me)) = (fi.node, fi.node.params().and_then(|p| p.posonlyargs.iter().chain(p.args.iter()).next())) else { return Vec::new() };
        let me = me.parameter.name.id.as_str();
        let mine = |t: &Expr| match t {
            Expr::Attribute(a) if matches!(&*a.value, Expr::Name(n) if n.id.as_str() == me) => Some(key_str_lit(a.attr.as_str())),
            _ => None,
        };
        let mut out = Vec::new();
        for st in f.body.iter().take_while(|st| !super::stmts::contains_return(std::slice::from_ref(*st)) && !super::is_generator(std::slice::from_ref(*st))) {
            match st {
                Stmt::Assign(a) => out.extend(a.targets.iter().filter_map(mine)),
                Stmt::AnnAssign(a) if a.value.is_some() => out.extend(mine(&a.target)),
                Stmt::AugAssign(a) => out.extend(mine(&a.target)),
                _ => {}
            }
        }
        out
    }

    /// Where `name` (read in the code of `fi`) is bound — a cell, a module's global — or the
    /// module it names (an import of a whole module in the function).
    fn resolve_use(&self, fi: &FuncInfo, uses: &CodeUses, name: StrId) -> Result<Loc, ObjId> {
        if let Some(&cell) = fi.closure.get(&name) {
            return Ok((cell, Ns::Cell, super::funcs::cell_key()));
        }
        if let Some(full) = uses.imports.get(&name) {
            if let Some(&m) = self.program.modules.get(full) {
                return Err(m);
            }
            if let Some((m, a)) = full.rsplit_once('.').and_then(|(m, a)| self.program.modules.get(m).map(|mo| (*mo, a))) {
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
    pub(in crate::interp) fn code_closure(&mut self, roots: &[ObjId], state: &State) -> std::rc::Rc<Closure> {
        if let Some(c) = self.effects.closures.get(roots) {
            return c.clone();
        }
        let mut out = Closure::default();
        let mut work: Vec<ObjId> = roots.to_vec();
        let (mut attrs, mut reads): (FxHashSet<StrId>, FxHashSet<StrId>) = Default::default();
        let mut classes: Vec<ObjId> = Vec::new();
        let (mut done, mut done_attrs): (FxHashSet<(ObjId, KeyId)>, FxHashSet<(ObjId, KeyId)>) = Default::default();
        let code = |an: &Analyzer, v: &Value, work: &mut Vec<ObjId>| work.extend(v.pts.iter().filter(|x| matches!(an.heap.kind(*x), Kind::Function | Kind::Class | Kind::Bound | Kind::Instance | Kind::Partial | Kind::Module)));
        loop {
            while let Some(o) = work.pop() {
                if !out.code.insert(o) {
                    continue;
                }
                match (self.heap.kind(o), &self.heap.obj(o).payload) {
                    (Kind::Function, Payload::Func(fi)) => {
                        let fi = fi.clone();
                        self.function_closure(&fi, state, &mut out, &mut attrs, &mut reads, &mut work);
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
                        if let Some(ci) = self.heap.class_info_opt(o) {
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
                // (a cache of what the program's code reaches, by root set: on large programs
                // the root sets rarely repeat, so it is emptied past a bound on what it holds)
                let size = roots.len() + c.code.len() + c.locs.len() + c.rebound.len() + c.values.len();
                self.effects.closures_size += size;
                if self.effects.closures_size > CLOSURE_CACHE {
                    self.effects.closures.clear();
                    self.effects.closures_size = size;
                }
                self.effects.closures.insert(roots.to_vec(), c.clone());
                return c;
            }
        }
    }

    /// A call of user code whose body does not run (past its budget, cut at the deadline): an
    /// unknown call that may also change what the code reaches beyond its arguments (the globals
    /// and cells it, and the code it can call, reads where they may change).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn skipped_call(&mut self, foid: ObjId, q: &str, self_v: Option<&Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> Value {
        // a method whose code touches its receiver only by attribute names (`self.x`, methods
        // run on it): the receiver's own attributes change only by those names, unless what
        // the call reaches otherwise reaches the receiver too (`Havoc::guard`); one of several
        // receivers (a method of each, past its budget) likewise, each by its names
        let recvs: Vec<ObjId> = self_v.map(|v| v.pts.to_vec()).filter(|r| !r.is_empty() && r.iter().all(|o| self.heap.kind(*o) == Kind::Instance)).unwrap_or_default();
        // (a literal argument's text, for the names its templates make)
        let fi = self.heap.func_info(foid);
        let positional: Vec<String> = fi.node.params().map(|p| p.posonlyargs.iter().chain(p.args.iter()).skip(usize::from(self_v.is_some())).map(|a| a.parameter.name.id.to_string()).collect()).unwrap_or_default();
        let bind = |n: &str| -> Option<String> {
            let given = positional.iter().position(|p| p == n).filter(|_| star_args.is_empty()).and_then(|i| args.get(i)).or_else(|| kwargs.get(&s(n)));
            let v = match given {
                Some(v) => v,
                None if star_kwargs.is_empty() && !positional.iter().position(|p| p == n).is_some_and(|i| i < args.len() || !star_args.is_empty()) => fi.defaults.get(&s(n))?,
                None => return None,
            };
            v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_str).map(|x| x.to_string())
        };
        let effs: Vec<(ObjId, RecvEffects)> = recvs.iter().map(|&o| Some((o, self.receiver_effects(foid, o, state, &bind)?))).collect::<Option<_>>().unwrap_or_default();
        // (on one receiver: the stores its code makes on every way it completes)
        let must: Vec<KeyId> = match effs.as_slice() {
            [(_, e)] if fi_is_method(self, foid) => self.must_stores(foid).into_iter().filter(|k| !e.deletes.contains(k)).collect(),
            _ => Vec::new(),
        };
        self.havoc.guard = effs.iter().map(|(o, e)| (*o, 0, std::rc::Rc::from(e.used.as_slice()))).collect();
        let walked = self.havoc.visited;
        let by_names = !effs.is_empty();
        let r = self.unknown_call(q, self_v, args, kwargs, star_args, star_kwargs, state, node);
        let site = s(&format!("{}@L{}", q, node.line));
        self.names.skipped_sites.insert(site);
        let mut roots = vec![foid];
        roots.append(&mut self.havoc.code);
        // (the methods that run with the receiver: what its code reaches through it)
        roots.extend(effs.iter().flat_map(|(_, e)| e.code.iter().copied()));
        let rid = r.single().unwrap_or(0);
        self.follow_code(roots, rid, site, state);
        let met: Vec<u32> = std::mem::take(&mut self.havoc.guard).into_iter().map(|(_, n, _)| n).collect();
        for ((o, RecvEffects { stores, deletes, stores_like, .. }), n) in effs.into_iter().zip(met) {
            if n > 1 {
                let walk = self.havoc.new_walk();
                self.havoc.walk_from(&mut self.heap, &[Value::obj(o)], rid, site, state, walk);
            } else {
                let any = Value::obj(crate::interp::any_object(&mut self.heap));
                let mut row = state.row(o, Ns::Attr);
                for k in stores {
                    let v = state.slot(o, Ns::Attr, k).join(&any);
                    state.set_slot(o, Ns::Attr, k, v, false);
                    row = row.store(k, false);
                }
                for k in deletes {
                    row = row.delete(k, false);
                }
                if !stores_like.is_empty() {
                    row = row.mutate_like(site, &|k| stores_like.iter().any(|l| l.matches(k)));
                }
                state.set_row(o, Ns::Attr, row);
            }
        }
        if super::debug_enabled() {
            eprintln!("skip {} at M{}:L{} havoc={}{}", q, node.module, node.line, self.havoc.visited - walked, if by_names { " (receiver by names)" } else { "" });
        }
        // (a raise after the code's effects on what it reaches beyond its arguments)
        self.flag_may_raise(state);
        // (what its own code stores on every way it completes is there after the call: the
        // raises above may come before)
        if let ([o], false) = (recvs.as_slice(), must.is_empty()) {
            let o = *o;
            let any = Value::obj(crate::interp::any_object(&mut self.heap));
            let mut row = state.row(o, Ns::Attr);
            for k in must {
                state.set_slot(o, Ns::Attr, k, any.clone(), true);
                row = row.store(k, true);
            }
            state.set_row(o, Ns::Attr, row);
        }
        r
    }

    /// What a function's code adds to a closure: the attribute names it changes and reads, its
    /// mutable defaults, and the slots of the names it reads (changed, rebound) with the code
    /// they hold.
    fn function_closure(&mut self, fi: &FuncInfo, state: &State, out: &mut Closure, attrs: &mut FxHashSet<StrId>, reads: &mut FxHashSet<StrId>, work: &mut Vec<ObjId>) {
        let code = |an: &Analyzer, v: &Value, work: &mut Vec<ObjId>| work.extend(v.pts.iter().filter(|x| matches!(an.heap.kind(*x), Kind::Function | Kind::Class | Kind::Bound | Kind::Instance | Kind::Partial | Kind::Module)));
        let uses = self.code_uses(&fi);
        attrs.extend(uses.attrs.iter().copied());
        reads.extend(uses.reads.iter().copied());
        // (a mutable default is the function's own state)
        for v in fi.defaults.values() {
            out.values.push(v.clone());
            code(self, v, work);
        }
        for u in &uses.uses {
            // (`m.x` of a module m: its global x)
            let (loc, in_module) = match (self.resolve_use(&fi, &uses, u.name), u.attr) {
                (Ok(l), None) => (l, false),
                (Ok(l), Some(a)) => match state.slot(l.0, l.1, l.2).single().filter(|m| self.heap.kind(*m) == Kind::Module) {
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
            code(self, &state.slot(loc.0, loc.1, loc.2), work);
        }
    }
}

/// A function run with a receiver (not a static / class method).
fn fi_is_method(an: &Analyzer, f: ObjId) -> bool {
    an.heap.func_info_opt(f).is_some_and(|fi| fi.cls.is_some() && !fi.is_static.get() && !fi.is_classmethod.get())
}
