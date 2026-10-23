//! Lexical scope analysis, from a module's text before it runs: which names of each function
//! (and lambda, class body, module) are locals, cells (captured by nested functions), free
//! (a nested function's names of an enclosing one) or globals, as CPython's compiler decides.
//! The interpreter looks a name up in the scope of the running frame (`Frame::scope`).

use rustc_hash::{FxHashMap, FxHashSet};
use ruff_python_ast as ast;
use ruff_python_ast::visitor::source_order::{self, SourceOrderVisitor};

/// What code a scope belongs to (a lambda's is a function's).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ScopeKind {
    #[default]
    Module,
    Function,
    Class,
}

/// The names of one scope (by `Scopes` index).
#[derive(Debug, Default)]
pub struct Scope {
    pub kind: ScopeKind,
    /// the enclosing scope (None for the module and detached scopes)
    pub parent: Option<usize>,
    /// names the code binds (assignments, parameters, imports, `def`, `class`, `except … as`,
    /// patterns, walrus targets)
    pub locals: FxHashSet<String>,
    /// names declared `global`
    pub globals_decl: FxHashSet<String>,
    /// names declared `nonlocal`
    pub nonlocals_decl: FxHashSet<String>,
    /// names the code reads
    pub uses: FxHashSet<String>,
    /// the scopes nested in it
    pub children: Vec<usize>,
    /// names of an enclosing function the code (or a function nested in it) reads
    pub free: FxHashSet<String>,
    /// locals a nested function reads (they live in cells)
    pub cells: FxHashSet<String>,
    /// binding occurrences per name (one inside a loop counts twice)
    pub binds: FxHashMap<String, u32>,
    /// a generator expression's (its first iterable is read in the enclosing scope; a walrus
    /// in it binds in the nearest enclosing scope that is not one)
    pub genexp: bool,
    /// cells a nested function rebinds (`nonlocal`)
    pub rebound_cells: FxHashSet<String>,
}

impl Scope {
    /// Is `name` a local of this scope (bound here, not declared global or nonlocal)?
    pub fn is_local(&self, name: &str) -> bool {
        self.locals.contains(name) && !self.globals_decl.contains(name) && !self.nonlocals_decl.contains(name)
    }
}

/// Scopes of one module, keyed by the address of the defining node.
#[derive(Debug, Default)]
pub struct Scopes {
    pub scopes: Vec<Scope>,
    pub by_node: FxHashMap<usize, usize>,
    /// generator expressions that run where they are written (`syntax::eager_genexps`), by
    /// address
    pub eager_genexps: FxHashSet<usize>,
    /// generator expressions a name is bound to that the next statement gives whole to the
    /// function called by this name (`syntax::consumed_next_genexps`): they run where written
    /// if it takes every item at once there
    pub consumed_next: FxHashMap<usize, String>,
    /// module names bound more than once (twice in the text, in a loop, or by a function
    /// through `global`): their value at a call depends on when the call happens
    pub rebound: FxHashSet<String>,
}

impl Scopes {
    /// The module's own scope.
    pub fn module(&self) -> usize {
        0
    }
    /// The scope a `def`, lambda or class statement (by its address) opens.
    pub fn of_node(&self, addr: usize) -> Option<usize> {
        self.by_node.get(&addr).copied()
    }
    pub fn get(&self, id: usize) -> &Scope {
        &self.scopes[id]
    }
}

/// A node's address: its identity (trees are leaked, `frontend.rs`).
pub fn node_addr<T>(n: &T) -> usize {
    n as *const T as usize
}

/// The first pass: walks a module, opening a scope per function, lambda and class, and
/// records what each binds, declares and reads.
struct Collector<'a> {
    scopes: &'a mut Scopes,
    scope: usize,
    loop_depth: u32,
    /// try statements the walk is in (in the running function)
    try_depth: u32,
}

impl<'a> Collector<'a> {
    fn enter(&mut self, addr: usize, kind: ScopeKind) -> usize {
        let id = self.scopes.scopes.len();
        self.scopes.scopes.push(Scope { kind, parent: Some(self.scope), ..Default::default() });
        self.scopes.by_node.insert(addr, id);
        self.scopes.scopes[self.scope].children.push(id);
        id
    }
    fn cur(&mut self) -> &mut Scope {
        &mut self.scopes.scopes[self.scope]
    }
    fn bind(&mut self, name: String) {
        let w = if self.loop_depth > 0 { 2 } else { 1 };
        *self.cur().binds.entry(name.clone()).or_insert(0) += w;
        self.cur().locals.insert(name);
    }
    /// A walrus target: bound in the nearest scope that is not a generator expression's, and
    /// declared there (`nonlocal`, or `global` at module level) in those between.
    fn bind_walrus(&mut self, name: String) {
        let saved = self.scope;
        while self.scopes.scopes[self.scope].genexp {
            let parent = self.scopes.scopes[self.scope].parent.unwrap_or(0);
            let at_module = self.scopes.scopes[parent].kind == ScopeKind::Module;
            let sc = self.cur();
            if at_module { &mut sc.globals_decl } else { &mut sc.nonlocals_decl }.insert(name.clone());
            sc.uses.insert(name.clone());
            self.scope = parent;
        }
        self.bind(name);
        self.scope = saved;
    }
    /// The parameters' default values, read in the enclosing scope.
    fn visit_defaults(&mut self, params: &ast::Parameters) {
        for p in params.posonlyargs.iter().chain(params.args.iter()).chain(params.kwonlyargs.iter()) {
            if let Some(d) = &p.default {
                self.visit_expr(d);
            }
        }
    }
    /// The parameters, locals of scope `inner`; their annotations are read here.
    fn visit_params(&mut self, inner: usize, params: &ast::Parameters) {
        for p in all_params(params) {
            self.scopes.scopes[inner].locals.insert(p.name.id.to_string());
            if let Some(ann) = &p.annotation {
                self.visit_expr(ann);
            }
        }
    }
}

/// Every parameter, in order: positional-only, positional, keyword-only, `*args`, `**kwargs`.
pub fn all_params(params: &ast::Parameters) -> Vec<&ast::Parameter> {
    let mut out: Vec<&ast::Parameter> = Vec::new();
    out.extend(params.posonlyargs.iter().map(|p| &p.parameter));
    out.extend(params.args.iter().map(|p| &p.parameter));
    out.extend(params.kwonlyargs.iter().map(|p| &p.parameter));
    if let Some(v) = &params.vararg {
        out.push(v);
    }
    if let Some(k) = &params.kwarg {
        out.push(k);
    }
    out
}

impl<'a, 'b> SourceOrderVisitor<'b> for Collector<'a> {
    fn visit_stmt(&mut self, stmt: &'b ast::Stmt) {
        match stmt {
            ast::Stmt::FunctionDef(f) => {
                self.bind(f.name.id.to_string());
                for d in &f.decorator_list {
                    self.visit_expr(&d.expression);
                }
                self.visit_defaults(&f.parameters);
                if let Some(r) = &f.returns {
                    self.visit_expr(r);
                }
                let inner = self.enter(node_addr(f), ScopeKind::Function);
                self.visit_params(inner, &f.parameters);
                let saved = (self.scope, self.loop_depth, self.try_depth);
                self.scope = inner;
                self.loop_depth = 0;
                self.try_depth = 0;
                self.visit_body(&f.body);
                (self.scope, self.loop_depth, self.try_depth) = saved;
            }
            ast::Stmt::ClassDef(c) => {
                self.bind(c.name.id.to_string());
                for d in &c.decorator_list {
                    self.visit_expr(&d.expression);
                }
                if let Some(args) = &c.arguments {
                    for b in args.args.iter() {
                        self.visit_expr(b);
                    }
                    for kw in args.keywords.iter() {
                        self.visit_expr(&kw.value);
                    }
                }
                let inner = self.enter(node_addr(c), ScopeKind::Class);
                let saved = self.scope;
                self.scope = inner;
                for s in &c.body {
                    self.visit_stmt(s);
                }
                self.scope = saved;
            }
            ast::Stmt::Global(g) => {
                for n in &g.names {
                    self.cur().globals_decl.insert(n.id.to_string());
                }
            }
            ast::Stmt::Nonlocal(g) => {
                for n in &g.names {
                    self.cur().nonlocals_decl.insert(n.id.to_string());
                    self.cur().uses.insert(n.id.to_string());
                }
            }
            ast::Stmt::Import(i) => {
                for a in &i.names {
                    let name = a.asname.as_ref().map(|x| x.id.to_string()).unwrap_or_else(|| a.name.id.to_string());
                    let top = name.split('.').next().unwrap_or("").to_string();
                    self.bind(top);
                }
            }
            ast::Stmt::ImportFrom(i) => {
                for a in &i.names {
                    if a.name.id.as_str() != "*" {
                        let name = a.asname.as_ref().map(|x| x.id.to_string()).unwrap_or_else(|| a.name.id.to_string());
                        self.bind(name);
                    }
                }
            }
            ast::Stmt::For(_) | ast::Stmt::While(_) => {
                self.loop_depth += 1;
                source_order::walk_stmt(self, stmt);
                self.loop_depth -= 1;
            }
            ast::Stmt::Try(_) => {
                self.try_depth += 1;
                source_order::walk_stmt(self, stmt);
                self.try_depth -= 1;
            }
            _ => source_order::walk_stmt(self, stmt),
        }
    }

    fn visit_body(&mut self, body: &'b [ast::Stmt]) {
        // (in a function, out of any try: `syntax::consumed_next_genexps`)
        if self.try_depth == 0 && self.scopes.scopes[self.scope].kind == ScopeKind::Function && !self.scopes.scopes[self.scope].genexp {
            for (g, callee) in crate::syntax::consumed_next_genexps(body) {
                self.scopes.consumed_next.insert(node_addr(g), callee.to_string());
            }
        }
        source_order::walk_body(self, body);
    }

    fn visit_expr(&mut self, expr: &'b ast::Expr) {
        match expr {
            ast::Expr::Lambda(l) => {
                if let Some(params) = &l.parameters {
                    self.visit_defaults(params);
                }
                let inner = self.enter(node_addr(l), ScopeKind::Function);
                if let Some(params) = &l.parameters {
                    for p in all_params(params) {
                        self.scopes.scopes[inner].locals.insert(p.name.id.to_string());
                    }
                }
                let saved = self.scope;
                self.scope = inner;
                self.visit_expr(&l.body);
                self.scope = saved;
            }
            ast::Expr::Name(n) => {
                if matches!(n.ctx, ast::ExprContext::Store | ast::ExprContext::Del) {
                    self.bind(n.id.to_string());
                } else {
                    self.cur().uses.insert(n.id.to_string());
                }
            }
            ast::Expr::Named(n) => {
                if let ast::Expr::Name(t) = &*n.target {
                    self.bind_walrus(t.id.to_string());
                }
                self.visit_expr(&n.value);
            }
            ast::Expr::Call(_) | ast::Expr::List(_) | ast::Expr::Tuple(_) | ast::Expr::Set(_) => {
                for g in crate::syntax::eager_genexps(expr) {
                    self.scopes.eager_genexps.insert(node_addr(g));
                }
                source_order::walk_expr(self, expr);
            }
            // a generator expression is a function of its own (run when its items are taken):
            // its first iterable is read here, the rest there
            ast::Expr::Generator(g) if !self.scopes.eager_genexps.contains(&node_addr(expr)) => {
                self.visit_expr(&g.generators[0].iter);
                let inner = self.enter(node_addr(expr), ScopeKind::Function);
                self.scopes.scopes[inner].genexp = true;
                let saved = (self.scope, self.loop_depth);
                (self.scope, self.loop_depth) = (inner, 1);
                for (i, c) in g.generators.iter().enumerate() {
                    if i > 0 {
                        self.visit_expr(&c.iter);
                    }
                    self.visit_expr(&c.target);
                    c.ifs.iter().for_each(|e| self.visit_expr(e));
                }
                self.visit_expr(&g.elt);
                (self.scope, self.loop_depth) = saved;
            }
            _ => source_order::walk_expr(self, expr),
        }
    }

    fn visit_except_handler(&mut self, handler: &'b ast::ExceptHandler) {
        let ast::ExceptHandler::ExceptHandler(h) = handler;
        if let Some(n) = &h.name {
            self.bind(n.id.to_string());
        }
        // (the handler itself, not a copy: scopes are keyed by node addresses)
        source_order::walk_except_handler(self, handler);
    }

    fn visit_pattern(&mut self, p: &'b ast::Pattern) {
        if let ast::Pattern::MatchAs(m) = p {
            if let Some(n) = &m.name {
                self.bind(n.id.to_string());
            }
        }
        source_order::walk_pattern(self, p);
    }
}

/// The second pass, innermost scopes first: a function's read of a name bound by an enclosing
/// function makes it free here (and in the functions between) and a cell there; a name free
/// in a child is free here too when an enclosing function binds it.
fn resolve(scopes: &mut Scopes, id: usize) {
    let children = scopes.scopes[id].children.clone();
    for ch in &children {
        resolve(scopes, *ch);
    }
    if scopes.scopes[id].kind == ScopeKind::Function {
        let names: Vec<String> = scopes.scopes[id].uses.union(&scopes.scopes[id].nonlocals_decl).cloned().collect();
        for name in names {
            if scopes.scopes[id].is_local(&name) || scopes.scopes[id].globals_decl.contains(&name) {
                continue;
            }
            let mut p = scopes.scopes[id].parent;
            let mut path: Vec<usize> = Vec::new();
            while let Some(pid) = p {
                match scopes.scopes[pid].kind {
                    ScopeKind::Function => {
                        if scopes.scopes[pid].is_local(&name) {
                            scopes.scopes[pid].cells.insert(name.clone());
                            if scopes.scopes[id].nonlocals_decl.contains(&name) {
                                scopes.scopes[pid].rebound_cells.insert(name.clone());
                            }
                            scopes.scopes[id].free.insert(name.clone());
                            for mid in &path {
                                scopes.scopes[*mid].free.insert(name.clone());
                            }
                            break;
                        }
                        path.push(pid);
                    }
                    ScopeKind::Module => break,
                    ScopeKind::Class => {}
                }
                p = scopes.scopes[pid].parent;
            }
        }
    }
    for ch in &children {
        let free: Vec<String> = scopes.scopes[*ch].free.iter().cloned().collect();
        for name in free {
            if scopes.scopes[id].kind == ScopeKind::Function && !scopes.scopes[id].is_local(&name) && !scopes.scopes[id].free.contains(&name) {
                let mut p = scopes.scopes[id].parent;
                while let Some(pid) = p {
                    if scopes.scopes[pid].kind == ScopeKind::Module {
                        break;
                    }
                    if scopes.scopes[pid].kind == ScopeKind::Function && scopes.scopes[pid].is_local(&name) {
                        scopes.scopes[id].free.insert(name.clone());
                        break;
                    }
                    p = scopes.scopes[pid].parent;
                }
            }
        }
    }
}

/// The scopes of a module, and its names bound more than once (`Scopes::rebound`).
pub fn analyse(module: &ast::ModModule) -> Scopes {
    let mut scopes = Scopes::default();
    scopes.scopes.push(Scope { kind: ScopeKind::Module, parent: None, ..Default::default() });
    scopes.by_node.insert(node_addr(module), 0);
    {
        let mut coll = Collector { scopes: &mut scopes, scope: 0, loop_depth: 0, try_depth: 0 };
        for s in &module.body {
            coll.visit_stmt(s);
        }
    }
    resolve(&mut scopes, 0);
    let mut rebound: FxHashSet<String> = scopes.scopes[0].binds.iter().filter(|(_, n)| **n >= 2).map(|(k, _)| k.clone()).collect();
    for sc in scopes.scopes.iter().skip(1) {
        for g in sc.globals_decl.iter() {
            if sc.locals.contains(g) {
                rebound.insert(g.clone());
            }
        }
    }
    scopes.rebound = rebound;
    scopes
}
