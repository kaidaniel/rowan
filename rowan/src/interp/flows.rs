//! What a function's code does with the values it handles, from its text alone (flow-
//! insensitive, by names): for calls whose bodies do not run (`effects.rs`), which values that
//! exist before the call it may change, keep or give away.
//!
//! A value *changes* where code rowan does not follow precisely may change or keep it: it is
//! given to a call (not to builtins and methods known to keep nothing), stored, returned or
//! yielded, put in a container, bound to a non-local name, or a method that may change its
//! receiver is called on it. A local name is an alias of what is assigned to it, so `x = G`
//! changes nothing until `x` does. What a value *holds* (its items, its attributes' values) is
//! told apart from the value: `lib(G.x)`, `for v in G: lib(v)` give away what `G` holds, not `G`.

use ruff_python_ast::{self as ast, Expr, Stmt};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::intern::{key_as_str, s, KeyId, StrId};

/// Where a value comes from.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Origin {
    /// a parameter of the function (its value when the call binds it)
    Param(StrId),
    /// a name the function does not bind (a global, a cell, a module its imports bind)
    Name(StrId),
    /// `n.a` of such a name: a module's global `a` when `n` is a module, else what `n` holds
    NameAttr(StrId, StrId),
    /// the value of attribute `a` of anything (`e.a`): a class attribute `a` among others
    Attr(StrId),
    /// the class of a value (`type(e)`, `e.__class__`)
    TypeOf,
    /// what a call of a name the function does not bind returns (`f(…)`; `n.f(…)`: Some(f))
    Result(StrId, Option<StrId>),
}

/// The value an origin gives, or something it holds.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Level {
    /// the value itself
    Own,
    /// something it holds (its items, its attributes' values)
    Held,
}

/// Where a value comes from: an origin, and whether it is that value or something it holds.
pub type Src = (Origin, Level);

/// A callee by the name the code calls it: `f(…)` (f, None), `m.f(…)` (m, Some(f)).
pub type Callee = (StrId, Option<StrId>);

/// Where an argument goes in a call: the i-th positional one, or a keyword.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Slot {
    Pos(usize),
    Kw(StrId),
}

/// A value given to a call of a name the function does not bind: what may happen to it is what
/// the callee's code does with that parameter, when the name holds the program's functions
/// (`effects.rs`), else anything. Only values of names (and calls' results) are put off so.
pub struct Pass {
    pub callee: Callee,
    pub slot: Slot,
    pub srcs: Vec<Src>,
    /// the names (and modules' attributes) whose values the call's arguments may be, when their
    /// text shows nothing else among them that may be code (`arg_names`); None: some may be
    pub args: Option<std::rc::Rc<Vec<Src>>>,
}

/// What may happen to an origin's value where the code does not run.
#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub struct Flags {
    /// it may change or be kept by code rowan does not follow (see the module comment)
    pub changes: bool,
    /// what it holds may
    pub held: bool,
    /// what its parts hold may (a part of it given to code that changes what that holds)
    pub held2: bool,
    /// it is iterated: what it holds is used, and an iterator advances
    pub iterated: bool,
    /// it is called (its code runs; a library method runs on its receiver)
    pub called: bool,
    /// a method is called on it (`v.m(…)`, not one known to change nothing): on an instance of
    /// the program's classes that runs their code, which changes it only by attribute names
    pub methods: bool,
    /// the code binds the name (a `global` / `nonlocal` assignment, `m.x = …` of a module)
    pub rebinds: bool,
    /// it is changed in place as a container (`v[k] = …`, `del v[k]`, `v.append(…)`, `v.pop()`):
    /// a dict, list or set changes its items, not what they are
    pub edits: bool,
    /// among those changes, its items may be removed (`del v[k]`, `v.pop(…)`, `v.clear()`): the
    /// program's own code, not code rowan cannot see, so its keys are not `likely` after
    pub removes: bool,
}

impl Flags {
    /// Given away: anything may happen to it.
    pub const CHANGE: Flags = Flags { changes: true, held: false, held2: false, iterated: false, called: false, methods: false, rebinds: false, edits: false, removes: false };

    /// What happens to a value, for its source at `level`: itself, or what holds it.
    pub fn lift(&self, level: Level) -> Flags {
        match level {
            Level::Own => Flags { rebinds: false, ..*self },
            Level::Held => Flags { held: self.changes || self.methods || self.edits || self.called || self.iterated, held2: self.held || self.held2, ..Flags::default() },
        }
    }

    pub fn join(&mut self, o: &Flags) {
        self.changes |= o.changes;
        self.held |= o.held;
        self.held2 |= o.held2;
        self.iterated |= o.iterated;
        self.called |= o.called;
        self.rebinds |= o.rebinds;
        self.methods |= o.methods;
        self.edits |= o.edits;
        self.removes |= o.removes;
    }
}

/// How an expression's value is used where it stands.
#[derive(Clone, Copy, PartialEq)]
enum Pos {
    /// read, compared, tested: nothing keeps it
    Read,
    /// bound to locals: it goes where they go (what is made here and not tracked is kept)
    Bind,
    /// given away or changed in place
    Change,
    Iterate,
    Call,
    /// a method is called on it
    Method,
    /// changed in place as a container
    Edit,
    /// changed in place as a container, items removed
    Remove,
}

/// A name the code reads that it does not bind, with an attribute read on it (`m.x`).
pub struct Use {
    pub name: StrId,
    pub attr: Option<StrId>,
    pub flags: Flags,
}

/// What a function's code (its nested functions, lambdas and comprehensions included) does
/// with values: the names it does not bind, the attributes of anything, its parameters; the
/// modules its own imports bind; the attribute names it reads, stores and deletes.
#[derive(Default)]
pub struct CodeUses {
    pub uses: Vec<Use>,
    /// per attribute name: what may happen to the value read under it (`e.a`)
    pub attrs: FxHashMap<StrId, Flags>,
    /// per parameter: what may happen to its value (a default's, for one)
    pub params: FxHashMap<StrId, Flags>,
    pub imports: FxHashMap<StrId, String>,
    /// the attribute names it assigns (`x.a = …`, `setattr(x, "a", …)`) and deletes
    pub stored: FxHashSet<StrId>,
    pub deleted: FxHashSet<StrId>,
    /// those it assigns or deletes on something other than a method's own instance (`self`):
    /// a class's attribute among others
    pub class_stores: FxHashSet<StrId>,
    /// (name, attribute) of an assignment to what a name it does not bind holds
    /// (`REGISTRY[k].a = …`): a class the name's value holds has its attribute rebound
    pub class_stores_in: Vec<(StrId, StrId)>,
    /// (callee, attribute) of an assignment to what a call by name returns (`make().a = …`,
    /// also through a local): a class the call may give back has its attribute rebound
    pub class_stores_result: Vec<(Callee, StrId)>,
    /// the attribute names it reads at all (a method of a class runs only when its name is
    /// read, or for a dunder)
    pub reads: FxHashSet<StrId>,
    /// how it uses its first parameter (a method's receiver), None when it has none
    pub recv: Option<RecvUse>,
    /// what may happen to the results of the calls it makes by name, where it uses them
    pub results: FxHashMap<Callee, Flags>,
    /// what its own `return`s give back of names and calls' results (what may happen to them
    /// is what the callers do with the result, `effects::caller_returns`)
    pub returns: Vec<Src>,
    /// a `return` may give back something else that may be a class: a parameter, an attribute,
    /// `type(x)` (not only names' values, calls' results and new values)
    pub returns_other: bool,
    /// names and calls' results it gives to calls by name
    pub passes: Vec<Pass>,
    /// the attribute names it calls on what is not a name it does not bind (`x.f(…)` of a
    /// local x, `a.b.f(…)`): what such a call returns is not followed (`Origin::Result`)
    pub untracked: FxHashSet<StrId>,
}

/// How a method's own code uses its receiver (its first parameter, and the locals bound to it):
/// the attribute names it reads, stores, deletes and calls on it, the methods it calls through
/// `super()`, or that the receiver escapes (passed on, compared, used by a computed name or a
/// dunder, captured by a nested function).
#[derive(Default)]
pub struct RecvUse {
    pub(in crate::interp) escapes: bool,
    /// the method returns its receiver (`return self`: a builder)
    pub(in crate::interp) returned: bool,
    pub(in crate::interp) loads: FxHashSet<StrId>,
    pub(in crate::interp) stores: FxHashSet<StrId>,
    pub(in crate::interp) deletes: FxHashSet<StrId>,
    pub(in crate::interp) calls: FxHashSet<StrId>,
    pub(in crate::interp) supers: FxHashSet<StrId>,
    /// attribute names built from a template (`f"{name}_dynamic"`) it reads, stores and
    /// deletes by `getattr` / `hasattr`, `setattr` and `delattr`
    pub(in crate::interp) loads_like: Vec<Like>,
    pub(in crate::interp) stores_like: Vec<Like>,
    pub(in crate::interp) deletes_like: Vec<Like>,
}

/// A name made by a template: its literal parts and its holes (a hole that is a name: the
/// name). Not knowing what the holes hold, it is any name that starts and ends as it does.
#[derive(Clone, PartialEq)]
pub struct Like(Vec<Result<String, Option<String>>>);

impl Like {
    /// Could the template make the name `k` (it starts and ends as the template's literal ends)?
    pub fn matches(&self, k: KeyId) -> bool {
        let start: String = self.0.iter().map_while(|p| p.as_ref().ok()).cloned().collect();
        let end: String = self.0.iter().rev().map_while(|p| p.as_ref().ok()).cloned().collect::<Vec<String>>().into_iter().rev().collect();
        key_as_str(k).is_some_and(|n| n.len() >= start.len() + end.len() && n.starts_with(&start) && n.ends_with(&end))
    }

    /// The one name it makes when each hole is a name `bind` knows the text of.
    pub fn resolve(&self, bind: &dyn Fn(&str) -> Option<String>) -> Option<String> {
        self.0.iter().map(|p| match p {
            Ok(l) => Some(l.clone()),
            Err(n) => bind(n.as_deref()?),
        }).collect()
    }
}

/// The template an f-string with holes is (`f"{a}_dynamic"`: "", "_dynamic").
fn template(e: &Expr) -> Option<Like> {
    let Expr::FString(f) = e else { return None };
    let mut parts: Vec<Result<String, Option<String>>> = Vec::new();
    for p in f.value.iter() {
        match p {
            ast::FStringPart::Literal(sl) => parts.push(Ok(sl.value.to_string())),
            ast::FStringPart::FString(fs) => {
                for el in fs.elements.iter() {
                    match el {
                        ast::InterpolatedStringElement::Literal(l) => parts.push(Ok(l.value.to_string())),
                        // (`{name}` without conversion or format: the name's text)
                        ast::InterpolatedStringElement::Interpolation(i) => parts.push(Err(match (&*i.expression, i.conversion, &i.format_spec) {
                            (Expr::Name(n), ast::ConversionFlag::None, None) => Some(n.id.to_string()),
                            _ => None,
                        })),
                    }
                }
            }
        }
    }
    parts.iter().any(|p| p.is_err()).then_some(Like(parts))
}

/// Methods that change nothing and keep nothing of what they are given, and builtins likewise
/// (their results may still be what they were given: `d.get(k, default)`).
fn pure_method(m: &str) -> bool {
    super::PURE_METHODS.contains(&m) || matches!(m, "__getitem__" | "__contains__" | "__len__" | "__eq__" | "__hash__" | "__str__" | "__repr__" | "__format__")
}

/// The methods by which a dict, list or set changes its items (`pop`, `popitem` and
/// `setdefault` give one back: `srcs`); on other objects, a method that may change them.
pub const CONTAINER_MUTATORS: &[&str] = &["append", "extend", "insert", "remove", "pop", "clear", "sort", "reverse", "update", "setdefault", "popitem", "add", "discard", "difference_update", "intersection_update", "symmetric_difference_update"];

/// Those of `CONTAINER_MUTATORS` that may remove items.
pub const CONTAINER_REMOVERS: &[&str] = &["remove", "pop", "clear", "popitem", "discard", "difference_update", "intersection_update", "symmetric_difference_update"];

/// Builtins that iterate their first argument and keep nothing of it but in their result.
const PASSING_BUILTINS: &[&str] = &["list", "tuple", "set", "frozenset", "dict", "sorted", "reversed", "enumerate", "zip", "min", "max", "sum", "any", "all", "iter"];

/// The scan (`code_uses`): `locals` are the function's own names (parameters included).
pub struct Flow<'a> {
    pub locals: &'a dyn Fn(&str) -> bool,
    /// the instance receiver's name, for a method run with one (not a static or class method)
    pub me: Option<&'a str>,
    pub pkg: String,
    /// per local name: where what it is bound to comes from
    alias: FxHashMap<StrId, Vec<Src>>,
    flags: FxHashMap<Origin, Flags>,
    /// names read that the function does not bind, in order
    names: Vec<(StrId, Option<StrId>)>,
    /// the first parameter (a method's receiver), what the code does with it, the locals bound
    /// once to a template, and how deep in nested functions the scan is
    first: Option<StrId>,
    recv: RecvUse,
    named: FxHashMap<StrId, Option<Like>>,
    depth: u32,
    pub out: CodeUses,
}

impl<'a> Flow<'a> {
    /// A scan of code whose own names `locals` picks, `me` its receiver's name, in package `pkg`.
    pub fn new(locals: &'a dyn Fn(&str) -> bool, me: Option<&'a str>, pkg: String) -> Self {
        Flow { locals, me, pkg, alias: FxHashMap::default(), flags: FxHashMap::default(), names: Vec::new(), first: None, recv: RecvUse::default(), named: FxHashMap::default(), depth: 0, out: CodeUses::default() }
    }

    /// Scan a function: its parameters, then its body (imports first: a use before the import
    /// statement binds the same name).
    pub fn scan(mut self, params: &[StrId], first: Option<StrId>, body: &'static [Stmt], lambda: Option<&'static Expr>) -> CodeUses {
        self.first = first;
        for st in crate::syntax::walk_stmts(body) {
            match st {
                Stmt::Import(i) => {
                    for a in &i.names {
                        let full = a.name.id.as_str();
                        let top = full.split('.').next().unwrap_or(full);
                        match &a.asname {
                            Some(x) => self.out.imports.insert(s(x.as_str()), full.to_string()),
                            None => self.out.imports.insert(s(top), top.to_string()),
                        };
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
                        self.out.imports.insert(s(local), format!("{}.{}", base.join("."), a.name.as_str()));
                    }
                }
                _ => {}
            }
        }
        for p in params {
            self.alias.insert(*p, vec![(Origin::Param(*p), Level::Own)]);
        }
        // what each local is bound to, to a fixpoint (a binding may name other locals)
        let mut b = Binds(&self, Vec::new());
        body.iter().for_each(|st| ast::visitor::Visitor::visit_stmt(&mut b, st));
        lambda.iter().for_each(|e| ast::visitor::Visitor::visit_expr(&mut b, e));
        let binds = b.1;
        // (a local bound once, to a template: that template)
        for (n, e, held) in &binds {
            let like = (!*held && !self.named.contains_key(n)).then(|| template(e)).flatten();
            self.named.insert(*n, like);
        }
        loop {
            let mut grew = false;
            for (n, e, held) in &binds {
                let mut srcs = self.srcs(e);
                if *held {
                    srcs = srcs.into_iter().map(|(o, _)| (o, Level::Held)).collect();
                }
                let cur = self.alias.entry(*n).or_default();
                for x in srcs {
                    if !cur.contains(&x) {
                        cur.push(x);
                        grew = true;
                    }
                }
            }
            if !grew {
                break;
            }
        }
        for st in body {
            self.stmt(st);
        }
        if let Some(e) = lambda {
            self.expr(e, Pos::Change);
        }
        // (the uses in order of their first appearance; a name something happens to through
        // what it is bound to, `REG.values()`, is one too)
        let mut seen: FxHashSet<(StrId, Option<StrId>)> = FxHashSet::default();
        let mut names = std::mem::take(&mut self.names);
        let mut more: Vec<(StrId, Option<StrId>)> = self.flags.keys().filter_map(|o| match o {
            Origin::Name(n) => Some((*n, None)),
            Origin::NameAttr(n, a) => Some((*n, Some(*a))),
            _ => None,
        }).collect();
        more.sort_unstable();
        names.extend(more);
        for (n, a) in names {
            if seen.insert((n, a)) {
                let o = match a {
                    Some(a) => Origin::NameAttr(n, a),
                    None => Origin::Name(n),
                };
                let flags = self.flags.get(&o).copied().unwrap_or_default();
                self.out.uses.push(Use { name: n, attr: a, flags });
            }
        }
        for (o, f) in &self.flags {
            match o {
                Origin::Param(p) => drop(self.out.params.insert(*p, *f)),
                Origin::Attr(a) => self.out.attrs.entry(*a).or_default().join(f),
                Origin::Result(n, a) => drop(self.out.results.insert((*n, *a), *f)),
                _ => {}
            }
        }
        if self.first.is_some() {
            self.out.recv = Some(std::mem::take(&mut self.recv));
        }
        self.out
    }

    /// A name the function binds (an import inside it is a name of the module it binds).
    fn local(&self, n: &str) -> bool {
        (self.locals)(n) && !self.out.imports.contains_key(&s(n))
    }

    // ------------------------------------------------------------- bindings
    /// The local names target `t` binds from `e` (`held`: from what `e` holds).
    fn bind_target(&self, t: &'static Expr, e: &'static Expr, held: bool, binds: &mut Vec<(StrId, &'static Expr, bool)>) {
        match t {
            Expr::Name(n) if self.local(n.id.as_str()) => binds.push((s(n.id.as_str()), e, held)),
            Expr::Starred(x) => self.bind_target(&x.value, e, true, binds),
            // (`a, b = x, y`: pairwise; from anything else: what it holds)
            Expr::Tuple(_) | Expr::List(_) => {
                let (ts, es) = match (t, e) {
                    (Expr::Tuple(a), Expr::Tuple(b)) => (&a.elts, Some(&b.elts)),
                    (Expr::List(a), Expr::List(b)) => (&a.elts, Some(&b.elts)),
                    (Expr::Tuple(a), _) => (&a.elts, None),
                    (Expr::List(a), _) => (&a.elts, None),
                    _ => unreachable!(),
                };
                match es.filter(|es| es.len() == ts.len() && !ts.iter().chain(es.iter()).any(|x| matches!(x, Expr::Starred(_)))) {
                    Some(es) => ts.iter().zip(es.iter()).for_each(|(t, e)| self.bind_target(t, e, held, binds)),
                    None => ts.iter().for_each(|t| self.bind_target(t, e, true, binds)),
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------- where values come from
    /// Where the value of `e` may come from (empty: a value made here, or unknown).
    fn srcs(&self, e: &Expr) -> Vec<Src> {
        let held = |v: Vec<Src>| v.into_iter().map(|(o, _)| (o, Level::Held)).collect::<Vec<Src>>();
        match e {
            Expr::Name(n) => {
                let n = n.id.as_str();
                if self.local(n) {
                    self.alias.get(&s(n)).cloned().unwrap_or_default()
                } else {
                    vec![(Origin::Name(s(n)), Level::Own)]
                }
            }
            Expr::Attribute(a) => match &*a.value {
                Expr::Name(n) if !self.local(n.id.as_str()) => vec![(Origin::NameAttr(s(n.id.as_str()), s(a.attr.as_str())), Level::Own)],
                v => {
                    let mut out = held(self.srcs(v));
                    out.push((Origin::Attr(s(a.attr.as_str())), Level::Own));
                    if a.attr.as_str() == "__class__" {
                        out.push((Origin::TypeOf, Level::Own));
                    }
                    out
                }
            },
            Expr::Subscript(x) => held(self.srcs(&x.value)),
            Expr::If(x) => [self.srcs(&x.body), self.srcs(&x.orelse)].concat(),
            Expr::BoolOp(b) => b.values.iter().flat_map(|v| self.srcs(v)).collect(),
            Expr::Named(n) => self.srcs(&n.value),
            Expr::BinOp(b) => [self.srcs(&b.left), self.srcs(&b.right)].concat(),
            Expr::Call(c) => {
                let args = || c.arguments.args.iter().chain(c.arguments.keywords.iter().map(|k| &k.value));
                match &*c.func {
                    // (`cast(T, x)`, `typing.cast(T, x)`: x)
                    Expr::Name(f) if f.id.as_str() == "cast" && !self.local("cast") && c.arguments.args.len() == 2 => self.srcs(&c.arguments.args[1]),
                    Expr::Attribute(a) if a.attr.as_str() == "cast" && matches!(&*a.value, Expr::Name(n) if n.id.as_str() == "typing") && c.arguments.args.len() == 2 => self.srcs(&c.arguments.args[1]),
                    // (a method that keeps nothing may give back what its receiver holds or an
                    // argument; a container's `pop` an item)
                    Expr::Attribute(a) if pure_method(a.attr.as_str()) || matches!(a.attr.as_str(), "pop" | "popitem" | "setdefault") => {
                        let mut out = held(self.srcs(&a.value));
                        out.extend(args().flat_map(|x| self.srcs(x)));
                        out
                    }
                    Expr::Name(f) if !self.local(f.id.as_str()) => match f.id.as_str() {
                        // (`getattr(x, "a", d)`: x.a or d; `vars(x)`: x's attributes)
                        "getattr" => {
                            let mut out = match (c.arguments.args.first(), c.arguments.args.get(1)) {
                                (Some(x), Some(Expr::StringLiteral(l))) => {
                                    let mut v = held(self.srcs(x));
                                    v.push((Origin::Attr(s(l.value.to_str())), Level::Own));
                                    v
                                }
                                (Some(x), _) => held(self.srcs(x)),
                                _ => Vec::new(),
                            };
                            out.extend(c.arguments.args.get(2).map(|d| self.srcs(d)).unwrap_or_default());
                            out
                        }
                        "vars" => c.arguments.args.first().map(|x| self.srcs(x)).unwrap_or_default(),
                        "type" if c.arguments.args.len() == 1 => vec![(Origin::TypeOf, Level::Own)],
                        // (`abs` / `round` may give back their argument, by its dunder; `sorted`,
                        // `min`, … what they were given; `len`, `str`, `isinstance`, … a new value)
                        // (a container made from an iterable — `dict(m)`, `list(xs)`, `sorted(xs)` — holds
                        // what it holds, as does an item of it — `min(xs)`; an iterator over it may be
                        // it — `iter(it)` — or advance it)
                        f if PASSING_BUILTINS.contains(&f) && !matches!(f, "iter" | "reversed" | "enumerate" | "zip") => held(args().flat_map(|x| self.srcs(x)).collect()),
                        f if PASSING_BUILTINS.contains(&f) || matches!(f, "abs" | "round") => args().flat_map(|x| self.srcs(x)).collect(),
                        f => vec![(Origin::Result(s(f), None), Level::Own)],
                    },
                    Expr::Attribute(a) => match &*a.value {
                        Expr::Name(n) if !self.local(n.id.as_str()) => vec![(Origin::Result(s(n.id.as_str()), Some(s(a.attr.as_str()))), Level::Own)],
                        _ => Vec::new(),
                    },
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    /// What a use at `pos` does to the origins of the values `srcs`.
    fn mark(&mut self, srcs: &[Src], pos: Pos) {
        for (o, l) in srcs {
            let f = self.flags.entry(*o).or_default();
            match (pos, l) {
                (Pos::Read | Pos::Bind, _) => {}
                (_, Level::Held) => f.held = true,
                (Pos::Change, Level::Own) => f.changes = true,
                (Pos::Iterate, Level::Own) => f.iterated = true,
                (Pos::Call, Level::Own) => f.called = true,
                (Pos::Method, Level::Own) => f.methods = true,
                (Pos::Edit, Level::Own) => f.edits = true,
                (Pos::Remove, Level::Own) => {
                    f.edits = true;
                    f.removes = true;
                }
            }
        }
    }

    /// A name read (each, also where nothing happens to it: the code it holds may run).
    fn name(&mut self, n: StrId, attr: Option<StrId>) {
        self.names.push((n, attr));
    }

    /// Whether `e` is a name that may hold the receiver (the first parameter, or a local bound
    /// to it); one inside a nested function or lambda lets it escape.
    fn recv_base(&mut self, e: &Expr) -> bool {
        let (Expr::Name(n), Some(first)) = (e, self.first) else { return false };
        let is = self.local(n.id.as_str()) && self.alias.get(&s(n.id.as_str())).is_some_and(|v| v.contains(&(Origin::Param(first), Level::Own)));
        self.recv.escapes |= is && self.depth > 0;
        is
    }

    /// A name for an attribute in a `getattr` / `setattr` / `delattr` call: a literal, or a
    /// template (in place, or a local bound once to one).
    fn attr_name(&self, e: Option<&Expr>) -> (Option<StrId>, Option<Like>) {
        match e {
            Some(Expr::StringLiteral(l)) => (Some(s(l.value.to_str())), None),
            Some(Expr::Name(n)) => (None, self.named.get(&s(n.id.as_str())).cloned().flatten()),
            Some(e) => (None, template(e)),
            None => (None, None),
        }
    }

    // ------------------------------------------------------------- uses
    /// The uses an expression makes, used itself at `pos`.
    fn expr(&mut self, e: &'static Expr, pos: Pos) {
        if !matches!(pos, Pos::Read | Pos::Bind) {
            let srcs = self.srcs(e);
            self.mark(&srcs, pos);
        }
        // (a container's parts are kept in it, unless only read: `x in (a, b)`; what an
        // expression passes on stands where it does)
        let part = if pos == Pos::Read { Pos::Read } else { Pos::Change };
        match e {
            Expr::Name(n) => {
                if !self.local(n.id.as_str()) {
                    self.name(s(n.id.as_str()), None);
                }
                // (the receiver itself, not bound to a local: it escapes)
                if pos != Pos::Bind && self.recv_base(e) {
                    self.recv.escapes = true;
                }
            }
            Expr::Attribute(a) => {
                self.out.reads.insert(s(a.attr.as_str()));
                match &*a.value {
                    Expr::Name(n) if !self.local(n.id.as_str()) => self.name(s(n.id.as_str()), Some(s(a.attr.as_str()))),
                    v if self.recv_base(v) => {
                        self.recv.loads.insert(s(a.attr.as_str()));
                        self.recv.escapes |= a.attr.as_str() == "__dict__";
                    }
                    v => self.expr(v, Pos::Read),
                }
            }
            Expr::Subscript(x) => {
                self.expr(&x.value, Pos::Read);
                self.expr(&x.slice, Pos::Read);
            }
            Expr::Call(c) => self.call(c),
            Expr::If(x) => {
                self.expr(&x.test, Pos::Read);
                self.expr(&x.body, pos);
                self.expr(&x.orelse, pos);
            }
            Expr::BoolOp(b) => b.values.iter().for_each(|v| self.expr(v, pos)),
            Expr::Named(n) => self.expr(&n.value, pos),
            Expr::BinOp(b) => {
                self.expr(&b.left, pos);
                self.expr(&b.right, pos);
            }
            Expr::Compare(c) => {
                self.expr(&c.left, Pos::Read);
                c.comparators.iter().for_each(|x| self.expr(x, Pos::Read));
            }
            Expr::UnaryOp(u) => self.expr(&u.operand, Pos::Read),
            Expr::List(x) => x.elts.iter().for_each(|v| self.expr(v, part)),
            Expr::Tuple(x) => x.elts.iter().for_each(|v| self.expr(v, part)),
            Expr::Set(x) => x.elts.iter().for_each(|v| self.expr(v, part)),
            Expr::Dict(d) => {
                for it in &d.items {
                    match &it.key {
                        Some(k) => {
                            self.expr(k, part);
                            self.expr(&it.value, part);
                        }
                        // (`{**m}`: m's values are kept, not m)
                        None => self.unpacked(&it.value, part),
                    }
                }
            }
            // (`[*xs]`, `f(*xs)`: xs's items are kept or given, not xs)
            Expr::Starred(x) => self.unpacked(&x.value, if pos == Pos::Read { Pos::Read } else { Pos::Change }),
            Expr::ListComp(ast::ExprListComp { elt, generators, .. }) | Expr::SetComp(ast::ExprSetComp { elt, generators, .. }) | Expr::Generator(ast::ExprGenerator { elt, generators, .. }) => {
                self.comprehension(generators);
                self.expr(elt, part);
            }
            Expr::DictComp(d) => {
                self.comprehension(&d.generators);
                self.expr(&d.key, part);
                self.expr(&d.value, part);
            }
            // (given to the code that consumes, awaits or later calls it)
            Expr::Yield(y) => {
                if let Some(v) = &y.value {
                    self.expr(v, Pos::Change);
                }
            }
            Expr::YieldFrom(y) => self.expr(&y.value, Pos::Change),
            Expr::Await(a) => self.expr(&a.value, Pos::Change),
            Expr::Lambda(l) => {
                if let Some(p) = &l.parameters {
                    for d in p.iter_non_variadic_params().filter_map(|p| p.default.as_deref()) {
                        self.expr(d, Pos::Change);
                    }
                }
                self.depth += 1;
                self.expr(&l.body, Pos::Change);
                self.depth -= 1;
            }
            _ => {
                struct Sub<'s, 'b>(&'s mut Flow<'b>);
                impl<'s, 'b> ast::visitor::Visitor<'static> for Sub<'s, 'b> {
                    fn visit_expr(&mut self, e: &'static Expr) {
                        self.0.expr(e, Pos::Read);
                    }
                }
                ast::visitor::walk_expr(&mut Sub(self), e);
            }
        }
    }

    /// `e` unpacked (`*e`, `**e`) where its parts go to `pos`: what it holds goes there, and it
    /// is iterated.
    fn unpacked(&mut self, e: &'static Expr, pos: Pos) {
        if pos != Pos::Read {
            let srcs: Vec<Src> = self.srcs(e).into_iter().map(|(o, _)| (o, Level::Held)).collect();
            self.mark(&srcs, pos);
            self.expr(e, Pos::Iterate);
        } else {
            self.expr(e, Pos::Read);
        }
    }

    fn comprehension(&mut self, generators: &'static [ast::Comprehension]) {
        for g in generators {
            self.expr(&g.iter, if self.all_local(&g.target) { Pos::Iterate } else { Pos::Change });
            self.target(&g.target);
            g.ifs.iter().for_each(|x| self.expr(x, Pos::Read));
        }
    }

    /// `e` given to code whose effects are put off (a callee by name, the callers of a return):
    /// the values of names and calls' results it is go to `out`; what else it is may change,
    /// and its parts are kept.
    fn give(&mut self, e: &'static Expr, out: &mut Vec<Src>) {
        let srcs = self.srcs(e);
        self.expr(e, Pos::Bind);
        let (later, now): (Vec<Src>, Vec<Src>) = srcs.into_iter().partition(|(o, _)| matches!(o, Origin::Name(_) | Origin::NameAttr(..) | Origin::Result(..)));
        self.mark(&now, Pos::Change);
        // (a name's value put off is a use of the name: `REG.get(k)` is one of `REG` too)
        for (o, _) in &later {
            match *o {
                Origin::Name(n) => self.name(n, None),
                Origin::NameAttr(n, a) => self.name(n, Some(a)),
                _ => {}
            }
        }
        // (the receiver itself: it escapes)
        self.recv.escapes |= self.first.is_some_and(|f| now.contains(&(Origin::Param(f), Level::Own)));
        out.extend(later);
    }

    /// The arguments of a call of a name the function does not bind (after a `*xs`, or in a
    /// `**kw`: given away).
    fn pass_args(&mut self, callee: Callee, c: &'static ast::ExprCall) {
        let star = c.arguments.args.iter().position(|x| matches!(x, Expr::Starred(_))).unwrap_or(usize::MAX);
        let pos = c.arguments.args.iter().enumerate().map(|(i, x)| (x, (i < star).then_some(Slot::Pos(i))));
        let kw = c.arguments.keywords.iter().map(|k| (&k.value, k.arg.as_ref().map(|a| Slot::Kw(s(a.as_str())))));
        let all: Vec<(&'static Expr, Option<Slot>)> = pos.chain(kw).collect();
        let mut names = Vec::new();
        let args = all.iter().all(|(x, slot)| slot.is_some() && self.arg_names(x, &mut names)).then(|| std::rc::Rc::new(names));
        for (x, slot) in all {
            let Some(slot) = slot else {
                // (`**kw`: the callee gets a new dict of kw's values)
                if c.arguments.keywords.iter().any(|k| k.arg.is_none() && std::ptr::eq(&k.value, x)) {
                    self.unpacked(x, Pos::Change);
                } else {
                    self.expr(x, Pos::Change);
                }
                continue;
            };
            let mut srcs = Vec::new();
            self.give(x, &mut srcs);
            if !srcs.is_empty() {
                self.out.passes.push(Pass { callee, slot, srcs, args: args.clone() });
            }
        }
    }

    /// Whether argument `e` can be code only as the value of a name (or of a module's
    /// attribute) its text names, which go to `out`: a constant, a display, an operation, a
    /// pure builtin's result are data. A local, a parameter, an attribute of anything else, a
    /// call's result, a lambda may be code: false.
    fn arg_names(&self, e: &Expr, out: &mut Vec<Src>) -> bool {
        match e {
            Expr::StringLiteral(_) | Expr::BytesLiteral(_) | Expr::NumberLiteral(_) | Expr::BooleanLiteral(_) | Expr::NoneLiteral(_) | Expr::EllipsisLiteral(_) | Expr::FString(_) => true,
            Expr::List(_) | Expr::Tuple(_) | Expr::Set(_) | Expr::Dict(_) | Expr::BinOp(_) | Expr::UnaryOp(_) | Expr::Compare(_) => true,
            Expr::BoolOp(b) => b.values.iter().all(|v| self.arg_names(v, out)),
            Expr::If(x) => self.arg_names(&x.body, out) && self.arg_names(&x.orelse, out),
            Expr::Name(n) if !self.local(n.id.as_str()) => {
                out.push((Origin::Name(s(n.id.as_str())), Level::Own));
                true
            }
            Expr::Attribute(a) => match &*a.value {
                Expr::Name(n) if !self.local(n.id.as_str()) => {
                    out.push((Origin::NameAttr(s(n.id.as_str()), s(a.attr.as_str())), Level::Own));
                    true
                }
                _ => false,
            },
            Expr::Call(c) => matches!(&*c.func, Expr::Name(f) if super::PURE_BUILTINS.contains(&f.id.as_str()) && !self.local(f.id.as_str())),
            _ => false,
        }
    }

    /// `m.a.f` of a name `m` the function does not bind: (m, "a.f").
    fn dotted_callee(&self, e: &Expr) -> Option<(StrId, String)> {
        match e {
            Expr::Attribute(a) => match &*a.value {
                Expr::Name(n) if !self.local(n.id.as_str()) => Some((s(n.id.as_str()), a.attr.to_string())),
                v => self.dotted_callee(v).map(|(n, path)| (n, format!("{}.{}", path, a.attr.as_str()))),
            },
            _ => None,
        }
    }

    /// A call's uses: its callee by name, its arguments given to it (to a method of a value, a
    /// library function, the program's function by name: `Pass`).
    fn call(&mut self, c: &'static ast::ExprCall) {
        let args: Vec<&'static Expr> = c.arguments.args.iter().chain(c.arguments.keywords.iter().map(|k| &k.value)).collect();
        match &*c.func {
            Expr::Attribute(a) => {
                let m = a.attr.as_str();
                self.out.reads.insert(s(m));
                let pure = pure_method(m);
                // (the receiver: a method that may change it; `n.f(…)` of a name: n's method, or
                // a module's function)
                match &*a.value {
                    Expr::Name(n) if !self.local(n.id.as_str()) => {
                        self.name(s(n.id.as_str()), Some(s(m)));
                        if !pure {
                            self.mark(&[(Origin::NameAttr(s(n.id.as_str()), s(m)), Level::Own)], Pos::Call);
                            return self.pass_args((s(n.id.as_str()), Some(s(m))), c);
                        }
                    }
                    v if self.recv_base(v) => drop(self.recv.calls.insert(s(m))),
                    // (`super().m(…)`, `super(C, self).m(…)`: a method of a base on the receiver)
                    Expr::Call(sc) if matches!(&*sc.func, Expr::Name(f) if f.id.as_str() == "super") && (sc.arguments.args.is_empty() || sc.arguments.args.get(1).is_some_and(|x| self.recv_base(x))) => {
                        self.recv.supers.insert(s(m));
                        sc.arguments.args.iter().take(1).for_each(|x| self.expr(x, Pos::Read));
                    }
                    v => {
                        let recv = self.srcs(v);
                        self.mark(&recv, if pure { Pos::Read } else if CONTAINER_REMOVERS.contains(&m) { Pos::Remove } else if CONTAINER_MUTATORS.contains(&m) { Pos::Edit } else { Pos::Method });
                        self.expr(v, Pos::Read);
                        // (a module's function called through a local, say: its result is not followed)
                        self.out.untracked.insert(s(m));
                        // (`os.path.join(…)` of a name: what it does with its arguments, by the callee's name)
                        if let Some((n, path)) = self.dotted_callee(&c.func).filter(|_| !pure) {
                            return self.pass_args((n, Some(s(&path))), c);
                        }
                    }
                }
                args.iter().for_each(|x| self.expr(x, if pure { Pos::Read } else { Pos::Change }));
            }
            Expr::Name(f) if !self.local(f.id.as_str()) => {
                let f = f.id.as_str();
                self.name(s(f), None);
                self.mark(&[(Origin::Name(s(f)), Level::Own)], Pos::Call);
                // (a builtin that only tells what the receiver is)
                if matches!(f, "isinstance" | "issubclass" | "id" | "callable" | "type") && c.arguments.keywords.is_empty() && c.arguments.args.first().is_some_and(|x| self.recv_base(x)) {
                    if f == "type" {
                        self.recv.loads.insert(s("__class__"));
                    }
                    c.arguments.args.iter().skip(1).for_each(|x| self.expr(x, Pos::Read));
                    return;
                }
                match f {
                    "setattr" | "delattr" | "getattr" | "hasattr" => self.attr_builtin(f, c),
                    // (`locals()` / `vars()`: every local's value)
                    "locals" | "vars" if c.arguments.args.is_empty() => {
                        let all: Vec<Src> = self.alias.values().flatten().copied().collect();
                        self.mark(&all, Pos::Change);
                    }
                    _ if super::PURE_BUILTINS.contains(&f) || f == "vars" => args.iter().for_each(|x| self.expr(x, Pos::Read)),
                    _ if PASSING_BUILTINS.contains(&f) => {
                        for (i, x) in c.arguments.args.iter().enumerate() {
                            self.expr(x, if i == 0 && !matches!(x, Expr::Starred(_)) { Pos::Iterate } else { Pos::Change });
                        }
                        c.arguments.keywords.iter().for_each(|k| self.expr(&k.value, Pos::Change));
                    }
                    _ => self.pass_args((s(f), None), c),
                }
            }
            // (the receiver called: its `__call__` runs, a torch module's `forward`)
            f if self.recv_base(f) => {
                self.recv.calls.extend([s("__call__"), s("forward")]);
                args.iter().for_each(|x| self.expr(x, Pos::Change));
            }
            f => {
                self.expr(f, Pos::Call);
                args.iter().for_each(|x| self.expr(x, Pos::Change));
            }
        }
    }

    /// `setattr` / `delattr` / `getattr` / `hasattr` by a name in the text: that attribute (by a
    /// computed name: not followed).
    fn attr_builtin(&mut self, f: &str, c: &'static ast::ExprCall) {
        let args = &c.arguments.args;
        let (lit, like) = self.attr_name(args.get(1));
        if let Some(n) = lit {
            self.out.reads.insert(n);
        }
        // (on the receiver: that name, or any name the template makes; else it escapes)
        if args.first().is_some_and(|x| self.recv_base(x)) {
            let r = &mut self.recv;
            match (f, lit, like) {
                ("getattr" | "hasattr", Some(n), _) => drop(r.loads.insert(n)),
                ("setattr", Some(n), _) => drop(r.stores.insert(n)),
                ("delattr", Some(n), _) => drop(r.deletes.insert(n)),
                ("getattr" | "hasattr", _, Some(l)) => r.loads_like.push(l),
                ("setattr", _, Some(l)) => r.stores_like.push(l),
                ("delattr", _, Some(l)) => r.deletes_like.push(l),
                _ => r.escapes = true,
            }
        }
        match f {
            "setattr" | "delattr" => {
                if let Some(x) = args.first() {
                    let srcs = self.srcs(x);
                    self.mark(&srcs, Pos::Change);
                    if let Some(n) = lit {
                        if f == "setattr" { &mut self.out.stored } else { &mut self.out.deleted }.insert(n);
                        self.class_store(x, n);
                    }
                    if !self.recv_base(x) {
                        self.expr(x, Pos::Read);
                    }
                }
                args.iter().skip(1).for_each(|x| self.expr(x, Pos::Change));
            }
            _ => {
                for (i, x) in args.iter().enumerate() {
                    if i > 0 || !self.recv_base(x) {
                        self.expr(x, Pos::Read);
                    }
                }
            }
        }
        c.arguments.keywords.iter().for_each(|k| self.expr(&k.value, Pos::Change));
    }

    /// An attribute `a` assigned or deleted on `base`, which may be a class: a name holding one
    /// (`K.x = …`, also through a local), `type(e)` / `e.__class__`, a parameter (a class
    /// method's `cls`, a class given to a function) — not the method's own instance.
    fn class_store(&mut self, base: &Expr, a: StrId) {
        if self.is_me(base) {
            return;
        }
        for (o, l) in self.srcs(base) {
            match (o, l) {
                (Origin::Name(n), Level::Own) => {
                    self.name(n, Some(a));
                    self.flags.entry(Origin::NameAttr(n, a)).or_default().rebinds = true;
                }
                (Origin::Name(n), Level::Held) => {
                    self.name(n, None);
                    self.out.class_stores_in.push((n, a));
                }
                // (`make().a = …`: a class the call may give back)
                (Origin::Result(n, f), Level::Own) => self.out.class_stores_result.push(((n, f), a)),
                (Origin::Param(_) | Origin::TypeOf, Level::Own) => drop(self.out.class_stores.insert(a)),
                _ => {}
            }
        }
    }

    /// The method's own instance (`self`), when the code never binds the name to anything else.
    fn is_me(&self, e: &Expr) -> bool {
        matches!((e, self.me), (Expr::Name(n), Some(me)) if n.id.as_str() == me && self.alias.get(&s(me)).is_some_and(|v| v.len() == 1))
    }

    /// Does target `t` bind local names only?
    fn all_local(&self, t: &Expr) -> bool {
        match t {
            Expr::Name(n) => self.local(n.id.as_str()),
            Expr::Tuple(x) => x.elts.iter().all(|y| self.all_local(y)),
            Expr::List(x) => x.elts.iter().all(|y| self.all_local(y)),
            Expr::Starred(x) => self.all_local(&x.value),
            _ => false,
        }
    }

    /// An assignment / deletion / loop target (`del`: `deleting`).
    fn target_as(&mut self, t: &'static Expr, deleting: bool) {
        match t {
            Expr::Name(n) => {
                if !self.local(n.id.as_str()) {
                    self.name(s(n.id.as_str()), None);
                    self.flags.entry(Origin::Name(s(n.id.as_str()))).or_default().rebinds = true;
                }
                self.recv.escapes |= self.first == Some(s(n.id.as_str()));
            }
            Expr::Attribute(a) => {
                let n = s(a.attr.as_str());
                self.out.reads.insert(n);
                if deleting { &mut self.out.deleted } else { &mut self.out.stored }.insert(n);
                self.class_store(&a.value, n);
                // (the object changes; `m.x = …` of a name: a module's global rebound, or the
                // object m changes)
                match &*a.value {
                    Expr::Name(m) if !self.local(m.id.as_str()) => {
                        self.name(s(m.id.as_str()), Some(n));
                        self.flags.entry(Origin::NameAttr(s(m.id.as_str()), n)).or_default().rebinds = true;
                    }
                    v if self.recv_base(v) => drop(if deleting { &mut self.recv.deletes } else { &mut self.recv.stores }.insert(n)),
                    v => {
                        let base = self.srcs(v);
                        self.mark(&base, Pos::Change);
                        self.expr(v, Pos::Read);
                    }
                }
            }
            Expr::Subscript(x) => {
                let base = self.srcs(&x.value);
                self.mark(&base, if deleting { Pos::Remove } else { Pos::Edit });
                self.expr(&x.value, Pos::Read);
                self.expr(&x.slice, Pos::Read);
            }
            Expr::Tuple(x) => x.elts.iter().for_each(|y| self.target_as(y, deleting)),
            Expr::List(x) => x.elts.iter().for_each(|y| self.target_as(y, deleting)),
            Expr::Starred(x) => self.target_as(&x.value, deleting),
            e => self.expr(e, Pos::Change),
        }
    }

    fn target(&mut self, t: &'static Expr) {
        self.target_as(t, false)
    }

    fn body(&mut self, b: &'static [Stmt]) {
        b.iter().for_each(|st| self.stmt(st));
    }

    /// A statement's uses.
    fn stmt(&mut self, st: &'static Stmt) {
        match st {
            Stmt::Expr(x) => self.expr(&x.value, Pos::Read),
            Stmt::Assign(a) => {
                // (bound to locals only: where it goes is where they go)
                let local = a.targets.iter().all(|t| self.all_local(t));
                self.expr(&a.value, if local { Pos::Bind } else { Pos::Change });
                a.targets.iter().for_each(|t| self.target(t));
            }
            Stmt::AnnAssign(a) => {
                if let Some(v) = &a.value {
                    self.expr(v, if self.all_local(&a.target) { Pos::Bind } else { Pos::Change });
                    self.target(&a.target);
                }
            }
            Stmt::AugAssign(a) => {
                // (an in-place operation: the target's value changes, and holds the value's parts)
                self.expr(&a.value, Pos::Change);
                self.expr(&a.target, Pos::Change);
                self.target(&a.target);
            }
            Stmt::Delete(d) => d.targets.iter().for_each(|t| self.target_as(t, true)),
            Stmt::For(f) => {
                self.expr(&f.iter, if self.all_local(&f.target) { Pos::Iterate } else { Pos::Change });
                self.target(&f.target);
                self.body(&f.body);
                self.body(&f.orelse);
            }
            Stmt::While(w) => {
                self.expr(&w.test, Pos::Read);
                self.body(&w.body);
                self.body(&w.orelse);
            }
            Stmt::If(i) => {
                self.expr(&i.test, Pos::Read);
                self.body(&i.body);
                for c in &i.elif_else_clauses {
                    if let Some(t) = &c.test {
                        self.expr(t, Pos::Read);
                    }
                    self.body(&c.body);
                }
            }
            Stmt::Assert(a) => {
                self.expr(&a.test, Pos::Read);
                if let Some(m) = &a.msg {
                    self.expr(m, Pos::Change);
                }
            }
            Stmt::With(w) => {
                // (its `__enter__` / `__exit__` run on it)
                for it in &w.items {
                    self.expr(&it.context_expr, Pos::Change);
                    if let Some(v) = &it.optional_vars {
                        self.target(v);
                    }
                }
                self.body(&w.body);
            }
            Stmt::Return(r) => {
                // (`return self`: the caller gets the receiver back, no other code changes it)
                match &r.value {
                    Some(v) if self.depth == 0 && self.recv_base(v) => {
                        self.recv.returned = true;
                        self.out.returns_other = true;
                        let first = self.first.map(|f| (Origin::Param(f), Level::Own));
                        let others: Vec<Src> = self.srcs(v).into_iter().filter(|x| Some(*x) != first).collect();
                        self.mark(&others, Pos::Change);
                    }
                    // (names' values and calls' results go to the callers)
                    Some(v) if self.depth == 0 => {
                        self.out.returns_other |= self.srcs(v).iter().any(|(o, _)| !matches!(o, Origin::Name(_) | Origin::NameAttr(..) | Origin::Result(..)));
                        let mut srcs = Vec::new();
                        self.give(v, &mut srcs);
                        self.out.returns.extend(srcs);
                    }
                    Some(v) => self.expr(v, Pos::Change),
                    None => {}
                }
            }
            Stmt::Raise(r) => {
                for x in r.exc.iter().chain(r.cause.iter()) {
                    self.expr(x, Pos::Change);
                }
            }
            _ => self.compound_stmt(st),
        }
    }

    /// The statements that hold others or define code (`stmt`).
    fn compound_stmt(&mut self, st: &'static Stmt) {
        match st {
            Stmt::Try(t) => {
                self.body(&t.body);
                for h in &t.handlers {
                    let ast::ExceptHandler::ExceptHandler(h) = h;
                    if let Some(x) = &h.type_ {
                        self.expr(x, Pos::Read);
                    }
                    self.body(&h.body);
                }
                self.body(&t.orelse);
                self.body(&t.finalbody);
            }
            Stmt::FunctionDef(f) => {
                f.decorator_list.iter().for_each(|d| self.expr(&d.expression, Pos::Change));
                for d in f.parameters.iter_non_variadic_params().filter_map(|p| p.default.as_deref()) {
                    self.expr(d, Pos::Change);
                }
                self.depth += 1;
                self.body(&f.body);
                self.depth -= 1;
            }
            Stmt::ClassDef(c) => {
                c.decorator_list.iter().for_each(|d| self.expr(&d.expression, Pos::Change));
                if let Some(a) = &c.arguments {
                    a.args.iter().for_each(|x| self.expr(x, Pos::Change));
                    a.keywords.iter().for_each(|k| self.expr(&k.value, Pos::Change));
                }
                self.depth += 1;
                self.body(&c.body);
                self.depth -= 1;
            }
            // (what a pattern binds is part of the subject)
            Stmt::Match(m) => {
                self.expr(&m.subject, Pos::Change);
                for c in &m.cases {
                    if let Some(g) = &c.guard {
                        self.expr(g, Pos::Read);
                    }
                    self.body(&c.body);
                }
            }
            Stmt::Import(_) | Stmt::ImportFrom(_) | Stmt::Global(_) | Stmt::Nonlocal(_) | Stmt::Pass(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
            _ => {
                struct Sub<'s, 'b>(&'s mut Flow<'b>);
                impl<'s, 'b> ast::visitor::Visitor<'static> for Sub<'s, 'b> {
                    fn visit_expr(&mut self, e: &'static Expr) {
                        self.0.expr(e, Pos::Change);
                    }
                    fn visit_stmt(&mut self, st: &'static Stmt) {
                        self.0.stmt(st);
                    }
                }
                ast::visitor::walk_stmt(&mut Sub(self), st);
            }
        }
    }
}

/// The first pass of `Flow::scan`: what each local is bound to (`held`: what it holds).
struct Binds<'s, 'a>(&'s Flow<'a>, Vec<(StrId, &'static Expr, bool)>);

impl ast::visitor::Visitor<'static> for Binds<'_, '_> {
    fn visit_stmt(&mut self, st: &'static Stmt) {
        match st {
            Stmt::Assign(a) => a.targets.iter().for_each(|t| self.0.bind_target(t, &a.value, false, &mut self.1)),
            Stmt::AnnAssign(ast::StmtAnnAssign { target, value: Some(v), .. }) => self.0.bind_target(target, v, false, &mut self.1),
            // (`x += e`: x may hold what e holds)
            Stmt::AugAssign(a) => self.0.bind_target(&a.target, &a.value, true, &mut self.1),
            // (`for c in [A, B]`: c is one of the items)
            Stmt::For(f) => match &*f.iter {
                Expr::List(ast::ExprList { elts, .. }) | Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::Set(ast::ExprSet { elts, .. }) if !elts.is_empty() && !elts.iter().any(|e| matches!(e, Expr::Starred(_))) => {
                    elts.iter().for_each(|e| self.0.bind_target(&f.target, e, false, &mut self.1));
                }
                it => self.0.bind_target(&f.target, it, true, &mut self.1),
            },
            Stmt::With(w) => {
                for it in &w.items {
                    if let Some(v) = &it.optional_vars {
                        self.0.bind_target(v, &it.context_expr, false, &mut self.1);
                    }
                }
            }
            _ => {}
        }
        ast::visitor::walk_stmt(self, st);
    }

    fn visit_expr(&mut self, e: &'static Expr) {
        match e {
            Expr::Named(n) => self.0.bind_target(&n.target, &n.value, false, &mut self.1),
            Expr::ListComp(ast::ExprListComp { generators, .. }) | Expr::SetComp(ast::ExprSetComp { generators, .. }) | Expr::DictComp(ast::ExprDictComp { generators, .. }) | Expr::Generator(ast::ExprGenerator { generators, .. }) => {
                generators.iter().for_each(|g| self.0.bind_target(&g.target, &g.iter, true, &mut self.1));
            }
            _ => {}
        }
        ast::visitor::walk_expr(self, e);
    }
}

/// The names some code calls (`f(…)`, `m.f(…)`: f), and those it uses otherwise, as values that
/// any code may call later (`g(f)`, `x = f`, `m.f` read, `getattr(m, "f")`, a `def`'s name with
/// several decorators; an import's alias: the name it binds too); those of `def`s with one
/// decorator (what calls them is the decorator's).
#[derive(Default)]
pub struct NameUses {
    pub called: FxHashSet<StrId>,
    pub values: FxHashSet<StrId>,
    pub decorated: FxHashSet<StrId>,
    aliases: Vec<(StrId, StrId)>,
}

impl NameUses {
    /// Adds the names these statements call and use.
    pub fn scan(&mut self, body: &'static [Stmt]) {
        body.iter().for_each(|st| ast::visitor::Visitor::visit_stmt(self, st));
    }

    /// The names used as values, through the imports' aliases too.
    pub fn close(&mut self) {
        while let Some(&(_, n)) = self.aliases.iter().find(|(a, n)| self.values.contains(a) && !self.values.contains(n)) {
            self.values.insert(n);
        }
    }
}

impl ast::visitor::Visitor<'static> for NameUses {
    fn visit_stmt(&mut self, st: &'static Stmt) {
        match st {
            Stmt::FunctionDef(f) if !f.decorator_list.is_empty() => drop(if f.decorator_list.len() == 1 { &mut self.decorated } else { &mut self.values }.insert(s(f.name.as_str()))),
            Stmt::ImportFrom(i) => self.aliases.extend(i.names.iter().filter_map(|a| Some((s(a.asname.as_ref()?.as_str()), s(a.name.as_str()))))),
            // (an import in a class body binds class attributes: methods' `self.f(…)`)
            Stmt::ClassDef(c) => {
                for st in c.body.iter() {
                    if let Stmt::ImportFrom(i) = st {
                        self.values.extend(i.names.iter().map(|a| s(a.name.as_str())));
                    }
                }
            }
            _ => {}
        }
        ast::visitor::walk_stmt(self, st);
    }

    fn visit_expr(&mut self, e: &'static Expr) {
        match e {
            Expr::Call(c) => {
                match &*c.func {
                    Expr::Name(n) => drop(self.called.insert(s(n.id.as_str()))),
                    Expr::Attribute(a) => {
                        self.called.insert(s(a.attr.as_str()));
                        self.visit_expr(&a.value);
                    }
                    f => self.visit_expr(f),
                }
                if let (Expr::Name(f), Some(Expr::StringLiteral(l))) = (&*c.func, c.arguments.args.get(1)) {
                    if f.id.as_str() == "getattr" {
                        self.values.insert(s(l.value.to_str()));
                    }
                }
                c.arguments.args.iter().for_each(|x| self.visit_expr(x));
                c.arguments.keywords.iter().for_each(|k| self.visit_expr(&k.value));
                return;
            }
            Expr::Name(n) if n.ctx.is_load() => drop(self.values.insert(s(n.id.as_str()))),
            Expr::Attribute(a) if a.ctx.is_load() => drop(self.values.insert(s(a.attr.as_str()))),
            _ => {}
        }
        ast::visitor::walk_expr(self, e);
    }
}
