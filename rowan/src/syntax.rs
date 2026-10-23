//! Questions about the syntax tree that its text alone answers, without running anything:
//! the dotted text of an attribute chain, the names an expression uses, whether a body yields,
//! returns, jumps out of its loop or assigns a name. The interpreter and the models ask them;
//! the answers never depend on the analysis state.

use ruff_python_ast as ast;
use ruff_python_ast::visitor::source_order::{self, SourceOrderVisitor};

use crate::frontend;
use crate::intern::{self, StrId};

/// The dotted text of a name or attribute chain (`os.path.join`), None for anything else
/// (`f().x`, `a[0].b`).
pub fn chain_text(node: &ast::Expr) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    let mut n = node;
    while let ast::Expr::Attribute(a) = n {
        parts.push(a.attr.id.as_str());
        n = &a.value;
    }
    let ast::Expr::Name(nm) = n else { return None };
    parts.push(nm.id.as_str());
    parts.reverse();
    Some(parts.join("."))
}

/// The statement lists directly inside a compound statement, in source order: an `if`'s
/// branches, a loop's body and `else`, a `with`'s body, a `try`'s body, handlers, `else` and
/// `finally`, a `match`'s cases (not a function's or class's body).
pub fn blocks(st: &ast::Stmt) -> Vec<&[ast::Stmt]> {
    match st {
        ast::Stmt::If(i) => std::iter::once(&i.body[..]).chain(i.elif_else_clauses.iter().map(|c| &c.body[..])).collect(),
        ast::Stmt::For(f) => vec![&f.body, &f.orelse],
        ast::Stmt::While(w) => vec![&w.body, &w.orelse],
        ast::Stmt::With(w) => vec![&w.body],
        ast::Stmt::Try(t) => {
            let handlers = t.handlers.iter().map(|h| {
                let ast::ExceptHandler::ExceptHandler(h) = h;
                &h.body[..]
            });
            std::iter::once(&t.body[..]).chain(handlers).chain([&t.orelse[..], &t.finalbody[..]]).collect()
        }
        ast::Stmt::Match(m) => m.cases.iter().map(|c| &c.body[..]).collect(),
        _ => Vec::new(),
    }
}

/// Does a loop body `break` or `continue` this loop (not a nested loop, function or class)?
pub fn contains_jump(body: &[ast::Stmt]) -> bool {
    fn visit(n: &ast::Stmt) -> bool {
        match n {
            ast::Stmt::Break(_) | ast::Stmt::Continue(_) => true,
            // (a nested loop's jumps are its own: only its `else` may jump out of this one)
            ast::Stmt::For(ast::StmtFor { orelse, .. }) | ast::Stmt::While(ast::StmtWhile { orelse, .. }) => orelse.iter().any(visit),
            _ => blocks(n).iter().any(|b| b.iter().any(visit)),
        }
    }
    body.iter().any(visit)
}

/// Does a function body yield (its own yields: not a nested function's)? Remembered per body
/// (trees are leaked: a body's address is its identity), as calls ask it at every call.
pub fn is_generator(body: &'static [ast::Stmt]) -> bool {
    thread_local! { static GENS: std::cell::RefCell<rustc_hash::FxHashMap<(usize, usize), bool>> = Default::default(); }
    let key = (body.as_ptr() as usize, body.len());
    if let Some(g) = GENS.with(|g| g.borrow().get(&key).copied()) {
        return g;
    }
    let g = yields(body);
    GENS.with(|m| m.borrow_mut().insert(key, g));
    g
}

/// `is_generator` uncached.
fn yields(body: &[ast::Stmt]) -> bool {
    struct V(bool);
    impl<'a> SourceOrderVisitor<'a> for V {
        fn visit_expr(&mut self, e: &'a ast::Expr) {
            if matches!(e, ast::Expr::Yield(_) | ast::Expr::YieldFrom(_)) {
                self.0 = true;
            }
            // (a lambda owns its yields)
            if !self.0 && !matches!(e, ast::Expr::Lambda(_)) {
                source_order::walk_expr(self, e);
            }
        }
        fn visit_stmt(&mut self, st: &'a ast::Stmt) {
            // (nested functions and classes own their yields)
            if !self.0 && !matches!(st, ast::Stmt::FunctionDef(_) | ast::Stmt::ClassDef(_)) {
                source_order::walk_stmt(self, st);
            }
        }
    }
    let mut v = V(false);
    for st in body {
        v.visit_stmt(st);
        if v.0 {
            return true;
        }
    }
    false
}

/// How a generator body may be stopped at a yield by a consumer that stops early: None when it
/// delegates (`yield from`); else whether code of its own may catch the GeneratorExit raised
/// there and go on (an `except` clause, a with statement's `__exit__`).
pub fn stop_early_shape(body: &[ast::Stmt]) -> Option<bool> {
    struct V(bool, bool);
    impl<'a> SourceOrderVisitor<'a> for V {
        fn visit_expr(&mut self, e: &'a ast::Expr) {
            self.0 |= matches!(e, ast::Expr::YieldFrom(_));
            if !matches!(e, ast::Expr::Lambda(_) | ast::Expr::Generator(_)) {
                source_order::walk_expr(self, e);
            }
        }
        fn visit_stmt(&mut self, st: &'a ast::Stmt) {
            match st {
                ast::Stmt::FunctionDef(_) | ast::Stmt::ClassDef(_) => return,
                ast::Stmt::Try(t) if !t.handlers.is_empty() => self.1 = true,
                ast::Stmt::With(_) => self.1 = true,
                _ => {}
            }
            source_order::walk_stmt(self, st);
        }
    }
    let mut v = V(false, false);
    body.iter().for_each(|st| v.visit_stmt(st));
    (!v.0).then_some(v.1)
}

/// The statements of a body and of the blocks inside them (not of nested functions or classes).
fn own_stmts(body: &[ast::Stmt]) -> Vec<&ast::Stmt> {
    let mut out = Vec::new();
    for st in body.iter().filter(|st| !matches!(st, ast::Stmt::FunctionDef(_) | ast::Stmt::ClassDef(_))) {
        out.push(st);
        out.extend(blocks(st).into_iter().flat_map(own_stmts));
    }
    out
}

/// Has a function body (not its nested functions) a `while` loop whose test is a constant that
/// is true (`while True:`, `while 1:`): code that may run on without end?
pub fn has_endless_loop(body: &[ast::Stmt]) -> bool {
    own_stmts(body).into_iter().any(|st| match st {
        ast::Stmt::While(w) => match &*w.test {
            ast::Expr::BooleanLiteral(b) => b.value,
            ast::Expr::NumberLiteral(n) => !matches!(&n.value, ast::Number::Int(i) if i.as_u8() == Some(0)),
            e => is_constant(e),
        },
        _ => false,
    })
}

/// May a function body's own `except` clauses catch GeneratorExit (a bare `except:`, one naming
/// GeneratorExit or BaseException)?
pub fn catches_generator_exit(body: &[ast::Stmt]) -> bool {
    let catches = |h: &ast::ExceptHandler| {
        let ast::ExceptHandler::ExceptHandler(h) = h;
        let names: Vec<String> = match h.type_.as_deref() {
            None => vec!["BaseException".into()],
            Some(ast::Expr::Tuple(tp)) => tp.elts.iter().map(dotted_last).collect(),
            Some(e) => vec![dotted_last(e)],
        };
        names.iter().any(|n| n == "GeneratorExit" || n == "BaseException")
    };
    own_stmts(body).into_iter().any(|st| matches!(st, ast::Stmt::Try(t) if t.handlers.iter().any(catches)))
}

/// The last part of a callee as written: `json.loads` → `loads`, `f(x)(y)` → `f`; another
/// expression's text.
pub fn dotted_last(n: &ast::Expr) -> String {
    match n {
        ast::Expr::Name(x) => x.id.to_string(),
        ast::Expr::Attribute(a) => a.attr.id.to_string(),
        ast::Expr::Call(c) => dotted_last(&c.func),
        other => frontend::text(other),
    }
}

/// Calls `f` on every expression inside `e` (itself included; nested lambdas and
/// comprehensions too), in source order.
fn each_expr<'a>(e: &'a ast::Expr, f: &mut dyn FnMut(&'a ast::Expr)) {
    ExprWalk(f).visit_expr(e)
}

/// Calls `f` on every expression in these statements (nested functions and classes too).
fn each_expr_in_body<'a>(body: &'a [ast::Stmt], f: &mut dyn FnMut(&'a ast::Expr)) {
    let mut v = ExprWalk(f);
    body.iter().for_each(|st| v.visit_stmt(st))
}

struct ExprWalk<'a, 'f>(&'f mut dyn FnMut(&'a ast::Expr));

impl<'a> SourceOrderVisitor<'a> for ExprWalk<'a, '_> {
    fn visit_expr(&mut self, e: &'a ast::Expr) {
        (self.0)(e);
        source_order::walk_expr(self, e);
    }
}

/// Every plain name an expression uses, in source order (repeats included).
pub fn names_in(e: &ast::Expr) -> Vec<String> {
    let mut out = Vec::new();
    each_expr(e, &mut |x| {
        if let ast::Expr::Name(n) = x {
            out.push(n.id.to_string());
        }
    });
    out
}

/// Does an expression contain a call (anywhere inside it)?
pub fn contains_call(e: &ast::Expr) -> bool {
    let mut found = false;
    each_expr(e, &mut |x| found |= matches!(x, ast::Expr::Call(_)));
    found
}

/// Functions that take an iterable's items at once, by the name they are called by.
const TAKE_ALL: &[&str] = &["list", "tuple", "set", "frozenset", "dict", "sorted", "sum", "min", "max", "bytes", "bytearray", "join", "extend", "update", "Counter", "deque", "OrderedDict", "cat", "concat", "concatenate", "stack"];

/// The generator expressions whose items are all taken before any other code runs, which
/// therefore run where they are written, as list comprehensions (their names the enclosing
/// code's), among a call's or a display's parts: one unpacked (`f(*g)`, `[*g, x]`, taken as it
/// is reached), and the sole argument of a builtin or method that takes an iterable's items at
/// once (`sum(…)`, `", ".join(…)`, `torch.cat(…)`) with nothing evaluated after it. Taken by
/// name: a name bound to something else (a shadowed `sum`) runs one so too, which is right only
/// if that callee also takes the items at once; otherwise the items' effects come too early (a
/// known gap).
pub fn eager_genexps(e: &ast::Expr) -> Vec<&ast::Expr> {
    let parts: &[ast::Expr] = match e {
        ast::Expr::Call(c) => &c.arguments.args,
        ast::Expr::List(l) => &l.elts,
        ast::Expr::Tuple(t) => &t.elts,
        ast::Expr::Set(s) => &s.elts,
        _ => return Vec::new(),
    };
    let mut out: Vec<&ast::Expr> = parts.iter().filter_map(|p| match p {
        ast::Expr::Starred(st) if matches!(&*st.value, ast::Expr::Generator(_)) => Some(&*st.value),
        _ => None,
    }).collect();
    if let Some(g) = e.as_call_expr().and_then(|c| sole_genexp(c).filter(|_| TAKE_ALL.contains(&callee_name(c)))) {
        out.push(g);
    }
    out
}

/// Generator expressions a body assigns to a name that its next statement first of all gives
/// whole to a function taking every item at once (`entries = (…)` then `for e in
/// sorted(entries):`, `return dict(items)`): nothing runs between, so they may run where
/// written (a raise in them then leaves the name unbound: not for bodies in a `try`).
pub fn consumed_next_genexps(body: &[ast::Stmt]) -> Vec<&ast::Expr> {
    let mut out = Vec::new();
    for w in body.windows(2) {
        let ast::Stmt::Assign(a) = &w[0] else { continue };
        let ([ast::Expr::Name(n)], g @ ast::Expr::Generator(ge)) = (&a.targets[..], &*a.value) else { continue };
        if ge.generators.iter().any(|c| c.is_async) || has_await(g) {
            continue;
        }
        let first: Option<&ast::Expr> = match &w[1] {
            ast::Stmt::For(f) => Some(&f.iter),
            ast::Stmt::Return(r) => r.value.as_deref(),
            ast::Stmt::Assign(x) => Some(&x.value),
            ast::Stmt::AnnAssign(x) => x.value.as_deref(),
            ast::Stmt::Expr(e) => Some(&e.value),
            _ => None,
        };
        let Some(ast::Expr::Call(c)) = first else { continue };
        let sole = matches!(&c.arguments.args[..], [ast::Expr::Name(x)] if x.id == n.id) && c.arguments.keywords.iter().all(|k| k.arg.is_some() && runs_no_code(&k.value));
        if sole && TAKE_ALL.contains(&callee_name(c)) && matches!(&*c.func, ast::Expr::Name(_)) {
            out.push(g);
        }
    }
    out
}

/// A call's sole argument when it is a generator expression and nothing is evaluated after it
/// but code that runs none of the program's (`runs_no_code`).
pub fn sole_genexp(c: &ast::ExprCall) -> Option<&ast::Expr> {
    match c.arguments.args.as_ref() {
        [g @ ast::Expr::Generator(_)] if c.arguments.keywords.iter().all(|k| k.arg.is_some() && runs_no_code(&k.value)) => Some(g),
        _ => None,
    }
}

/// The last name of what a call calls (`f` of `f(…)`, `join` of `s.join(…)`).
fn callee_name(c: &ast::ExprCall) -> &str {
    match &*c.func {
        ast::Expr::Name(n) => n.id.as_str(),
        ast::Expr::Attribute(a) => a.attr.as_str(),
        _ => "",
    }
}

/// Does evaluating `e` run none of the program's code but operators' dunders (names, constants,
/// lambdas, operators, tests and tuples or lists of them)?
pub fn runs_no_code(e: &ast::Expr) -> bool {
    match e {
        ast::Expr::Name(_) | ast::Expr::Lambda(_) | ast::Expr::NumberLiteral(_) | ast::Expr::StringLiteral(_) | ast::Expr::BytesLiteral(_) | ast::Expr::BooleanLiteral(_) | ast::Expr::NoneLiteral(_) | ast::Expr::EllipsisLiteral(_) => true,
        ast::Expr::If(x) => runs_no_code(&x.test) && runs_no_code(&x.body) && runs_no_code(&x.orelse),
        ast::Expr::BoolOp(x) => x.values.iter().all(runs_no_code),
        ast::Expr::Compare(x) => runs_no_code(&x.left) && x.comparators.iter().all(runs_no_code),
        ast::Expr::BinOp(x) => runs_no_code(&x.left) && runs_no_code(&x.right),
        ast::Expr::UnaryOp(x) => runs_no_code(&x.operand),
        ast::Expr::Tuple(x) => x.elts.iter().all(runs_no_code),
        ast::Expr::List(x) => x.elts.iter().all(runs_no_code),
        _ => false,
    }
}

/// Does an expression contain an `await` (anywhere inside it)?
pub fn has_await(e: &ast::Expr) -> bool {
    let mut found = false;
    each_expr(e, &mut |x| found |= matches!(x, ast::Expr::Await(_)));
    found
}

/// Is the expression a literal constant (str, bytes, number, bool, None, `...`)?
pub fn is_constant(e: &ast::Expr) -> bool {
    matches!(
        e,
        ast::Expr::StringLiteral(_) | ast::Expr::NumberLiteral(_) | ast::Expr::BooleanLiteral(_) | ast::Expr::NoneLiteral(_) | ast::Expr::BytesLiteral(_) | ast::Expr::EllipsisLiteral(_)
    )
}

/// How many times the plain name `name` occurs in these statements (nested functions too).
pub fn body_mentions(body: &[ast::Stmt], name: &str) -> usize {
    let mut n = 0;
    each_expr_in_body(body, &mut |x| {
        if matches!(x, ast::Expr::Name(nm) if nm.id.as_str() == name) {
            n += 1;
        }
    });
    n
}

/// A `return` somewhere in these statements (not inside nested functions or classes).
pub fn contains_return(body: &[ast::Stmt]) -> bool {
    body.iter().any(|s| matches!(s, ast::Stmt::Return(_)) || blocks(s).iter().any(|b| contains_return(b)))
}

/// Plain names a statement (and the blocks inside it) assigns.
pub fn collect_assigned(st: &ast::Stmt, out: &mut Vec<StrId>) {
    let add = |t: &ast::Expr, out: &mut Vec<StrId>| {
        for n in names_in(t) {
            let id = intern::s(&n);
            if !out.contains(&id) {
                out.push(id);
            }
        }
    };
    match st {
        ast::Stmt::Assign(a) => a.targets.iter().for_each(|t| add(t, out)),
        ast::Stmt::AnnAssign(a) => add(&a.target, out),
        ast::Stmt::AugAssign(a) => add(&a.target, out),
        ast::Stmt::If(i) => {
            i.body.iter().for_each(|x| collect_assigned(x, out));
            i.elif_else_clauses.iter().for_each(|c| c.body.iter().for_each(|x| collect_assigned(x, out)));
        }
        ast::Stmt::Try(t) => {
            t.body.iter().chain(t.orelse.iter()).chain(t.finalbody.iter()).for_each(|x| collect_assigned(x, out));
            for h in &t.handlers {
                let ast::ExceptHandler::ExceptHandler(h) = h;
                h.body.iter().for_each(|x| collect_assigned(x, out));
            }
        }
        ast::Stmt::With(w) => w.body.iter().for_each(|x| collect_assigned(x, out)),
        _ => {}
    }
}

/// `__name__ == "__main__"` (either way round).
pub fn is_main_test(test: &ast::Expr) -> bool {
    let ast::Expr::Compare(c) = test else { return false };
    let [op] = &c.ops[..] else { return false };
    let [right] = &c.comparators[..] else { return false };
    let name = |e: &ast::Expr| matches!(e, ast::Expr::Name(n) if n.id.as_str() == "__name__");
    let main = |e: &ast::Expr| matches!(e, ast::Expr::StringLiteral(s) if s.value.to_str() == "__main__");
    *op == ast::CmpOp::Eq && ((name(&c.left) && main(right)) || (main(&c.left) && name(right)))
}

/// May these statements `return` a true value (anything but `None` / `False`; not in nested
/// functions)? (`__exit__` swallowing the exception)
pub fn may_return_true(body: &[ast::Stmt]) -> bool {
    struct V(bool);
    impl<'a> SourceOrderVisitor<'a> for V {
        fn visit_stmt(&mut self, s: &'a ast::Stmt) {
            match s {
                ast::Stmt::FunctionDef(_) | ast::Stmt::ClassDef(_) => {}
                ast::Stmt::Return(r) => {
                    let falsy = match r.value.as_deref() {
                        None | Some(ast::Expr::NoneLiteral(_)) => true,
                        Some(ast::Expr::BooleanLiteral(b)) => !b.value,
                        _ => false,
                    };
                    self.0 |= !falsy;
                }
                _ => source_order::walk_stmt(self, s),
            }
        }
    }
    let mut v = V(false);
    body.iter().for_each(|s| v.visit_stmt(s));
    v.0
}

/// What these statements assign to the plain name `name` (`name = e`, `name &= e`, `name: T = e`:
/// each `e`; not in nested functions or classes).
pub fn assignments_to(body: &'static [ast::Stmt], name: &str) -> Vec<&'static ast::Expr> {
    use ast::{Expr, Stmt};
    struct V<'n>(&'n str, Vec<&'static Expr>);
    impl SourceOrderVisitor<'static> for V<'_> {
        fn visit_stmt(&mut self, st: &'static Stmt) {
            let name = self.0;
            let named = |t: &Expr| matches!(t, Expr::Name(n) if n.id.as_str() == name);
            match st {
                Stmt::FunctionDef(_) | Stmt::ClassDef(_) => return,
                Stmt::Assign(a) if a.targets.iter().any(named) => self.1.push(&a.value),
                Stmt::AugAssign(a) if named(&a.target) => self.1.push(&a.value),
                Stmt::AnnAssign(a) if named(&a.target) => self.1.extend(a.value.as_deref()),
                _ => {}
            }
            source_order::walk_stmt(self, st);
        }
    }
    let mut v = V(name, Vec::new());
    for st in body {
        v.visit_stmt(st);
    }
    v.1
}

/// Every statement in a body, nested ones too (in functions, classes and blocks), each
/// before the statements inside it.
pub fn walk_stmts(body: &'static [ast::Stmt]) -> Vec<&'static ast::Stmt> {
    fn go(s: &'static ast::Stmt, out: &mut Vec<&'static ast::Stmt>) {
        out.push(s);
        let own: &'static [ast::Stmt] = match s {
            ast::Stmt::FunctionDef(f) => &f.body,
            ast::Stmt::ClassDef(c) => &c.body,
            _ => &[],
        };
        for b in std::iter::once(own).chain(blocks(s)) {
            b.iter().for_each(|x| go(x, out));
        }
    }
    let mut out: Vec<&'static ast::Stmt> = Vec::new();
    body.iter().for_each(|x| go(x, &mut out));
    out
}

/// The literal texts a str template's result starts and ends with: an f-string (around its
/// fields), `"…%s…" % x` (simple conversions, no `%%`), `"…{}…".format(…)` (no `{{` / `}}`);
/// None for other expressions.
pub fn template_affix(e: &ast::Expr) -> Option<(String, String)> {
    let ends = |parts: &[Option<String>]| -> (String, String) {
        let first = parts.iter().position(Option::is_none).unwrap_or(parts.len());
        let last = parts.iter().rposition(Option::is_none).map_or(parts.len(), |i| i + 1);
        let cat = |ps: &[Option<String>]| ps.iter().flatten().cloned().collect::<String>();
        (cat(&parts[..first]), cat(&parts[last.max(first)..]))
    };
    match e {
        ast::Expr::FString(f) => {
            let mut parts: Vec<Option<String>> = Vec::new();
            for p in f.value.iter() {
                match p {
                    ast::FStringPart::Literal(l) => parts.push(Some(l.value.to_string())),
                    ast::FStringPart::FString(fs) => {
                        for el in fs.elements.iter() {
                            match el {
                                ast::InterpolatedStringElement::Literal(l) => parts.push(Some(l.value.to_string())),
                                ast::InterpolatedStringElement::Interpolation(_) => parts.push(None),
                            }
                        }
                    }
                }
            }
            Some(ends(&parts))
        }
        ast::Expr::BinOp(b) if b.op == ast::Operator::Mod => {
            let ast::Expr::StringLiteral(s) = &*b.left else { return None };
            let fmt = s.value.to_str();
            let mut parts: Vec<Option<String>> = Vec::new();
            let mut rest = fmt;
            while let Some(i) = rest.find('%') {
                parts.push(Some(rest[..i].to_string()));
                let conv = rest[i + 1..].chars().next()?;
                if !"sdria".contains(conv) {
                    return None;
                }
                parts.push(None);
                rest = &rest[i + 2..];
            }
            parts.push(Some(rest.to_string()));
            Some(ends(&parts))
        }
        ast::Expr::Call(c) => {
            let ast::Expr::Attribute(a) = &*c.func else { return None };
            let ast::Expr::StringLiteral(s) = &*a.value else { return None };
            let fmt = s.value.to_str();
            if a.attr.as_str() != "format" || fmt.contains("{{") || fmt.contains("}}") {
                return None;
            }
            match (fmt.find('{'), fmt.rfind('}')) {
                (Some(i), Some(j)) if i < j => Some((fmt[..i].to_string(), fmt[j + 1..].to_string())),
                (None, None) => Some((fmt.to_string(), String::new())),
                _ => None,
            }
        }
        _ => None,
    }
}

/// What a function's local `name` may hold when every binding of it in the function's own code
/// is `name = e` (its one target): each `e`. None when another binding may be the one (a
/// parameter, `global` / `nonlocal`, a loop, `with`, `except` or import target, unpacking, `:=`,
/// `+=`, `del`, `match`, a nested function or class of that name) or there is none.
pub fn sole_assignments(f: &'static ast::StmtFunctionDef, name: &str) -> Option<Vec<&'static ast::Expr>> {
    use ast::{Expr, Stmt};
    struct V<'n> {
        name: &'n str,
        out: Vec<&'static Expr>,
        other: bool,
    }
    impl SourceOrderVisitor<'static> for V<'_> {
        fn visit_stmt(&mut self, st: &'static Stmt) {
            let n = self.name;
            let named = |t: &Expr| matches!(t, Expr::Name(x) if x.id.as_str() == n);
            match st {
                Stmt::FunctionDef(d) => self.other |= d.name.as_str() == n,
                Stmt::ClassDef(d) => self.other |= d.name.as_str() == n,
                Stmt::Global(g) => self.other |= g.names.iter().any(|x| x.as_str() == n),
                Stmt::Nonlocal(g) => self.other |= g.names.iter().any(|x| x.as_str() == n),
                Stmt::Import(i) => self.other |= i.names.iter().any(|a| a.asname.as_ref().map_or(a.name.as_str().split('.').next() == Some(n), |x| x.as_str() == n)),
                Stmt::ImportFrom(i) => self.other |= i.names.iter().any(|a| a.asname.as_ref().map_or(a.name.as_str() == n || a.name.as_str() == "*", |x| x.as_str() == n)),
                Stmt::Match(_) => self.other = true,
                Stmt::Assign(a) if a.targets.len() == 1 && named(&a.targets[0]) => {
                    self.out.push(&a.value);
                    self.visit_expr(&a.value);
                }
                Stmt::AnnAssign(a) if named(&a.target) => {
                    if let Some(v) = a.value.as_deref() {
                        self.out.push(v);
                        self.visit_expr(v);
                    }
                }
                Stmt::Try(t) => {
                    self.other |= t.handlers.iter().any(|h| {
                        let ast::ExceptHandler::ExceptHandler(h) = h;
                        h.name.as_ref().is_some_and(|x| x.as_str() == n)
                    });
                    source_order::walk_stmt(self, st);
                }
                _ => source_order::walk_stmt(self, st),
            }
        }
        fn visit_expr(&mut self, e: &'static Expr) {
            match e {
                Expr::Name(x) if x.id.as_str() == self.name && !matches!(x.ctx, ast::ExprContext::Load) => self.other = true,
                Expr::Lambda(_) => {}
                _ => source_order::walk_expr(self, e),
            }
        }
    }
    let p = &f.parameters;
    let params = p.posonlyargs.iter().chain(p.args.iter()).chain(p.kwonlyargs.iter()).map(|a| a.parameter.name.as_str()).chain(p.vararg.iter().chain(p.kwarg.iter()).map(|a| a.name.as_str()));
    if params.into_iter().any(|x| x == name) {
        return None;
    }
    let mut v = V { name, out: Vec::new(), other: false };
    for st in &f.body {
        v.visit_stmt(st);
    }
    (!v.other && !v.out.is_empty()).then_some(v.out)
}
