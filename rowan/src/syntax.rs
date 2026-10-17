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

/// Does a loop body `break` or `continue` this loop (not a nested loop, function or class)?
pub fn contains_jump(body: &[ast::Stmt]) -> bool {
    fn visit(n: &ast::Stmt) -> bool {
        match n {
            ast::Stmt::Break(_) | ast::Stmt::Continue(_) => true,
            ast::Stmt::For(f) => f.orelse.iter().any(visit),
            ast::Stmt::While(w) => w.orelse.iter().any(visit),
            ast::Stmt::FunctionDef(_) | ast::Stmt::ClassDef(_) => false,
            ast::Stmt::If(i) => i.body.iter().any(visit) || i.elif_else_clauses.iter().any(|c| c.body.iter().any(visit)),
            ast::Stmt::With(w) => w.body.iter().any(visit),
            ast::Stmt::Try(t) => {
                t.body.iter().any(visit)
                    || t.handlers.iter().any(|h| {
                        let ast::ExceptHandler::ExceptHandler(h) = h;
                        h.body.iter().any(visit)
                    })
                    || t.orelse.iter().any(visit)
                    || t.finalbody.iter().any(visit)
            }
            ast::Stmt::Match(m) => m.cases.iter().any(|c| c.body.iter().any(visit)),
            _ => false,
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
    body.iter().any(|s| match s {
        ast::Stmt::Return(_) => true,
        ast::Stmt::If(i) => contains_return(&i.body) || i.elif_else_clauses.iter().any(|c| contains_return(&c.body)),
        ast::Stmt::For(f) => contains_return(&f.body) || contains_return(&f.orelse),
        ast::Stmt::While(w) => contains_return(&w.body) || contains_return(&w.orelse),
        ast::Stmt::With(w) => contains_return(&w.body),
        ast::Stmt::Try(t) => {
            contains_return(&t.body)
                || contains_return(&t.orelse)
                || contains_return(&t.finalbody)
                || t.handlers.iter().any(|h| {
                    let ast::ExceptHandler::ExceptHandler(h) = h;
                    contains_return(&h.body)
                })
        }
        ast::Stmt::Match(m) => m.cases.iter().any(|c| contains_return(&c.body)),
        _ => false,
    })
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
    let mut out: Vec<&'static ast::Stmt> = Vec::new();
    fn go(s: &'static ast::Stmt, out: &mut Vec<&'static ast::Stmt>) {
        out.push(s);
        match s {
            ast::Stmt::FunctionDef(f) => f.body.iter().for_each(|x| go(x, out)),
            ast::Stmt::ClassDef(c) => c.body.iter().for_each(|x| go(x, out)),
            ast::Stmt::For(f) => {
                f.body.iter().for_each(|x| go(x, out));
                f.orelse.iter().for_each(|x| go(x, out));
            }
            ast::Stmt::While(w) => {
                w.body.iter().for_each(|x| go(x, out));
                w.orelse.iter().for_each(|x| go(x, out));
            }
            ast::Stmt::If(i) => {
                i.body.iter().for_each(|x| go(x, out));
                i.elif_else_clauses.iter().for_each(|c| c.body.iter().for_each(|x| go(x, out)));
            }
            ast::Stmt::With(w) => w.body.iter().for_each(|x| go(x, out)),
            ast::Stmt::Try(t) => {
                t.body.iter().for_each(|x| go(x, out));
                for h in &t.handlers {
                    let ast::ExceptHandler::ExceptHandler(h) = h;
                    h.body.iter().for_each(|x| go(x, out));
                }
                t.orelse.iter().for_each(|x| go(x, out));
                t.finalbody.iter().for_each(|x| go(x, out));
            }
            ast::Stmt::Match(m) => m.cases.iter().for_each(|c| c.body.iter().for_each(|x| go(x, out))),
            _ => {}
        }
    }
    body.iter().for_each(|x| go(x, &mut out));
    out
}
