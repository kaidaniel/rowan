//! Tests of a function's own parameters on a path: in `if admin: row["role"]` the function
//! reads `role` of `row` only on calls where `admin` holds (`ReqInfo::only_if`, reported "only
//! if admin"): a relation between two parameters. `State::path` holds the tests every path
//! passed (a join keeps those of both sides); a requirement takes those of its parameter's
//! function when it is recorded, and a join keeps the tests both sides' readers passed.
//!
//! A test qualifies when it says the same at the `if` as at the call: it reads only parameters
//! the function never rebinds (nor a nested function), and nothing changed what it reads of
//! them (`k in p`: `k` not stored into, deleted from or changed by unknown code; a truth or `==`
//! test: an unknown object nothing wrote since). A call starts without its function's earlier
//! tests (another call's arguments: recursion) and gives the caller's back at its end.

use ruff_python_ast as ast;

use super::Analyzer;
use crate::domain::{Ns, Value};
use crate::frontend;
use crate::intern::{self, s, StrId};
use crate::state::State;

/// A condition longer than this is not shown (a requirement is then "sometimes").
const MAX_WHEN: usize = 60;

impl Analyzer {
    /// The Python texts of a test over the running function's parameters as passed, and of its
    /// negation (see the module comment).
    pub(super) fn param_test(&mut self, test: &'static ast::Expr, pre: &mut State) -> Option<(String, String)> {
        let foid = self.frame().func?;
        let fi = self.heap.func_info_opt(foid)?;
        let params: Vec<String> = crate::scopes::all_params(fi.node.params()?).iter().map(|p| p.name.id.to_string()).collect();
        let mi = self.heap.module_info(fi.module);
        let scope = mi.scopes.get(fi.scope);
        let fixed = |n: &str| params.iter().any(|p| p == n) && !scope.binds.contains_key(n) && !scope.cells.contains(n);
        if !self.test_fixed(test, &fixed, pre) {
            return None;
        }
        let pos = norm(&frontend::text(test));
        let neg = negate(test);
        (pos.len() <= MAX_WHEN && neg.len() <= MAX_WHEN).then_some((pos, neg))
    }

    fn test_fixed(&mut self, e: &'static ast::Expr, fixed: &dyn Fn(&str) -> bool, pre: &mut State) -> bool {
        let lit = |e: &ast::Expr| matches!(e, ast::Expr::StringLiteral(_) | ast::Expr::NumberLiteral(_) | ast::Expr::BooleanLiteral(_) | ast::Expr::NoneLiteral(_));
        match e {
            ast::Expr::Name(n) => fixed(n.id.as_str()) && {
                let v = self.dep_value(pre, s(n.id.as_str()));
                unwritten(&v, pre, None)
            },
            ast::Expr::UnaryOp(u) if u.op == ast::UnaryOp::Not => self.test_fixed(&u.operand, fixed, pre),
            ast::Expr::BoolOp(b) => b.values.iter().all(|x| self.test_fixed(x, fixed, pre)),
            ast::Expr::Compare(c) if c.ops.len() == 1 => {
                let (l, r) = (&*c.left, &c.comparators[0]);
                let name = |e: &ast::Expr| match e {
                    ast::Expr::Name(n) if fixed(n.id.as_str()) => Some(s(n.id.as_str())),
                    _ => None,
                };
                match c.ops[0] {
                    // (identity: a name never rebound is the object it was)
                    ast::CmpOp::Is | ast::CmpOp::IsNot => (name(l).is_some() || lit(l)) && (name(r).is_some() || lit(r)),
                    ast::CmpOp::Eq | ast::CmpOp::NotEq => {
                        let side = |a: &'static ast::Expr, b: &'static ast::Expr, me: &mut Self, pre: &mut State| name(a).is_some() && lit(b) && me.test_fixed(a, fixed, pre);
                        side(l, r, self, pre) || side(r, l, self, pre)
                    }
                    ast::CmpOp::In | ast::CmpOp::NotIn => {
                        let (ast::Expr::StringLiteral(k), Some(n)) = (l, name(r)) else { return false };
                        let key = intern::key(intern::Key::Str(k.value.to_str().into()));
                        let v = self.dep_value(pre, n);
                        unwritten(&v, pre, Some(key))
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// The tests a branch on `test` adds to its two sides' paths.
    pub(super) fn branch_tests(&mut self, test: &'static ast::Expr, pre: &mut State) -> Option<(StrId, StrId, StrId)> {
        let (pos, neg) = self.param_test(test, pre)?;
        let fi = self.heap.func_info_opt(self.frame().func?)?;
        Some((s(&format!("param {}.", fi.py_qualname())), s(&pos), s(&neg)))
    }
}

/// `st`'s path with one more test passed.
pub(super) fn add_test(st: &mut State, owner: StrId, test: StrId) {
    if st.reachable && !st.path.contains(&(owner, test)) {
        let mut p: Vec<(StrId, StrId)> = st.path.to_vec();
        p.push((owner, test));
        st.path = std::sync::Arc::new(p);
    }
}

/// What a test reads of a value is as it was at the call: an unknown object (a parameter's)
/// nothing stored into, deleted from or handed to unknown code since (`key`: only that key).
fn unwritten(v: &Value, st: &State, key: Option<intern::KeyId>) -> bool {
    v.pts.iter().all(|o| {
        [Ns::Item, Ns::Attr, Ns::Pos, Ns::Elem].iter().all(|ns| match st.rows.get(&(o, *ns)) {
            None => true,
            Some(r) => match key {
                Some(k) => *ns != Ns::Item || (!r.stored.contains(k) && !r.absent.contains(k) && r.mutators.is_empty()),
                None => r.stored.is_empty() && r.absent.is_empty() && r.mutators.is_empty(),
            },
        })
    }) && (key.is_none() || !v.pts.is_empty())
}

fn norm(t: &str) -> String {
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The Python text of `not test`, as one would write it (`k not in d`, `x is None`).
fn negate(test: &ast::Expr) -> String {
    match test {
        ast::Expr::UnaryOp(u) if u.op == ast::UnaryOp::Not => norm(&frontend::text(&*u.operand)),
        ast::Expr::Name(_) => format!("not {}", norm(&frontend::text(test))),
        ast::Expr::Compare(c) if c.ops.len() == 1 => {
            let op = match c.ops[0] {
                ast::CmpOp::In => "not in",
                ast::CmpOp::NotIn => "in",
                ast::CmpOp::Is => "is not",
                ast::CmpOp::IsNot => "is",
                ast::CmpOp::Eq => "!=",
                ast::CmpOp::NotEq => "==",
                _ => return format!("not ({})", norm(&frontend::text(test))),
            };
            format!("{} {} {}", norm(&frontend::text(&*c.left)), op, norm(&frontend::text(&c.comparators[0])))
        }
        _ => format!("not ({})", norm(&frontend::text(test))),
    }
}
