//! Inert code: a function body that runs a bounded handful of steps, making functions and
//! storing attributes (a `functools.wraps` decorator), told from its text (`inert`). A call of
//! it runs wherever it is called (`calls.rs`), also past a library entry's allowance or a run on
//! demand's work; a module's definition whose decorators are all
//! *plain* inert runs them when first read (`deferred.rs`).

use ruff_python_ast::{self as ast, Expr, Stmt};
use rustc_hash::FxHashSet;

use crate::intern::{s, StrId};

/// A call an inert body makes (`Inert`): its callee by the names it is called through (`a.b.c`
/// as `[a, b, c]`), and per argument whether it is one of the body's own values (a parameter
/// or a function it defines).
pub struct InertCall {
    pub callee: Vec<StrId>,
    pub own_args: Vec<bool>,
}

/// What an *inert* body does (`inert`): the calls it makes, the names it reads that it does not
/// bind (globals, cells, builtins), and whether it is *plain*: one positional parameter, every
/// attribute it reads one every function has (`__name__`, `__doc__`, …), every `return` giving
/// back one of its own values, and there is one. A plain body given a function, its calls
/// aside, cannot raise and gives back a function.
pub struct Inert {
    pub calls: Vec<InertCall>,
    pub free: Vec<StrId>,
    pub plain: bool,
}

/// The attributes every function has (what a plain inert body may read of one).
const FUNCTION_ATTRS: &[&str] = &["__name__", "__qualname__", "__doc__", "__module__", "__dict__"];

/// Whether a function's body is *inert*, and what it does then (`Inert`). An inert body (a
/// nested function's body aside: it runs when that function is called) is straight-line: a
/// docstring, `pass`, nested `def`s (their decorators calls, their defaults simple),
/// assignments of simple values to its locals and to attributes of its own values (its
/// parameters, the functions it defines), calls, a `return`. A simple value is a literal, a
/// name, an attribute of one of its own values (a function's attributes run no code), or a call
/// whose callee is a name or a dotted name and whose arguments are simple. Run, such a body does
/// a bounded handful of steps; what its calls do is for the caller to tell from what the names
/// hold.
pub fn inert(params: &ast::Parameters, body: &'static [Stmt]) -> Option<Inert> {
    let mut own: Vec<&str> = params.posonlyargs.iter().chain(params.args.iter()).chain(params.kwonlyargs.iter()).map(|p| p.parameter.name.id.as_str()).collect();
    own.extend(params.vararg.iter().chain(params.kwarg.iter()).map(|p| p.name.id.as_str()));
    let one_param = params.posonlyargs.len() + params.args.len() == 1 && params.kwonlyargs.is_empty() && params.vararg.is_none() && params.kwarg.is_none() && params.args.iter().chain(params.posonlyargs.iter()).all(|p| p.default.is_none());
    // (the names it binds: its parameters, its functions, its locals)
    let mut bound: FxHashSet<&str> = own.iter().copied().collect();
    for st in body {
        match st {
            Stmt::FunctionDef(f) => drop(bound.insert(f.name.id.as_str())),
            Stmt::Assign(a) => a.targets.iter().for_each(|t| {
                if let Expr::Name(n) = t {
                    bound.insert(n.id.as_str());
                }
            }),
            _ => {}
        }
    }
    let mut sc = Scan { calls: Vec::new(), free: Vec::new(), plain: one_param, bound };
    let mut returns = false;
    for st in body {
        let ok = match st {
            Stmt::Pass(_) => true,
            Stmt::Expr(x) => matches!(&*x.value, Expr::StringLiteral(_)) || matches!(&*x.value, Expr::Call(c) if sc.call(c, &own)),
            Stmt::FunctionDef(f) => {
                let decorators = f.decorator_list.iter().all(|d| match &d.expression {
                    Expr::Call(c) => sc.call(c, &own),
                    e => sc.chain(e, &mut Vec::new()),
                });
                // (a decorator without a call: its callee, called with the function)
                for d in f.decorator_list.iter().filter(|d| !matches!(d.expression, Expr::Call(_))) {
                    let mut callee = Vec::new();
                    sc.chain(&d.expression, &mut callee);
                    sc.calls.push(InertCall { callee, own_args: vec![true] });
                }
                let p = &f.parameters;
                let defaults = p.posonlyargs.iter().chain(p.args.iter()).chain(p.kwonlyargs.iter()).filter_map(|a| a.default.as_deref()).all(|d| sc.simple(d, &own));
                own.push(f.name.id.as_str());
                decorators && defaults && f.type_params.is_none()
            }
            Stmt::Assign(a) => {
                // (a local rebound: no longer a parameter's or a function's value)
                let targets = a.targets.iter().all(|t| match t {
                    Expr::Name(n) => {
                        own.retain(|o| *o != n.id.as_str());
                        true
                    }
                    Expr::Attribute(x) => matches!(&*x.value, Expr::Name(n) if own.contains(&n.id.as_str())),
                    _ => false,
                });
                targets && sc.simple(&a.value, &own)
            }
            Stmt::Return(r) => {
                returns = true;
                sc.plain &= matches!(r.value.as_deref(), Some(Expr::Name(n)) if own.contains(&n.id.as_str()));
                r.value.as_deref().is_none_or(|v| sc.simple(v, &own))
            }
            _ => false,
        };
        if !ok {
            return None;
        }
    }
    Some(Inert { calls: sc.calls, free: sc.free, plain: sc.plain && returns })
}

/// The scan of an inert body (`inert`): what it found so far, and the names the body binds.
struct Scan<'b> {
    calls: Vec<InertCall>,
    free: Vec<StrId>,
    plain: bool,
    bound: FxHashSet<&'b str>,
}
impl<'b> Scan<'b> {
    fn name(&mut self, n: &str) {
        let id = s(n);
        if !self.bound.contains(n) && !self.free.contains(&id) {
            self.free.push(id);
        }
    }
    fn chain(&mut self, e: &Expr, out: &mut Vec<StrId>) -> bool {
        match e {
            Expr::Name(n) => {
                self.name(n.id.as_str());
                out.push(s(n.id.as_str()));
                true
            }
            Expr::Attribute(a) => self.chain(&a.value, out) && {
                out.push(s(a.attr.as_str()));
                true
            },
            _ => false,
        }
    }
    fn simple(&mut self, e: &Expr, own: &[&str]) -> bool {
        match e {
            Expr::StringLiteral(_) | Expr::NumberLiteral(_) | Expr::BooleanLiteral(_) | Expr::NoneLiteral(_) => true,
            Expr::Name(n) => {
                self.name(n.id.as_str());
                true
            }
            Expr::Attribute(a) => match &*a.value {
                Expr::Name(n) if own.contains(&n.id.as_str()) => {
                    self.plain &= FUNCTION_ATTRS.contains(&a.attr.as_str());
                    true
                }
                _ => false,
            },
            Expr::Call(c) => self.call(c, own),
            _ => false,
        }
    }
    fn call(&mut self, c: &ast::ExprCall, own: &[&str]) -> bool {
        let mut callee = Vec::new();
        if !self.chain(&c.func, &mut callee) || c.arguments.args.iter().any(|a| matches!(a, Expr::Starred(_))) || c.arguments.keywords.iter().any(|k| k.arg.is_none()) {
            return false;
        }
        let args: Vec<&Expr> = c.arguments.args.iter().chain(c.arguments.keywords.iter().map(|k| &k.value)).collect();
        let own_args = args.iter().map(|a| matches!(a, Expr::Name(n) if own.contains(&n.id.as_str()))).collect();
        self.calls.push(InertCall { callee, own_args });
        args.into_iter().all(|a| self.simple(a, own))
    }
}
