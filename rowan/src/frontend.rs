//! Parsing and source positions. Modules are parsed with ruff's parser, private names
//! mangled as CPython does (`Mangler`), and leaked (`&'static`) for the lifetime of the
//! process: the interpreter walks them by reference from every frame, and a node's address is
//! its identity. A thread-local registry maps any node's address back to its module, and so
//! to its position and text (`node`, `text`).

use std::cell::RefCell;

use ruff_python_ast as ast;
use ruff_python_ast::visitor::source_order::{self, SourceOrderVisitor};
use ruff_python_ast::visitor::transformer::{walk_except_handler, walk_expr, walk_keyword, walk_parameter, walk_stmt, Transformer};
use ruff_source_file::LineIndex;
use ruff_text_size::{Ranged, TextRange};
use rustc_hash::FxHashMap;

/// A parsed module's name, path, text and line index.
pub struct ModuleSource {
    pub name: String,
    pub path: Option<String>,
    pub source: String,
    pub lines: LineIndex,
}

/// The modules parsed on this thread (by index), and the module of every node address.
struct Registry {
    modules: Vec<ModuleSource>,
    nodes: FxHashMap<usize, u32>,
}

thread_local! {
    static REG: RefCell<Registry> = RefCell::new(Registry { modules: Vec::new(), nodes: FxHashMap::default() });
}

thread_local! {
    static MAIN_IDX: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// The module index of the analysed file: its sites carry no module prefix.
pub fn set_main(idx: u32) {
    MAIN_IDX.with(|m| m.set(idx));
}

/// Prefix that keeps allocation and call sites of different modules apart (`M2:L5:3`);
/// empty for the main module so its site names stay `L5:3`.
pub fn site_prefix(module: u32) -> String {
    if MAIN_IDX.with(|m| m.get()) == module {
        String::new()
    } else {
        format!("M{}:", module)
    }
}

/// A located AST node: identity (address, 0 for synthetic nodes), module and position.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Node {
    pub addr: usize,
    pub module: u32,
    pub line: u32,
    pub end_line: u32,
    pub col: u32,
    pub end_col: u32,
}

impl Node {
    /// A node made up for code that has none (an entry run's call, a model's call), at a line.
    pub fn synthetic(module: u32, line: u32) -> Node {
        Node { addr: 0, module, line, end_line: line, col: 0, end_col: 0 }
    }
    /// No node: an observation, not a read of the program.
    pub fn none() -> Node {
        Node::synthetic(0, 0)
    }
}

thread_local! {
    /// parsed modules by (path, content hash): a long-lived process (`rowan serve`) parses
    /// each version of a file once, and the leaked trees stay bounded by distinct versions
    static PARSE_CACHE: std::cell::RefCell<std::collections::HashMap<(String, u64), (u32, &'static ast::ModModule)>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// A content hash for cache keys (not cryptographic).
pub fn content_hash(source: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = rustc_hash::FxHasher::default();
    source.hash(&mut h);
    h.finish()
}

thread_local! {
    /// time spent parsing on this thread (`parse_time`)
    static PARSE_TIME: std::cell::Cell<std::time::Duration> = const { std::cell::Cell::new(std::time::Duration::ZERO) };
}

/// The time this thread spent parsing modules so far (`rowan hovers`' setup report).
pub fn parse_time() -> std::time::Duration {
    PARSE_TIME.with(|t| t.get())
}

/// Parses module `name` (at `path`), registers its nodes and gives its index and tree; the
/// same text at the same path again gives the same module (`PARSE_CACHE`).
pub fn parse(name: &str, path: Option<&str>, source: String) -> Result<(u32, &'static ast::ModModule), String> {
    let t0 = std::time::Instant::now();
    let r = parse_uncounted(name, path, source);
    PARSE_TIME.with(|t| t.set(t.get() + t0.elapsed()));
    r
}

/// `parse` without the timing: CRLF line ends normalised, the cache consulted, the tree
/// parsed, mangled, leaked and its nodes registered.
fn parse_uncounted(name: &str, path: Option<&str>, source: String) -> Result<(u32, &'static ast::ModModule), String> {
    // (Windows line ends as `\n`: lines and columns are the same, texts over lines read alike)
    let source = if source.contains("\r\n") { source.replace("\r\n", "\n") } else { source };
    let key = path.map(|p| (p.to_string(), content_hash(&source)));
    if let Some(k) = &key {
        if let Some(hit) = PARSE_CACHE.with(|c| c.borrow().get(k).copied()) {
            return Ok(hit);
        }
    }
    let parsed = ruff_python_parser::parse_module(&source).map_err(|e| format!("{}: {}", path.unwrap_or(name), e))?;
    let mut syntax = parsed.into_syntax();
    let mangler = Mangler(std::cell::RefCell::new(Vec::new()));
    for st in syntax.body.iter_mut() {
        mangler.visit_stmt(st);
    }
    let module: &'static ast::ModModule = Box::leak(Box::new(syntax));
    let lines = LineIndex::from_source_text(&source);
    let idx = REG.with(|r| {
        let mut r = r.borrow_mut();
        r.modules.push(ModuleSource { name: name.to_string(), path: path.map(|p| p.to_string()), source, lines });
        (r.modules.len() - 1) as u32
    });
    let mut v = Registrar { addrs: Vec::new() };
    for s in &module.body {
        v.visit_stmt(s);
    }
    v.addrs.push(module as *const _ as usize);
    REG.with(|r| {
        let mut r = r.borrow_mut();
        for a in v.addrs {
            r.nodes.insert(a, idx);
        }
    });
    if let Some(k) = key {
        PARSE_CACHE.with(|c| c.borrow_mut().insert(k, (idx, module)));
    }
    Ok((idx, module))
}

/// Python's private-name mangling, done at compile time as CPython does: an identifier
/// `__x` (not ending in `__`) written inside a class `C` is `_C__x` (the innermost class,
/// leading underscores of its name dropped).
struct Mangler(std::cell::RefCell<Vec<String>>);

impl Mangler {
    /// `__x` inside the innermost class `C` becomes `_C__x`.
    fn mangle(&self, id: &mut ast::name::Name) {
        let Some(cls) = self.0.borrow().last().cloned() else { return };
        if id.starts_with("__") && !id.ends_with("__") && !id.contains('.') && !cls.is_empty() {
            *id = ast::name::Name::new(format!("_{}{}", cls, id));
        }
    }
}

impl Transformer for Mangler {
    fn visit_stmt(&self, stmt: &mut ast::Stmt) {
        match stmt {
            ast::Stmt::ClassDef(c) => {
                self.mangle(&mut c.name.id);
                for d in c.decorator_list.iter_mut() {
                    self.visit_decorator(d);
                }
                if let Some(a) = c.arguments.as_deref_mut() {
                    self.visit_arguments(a);
                }
                self.0.borrow_mut().push(c.name.id.trim_start_matches('_').to_string());
                self.visit_body(&mut c.body);
                self.0.borrow_mut().pop();
            }
            ast::Stmt::FunctionDef(f) => {
                self.mangle(&mut f.name.id);
                walk_stmt(self, stmt);
            }
            ast::Stmt::Global(g) => g.names.iter_mut().for_each(|n| self.mangle(&mut n.id)),
            ast::Stmt::Nonlocal(g) => g.names.iter_mut().for_each(|n| self.mangle(&mut n.id)),
            _ => walk_stmt(self, stmt),
        }
    }
    fn visit_expr(&self, expr: &mut ast::Expr) {
        match expr {
            ast::Expr::Name(n) => self.mangle(&mut n.id),
            ast::Expr::Attribute(a) => self.mangle(&mut a.attr.id),
            _ => {}
        }
        walk_expr(self, expr);
    }
    fn visit_parameter(&self, p: &mut ast::Parameter) {
        self.mangle(&mut p.name.id);
        walk_parameter(self, p);
    }
    fn visit_keyword(&self, k: &mut ast::Keyword) {
        if let Some(a) = k.arg.as_mut() {
            self.mangle(&mut a.id);
        }
        walk_keyword(self, k);
    }
    fn visit_except_handler(&self, h: &mut ast::ExceptHandler) {
        let ast::ExceptHandler::ExceptHandler(e) = h;
        if let Some(n) = e.name.as_mut() {
            self.mangle(&mut n.id);
        }
        walk_except_handler(self, h);
    }
}

/// Collects the addresses of every node of a parsed module (`node` locates them).
struct Registrar {
    addrs: Vec<usize>,
}

/// Address of the payload struct inside a `Stmt` (the interpreter locates nodes by either).
fn stmt_inner_addr(s: &ast::Stmt) -> usize {
    use ast::Stmt::*;
    match s {
        FunctionDef(x) => x as *const _ as usize,
        ClassDef(x) => x as *const _ as usize,
        Return(x) => x as *const _ as usize,
        Delete(x) => x as *const _ as usize,
        TypeAlias(x) => x as *const _ as usize,
        Assign(x) => x as *const _ as usize,
        AugAssign(x) => x as *const _ as usize,
        AnnAssign(x) => x as *const _ as usize,
        For(x) => x as *const _ as usize,
        While(x) => x as *const _ as usize,
        If(x) => x as *const _ as usize,
        With(x) => x as *const _ as usize,
        Match(x) => x as *const _ as usize,
        Raise(x) => x as *const _ as usize,
        Try(x) => x as *const _ as usize,
        Assert(x) => x as *const _ as usize,
        Import(x) => x as *const _ as usize,
        ImportFrom(x) => x as *const _ as usize,
        Global(x) => x as *const _ as usize,
        Nonlocal(x) => x as *const _ as usize,
        Expr(x) => x as *const _ as usize,
        Pass(x) => x as *const _ as usize,
        Break(x) => x as *const _ as usize,
        Continue(x) => x as *const _ as usize,
        IpyEscapeCommand(x) => x as *const _ as usize,
    }
}

/// Address of the payload struct inside an `Expr` (as `stmt_inner_addr`).
fn expr_inner_addr(e: &ast::Expr) -> usize {
    use ast::Expr::*;
    match e {
        BoolOp(x) => x as *const _ as usize,
        Named(x) => x as *const _ as usize,
        BinOp(x) => x as *const _ as usize,
        UnaryOp(x) => x as *const _ as usize,
        Lambda(x) => x as *const _ as usize,
        If(x) => x as *const _ as usize,
        Dict(x) => x as *const _ as usize,
        Set(x) => x as *const _ as usize,
        ListComp(x) => x as *const _ as usize,
        SetComp(x) => x as *const _ as usize,
        DictComp(x) => x as *const _ as usize,
        Generator(x) => x as *const _ as usize,
        Await(x) => x as *const _ as usize,
        Yield(x) => x as *const _ as usize,
        YieldFrom(x) => x as *const _ as usize,
        Compare(x) => x as *const _ as usize,
        Call(x) => x as *const _ as usize,
        FString(x) => x as *const _ as usize,
        TString(x) => x as *const _ as usize,
        StringLiteral(x) => x as *const _ as usize,
        BytesLiteral(x) => x as *const _ as usize,
        NumberLiteral(x) => x as *const _ as usize,
        BooleanLiteral(x) => x as *const _ as usize,
        NoneLiteral(x) => x as *const _ as usize,
        EllipsisLiteral(x) => x as *const _ as usize,
        Attribute(x) => x as *const _ as usize,
        Subscript(x) => x as *const _ as usize,
        Starred(x) => x as *const _ as usize,
        Name(x) => x as *const _ as usize,
        List(x) => x as *const _ as usize,
        Tuple(x) => x as *const _ as usize,
        Slice(x) => x as *const _ as usize,
        IpyEscapeCommand(x) => x as *const _ as usize,
    }
}

impl<'a> SourceOrderVisitor<'a> for Registrar {
    fn visit_stmt(&mut self, stmt: &'a ast::Stmt) {
        self.addrs.push(stmt as *const _ as usize);
        self.addrs.push(stmt_inner_addr(stmt));
        source_order::walk_stmt(self, stmt);
    }
    fn visit_expr(&mut self, expr: &'a ast::Expr) {
        self.addrs.push(expr as *const _ as usize);
        self.addrs.push(expr_inner_addr(expr));
        source_order::walk_expr(self, expr);
    }
    fn visit_except_handler(&mut self, h: &'a ast::ExceptHandler) {
        self.addrs.push(h as *const _ as usize);
        let ast::ExceptHandler::ExceptHandler(inner) = h;
        self.addrs.push(inner as *const _ as usize);
        source_order::walk_except_handler(self, h);
    }
    fn visit_comprehension(&mut self, c: &'a ast::Comprehension) {
        self.addrs.push(c as *const _ as usize);
        source_order::walk_comprehension(self, c);
    }
    fn visit_decorator(&mut self, d: &'a ast::Decorator) {
        self.addrs.push(d as *const _ as usize);
        source_order::walk_decorator(self, d);
    }
    fn visit_elif_else_clause(&mut self, c: &'a ast::ElifElseClause) {
        self.addrs.push(c as *const _ as usize);
        source_order::walk_elif_else_clause(self, c);
    }
    fn visit_match_case(&mut self, c: &'a ast::MatchCase) {
        self.addrs.push(c as *const _ as usize);
        source_order::walk_match_case(self, c);
    }
    fn visit_parameter(&mut self, p: &'a ast::Parameter) {
        self.addrs.push(p as *const _ as usize);
        source_order::walk_parameter(self, p);
    }
    fn visit_keyword(&mut self, k: &'a ast::Keyword) {
        self.addrs.push(k as *const _ as usize);
        source_order::walk_keyword(self, k);
    }
    fn visit_with_item(&mut self, w: &'a ast::WithItem) {
        self.addrs.push(w as *const _ as usize);
        source_order::walk_with_item(self, w);
    }
    fn visit_alias(&mut self, a: &'a ast::Alias) {
        self.addrs.push(a as *const _ as usize);
        source_order::walk_alias(self, a);
    }
}

/// (line, end line, column, end column) of a range in a module; lines 1-based, columns in bytes.
fn locate(module: u32, range: TextRange) -> (u32, u32, u32, u32) {
    REG.with(|r| {
        let r = r.borrow();
        let m = &r.modules[module as usize];
        let start = m.lines.line_column(range.start(), &m.source);
        let end = m.lines.line_column(range.end(), &m.source);
        let line = start.line.get() as u32;
        let end_line = end.line.get() as u32;
        let ls = m.lines.line_start(start.line, &m.source);
        let le = m.lines.line_start(end.line, &m.source);
        let col = (range.start() - ls).to_u32();
        let end_col = (range.end() - le).to_u32();
        (line, end_line, col, end_col)
    })
}

/// Locate a registered node (any Stmt / Expr / handler / comprehension of a parsed module).
pub fn node<T: Ranged>(n: &T) -> Node {
    let addr = n as *const T as usize;
    let module = REG.with(|r| r.borrow().nodes.get(&addr).copied());
    let Some(module) = module else {
        return Node { addr, module: 0, line: 0, end_line: 0, col: 0, end_col: 0 };
    };
    let (line, end_line, col, end_col) = locate(module, n.range());
    Node { addr, module, line, end_line, col, end_col }
}

/// The node of `n` in module `module` (a part the registry does not keep: a parameter).
pub fn node_in<T: Ranged>(module: u32, n: &T) -> Node {
    let (line, end_line, col, end_col) = locate(module, n.range());
    Node { addr: n as *const T as usize, module, line, end_line, col, end_col }
}

/// The module a registered node (by address) is in.
pub fn module_of_addr(addr: usize) -> Option<u32> {
    REG.with(|r| r.borrow().nodes.get(&addr).copied())
}

/// Source text of a node (stands in for `ast.unparse`).
pub fn text<T: Ranged>(n: &T) -> String {
    let addr = n as *const T as usize;
    REG.with(|r| {
        let r = r.borrow();
        let Some(&m) = r.nodes.get(&addr) else { return String::new() };
        let src = &r.modules[m as usize].source;
        let rg = n.range();
        src.get(rg.start().to_usize()..rg.end().to_usize()).map(|s| s.to_string()).unwrap_or_default()
    })
}

/// The source text of `range` in the module node `n` (registered) is in.
pub fn text_in<T: Ranged>(n: &T, range: ruff_text_size::TextRange) -> String {
    let addr = n as *const T as usize;
    REG.with(|r| {
        let r = r.borrow();
        let Some(&m) = r.nodes.get(&addr) else { return String::new() };
        r.modules[m as usize].source.get(range.start().to_usize()..range.end().to_usize()).map(|s| s.to_string()).unwrap_or_default()
    })
}

/// The dotted name a module was imported as (`__main__` for the analysed file).
pub fn module_name(idx: u32) -> String {
    REG.with(|r| r.borrow().modules[idx as usize].name.clone())
}

/// The file a module was read from.
pub fn module_path(idx: u32) -> Option<String> {
    REG.with(|r| r.borrow().modules[idx as usize].path.clone())
}

/// Does a `.send(` call appear in any module parsed so far? (a `yield` expression is None
/// unless something is sent; cached per module count)
pub fn any_send_call() -> bool {
    thread_local! {
        static SEND: RefCell<(usize, bool)> = const { RefCell::new((0, false)) };
    }
    REG.with(|r| {
        let r = r.borrow();
        SEND.with(|c| {
            let mut c = c.borrow_mut();
            if c.0 != r.modules.len() {
                *c = (r.modules.len(), r.modules.iter().any(|m| m.source.contains(".send(")));
            }
            c.1
        })
    })
}

/// Parses one expression (a string annotation, a pandas query), leaked like a module but not
/// registered: it has no position.
pub fn parse_expression(source: &str) -> Option<&'static ast::Expr> {
    let parsed = ruff_python_parser::parse_expression(source).ok()?;
    let e: &'static ast::ModExpression = Box::leak(Box::new(parsed.into_syntax()));
    Some(&e.body)
}

/// Source text at a located node (start..end on possibly several lines).
pub fn text_at(n: &Node) -> String {
    REG.with(|r| {
        let r = r.borrow();
        let Some(m) = r.modules.get(n.module as usize) else { return String::new() };
        if n.line == 0 {
            return String::new();
        }
        let start_line = ruff_source_file::OneIndexed::new(n.line as usize).unwrap();
        let end_line = ruff_source_file::OneIndexed::new(n.end_line.max(1) as usize).unwrap();
        let s = m.lines.line_start(start_line, &m.source).to_usize() + n.col as usize;
        let e = m.lines.line_start(end_line, &m.source).to_usize() + n.end_col as usize;
        m.source.get(s..e).map(|t| t.to_string()).unwrap_or_default()
    })
}

/// A column in UTF-16 code units (what editors count) from a byte column on a 1-based line.
pub fn utf16_col(module: u32, line: u32, col: u32) -> u32 {
    REG.with(|r| {
        let r = r.borrow();
        let Some(m) = r.modules.get(module as usize) else { return col };
        let Some(l) = ruff_source_file::OneIndexed::new(line as usize) else { return col };
        let s = m.lines.line_start(l, &m.source).to_usize();
        m.source.get(s..s + col as usize).map(|t| t.encode_utf16().count() as u32).unwrap_or(col)
    })
}
