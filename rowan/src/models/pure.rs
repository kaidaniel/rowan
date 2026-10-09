//! Calls on known literals computed exactly (`models::pyvalues`): builtins, `math`,
//! `operator`, `statistics` and `str` methods whose arguments are all literals. Anything
//! `pyvalues` declines (or that would raise) falls through to the ordinary model.

use crate::domain::{join_values, KeyVal, Ns, ObjId, Value};
use crate::intern::{self, KeyId, KeySet};
use crate::interp::{Analyzer, Call};
use crate::models::pyvalues::{self, PyResult};
use crate::state::{Kind, State};

/// A literal key for a value: a literal, or a tuple (a list where `as_seq`) of literals.
fn lit_of(an: &Analyzer, st: &State, v: &Value, as_seq: bool) -> Option<KeyId> {
    if v.pts.is_empty() {
        return v.kv.as_lit();
    }
    if !v.kv.is_bot() {
        return None;
    }
    let o: ObjId = v.single()?;
    match an.kind(o) {
        Kind::Tuple => an.literal_tuple(o, st, 0),
        Kind::List if as_seq => {
            let n = an.obj(o).literal_len?;
            let items: Option<Vec<KeyId>> = (0..n)
                .map(|i| {
                    let x = st.slot(o, Ns::Pos, intern::key_int(i as i64));
                    x.kv.as_lit().filter(|_| x.pts.is_empty())
                })
                .collect();
            Some(intern::key_tuple(items?))
        }
        _ => None,
    }
}

/// The literal choices of a value: one literal, or each of several (a loop variable).
fn choices(v: &Value) -> Option<Vec<KeyId>> {
    if !v.pts.is_empty() {
        return None;
    }
    match &v.kv {
        KeyVal::Lit(k) => Some(vec![*k]),
        KeyVal::Lits(ks) if ks.len() <= 8 => Some(ks.iter().collect()),
        _ => None,
    }
}

fn result_value(an: &mut Analyzer, st: &mut State, c: &Call, r: PyResult) -> Value {
    match r {
        PyResult::Key(k) => key_value(an, st, c, k),
        PyResult::List(ks) => {
            let vals: Vec<Value> = ks.iter().map(|k| Value::lit(*k)).collect();
            let kv = if ks.is_empty() { KeyVal::Lits(KeySet::new()) } else { KeyVal::lits(KeySet::from_iter(ks.iter().copied())) };
            an.alloc_positional(Kind::List, &c.node, Some(vals), Value::new(), kv, st)
        }
    }
}

/// A result key as a value: a tuple key becomes a tuple object (positions), others literals.
fn key_value(an: &mut Analyzer, st: &mut State, c: &Call, k: KeyId) -> Value {
    let items = intern::with_key(k, |kk| match kk {
        intern::Key::Tuple(xs) => Some(xs.clone()),
        _ => None,
    });
    match items {
        Some(xs) => {
            let vals: Vec<Value> = xs.iter().map(|k| key_value(an, st, c, *k)).collect();
            an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), crate::domain::DATA, st)
        }
        None => Value::lit(k),
    }
}

const BUILTINS: &[&str] = &["abs", "round", "pow", "divmod", "min", "max", "int", "float", "str", "bool", "chr", "ord", "hex", "oct", "bin", "len", "repr", "ascii", "format"];
const SEQ_OK: &[&str] = &["min", "max", "len"];

/// Some(value) when the call is on literals and computed exactly.
pub fn try_pure(an: &mut Analyzer, st: &mut State, c: &Call) -> Option<Value> {
    if crate::interp::disabled("pure") || !c.star_args.is_empty() || !c.star_kwargs.is_empty() {
        return None;
    }
    let q = c.qual.as_str();
    let (family, name) = q.rsplit_once('.')?;
    let as_seq = SEQ_OK.contains(&name) || family == "statistics" || (family == "str" && name == "join");
    let args: Option<Vec<KeyId>> = c.args.iter().map(|a| lit_of(an, st, a, as_seq)).collect();
    let kwargs: Option<Vec<(String, KeyId)>> = c.kwargs.iter().map(|(k, v)| lit_of(an, st, v, false).map(|x| (intern::str_get(*k).to_string(), x))).collect();
    match family {
        "builtins" if BUILTINS.contains(&name) && !c.args.is_empty() => {
            let (args, kwargs) = (args?, kwargs?);
            let kw: Vec<(&str, KeyId)> = kwargs.iter().map(|(k, v)| (k.as_str(), *v)).collect();
            pyvalues::builtin(name, &args, &kw).map(|k| key_value(an, st, c, k))
        }
        "math" => {
            // (torch programs: math results stay the tensor model's numbers)
            if !c.kwargs.is_empty() || an.torch_on {
                return None;
            }
            pyvalues::math_call(name, &args?).map(Value::lit)
        }
        "operator" => {
            if !c.kwargs.is_empty() {
                return None;
            }
            pyvalues::operator_call(name, &args?).map(|k| key_value(an, st, c, k))
        }
        "statistics" => {
            let a = args?;
            let vals = intern::with_key(*a.first()?, |kk| match kk {
                intern::Key::Tuple(xs) => Some(xs.clone()),
                _ => None,
            })?;
            pyvalues::statistics_call(name, &vals).map(Value::lit)
        }
        "str" if name == "format_map" => {
            // `fmt.format_map(mapping)`: a closed mapping of literals
            let recv = c.recv.clone()?;
            let text = intern::key_as_str(recv.kv.as_lit().filter(|_| recv.pts.is_empty())?)?;
            let m = c.args.first()?.single().filter(|o| an.kind(*o) == Kind::Dict)?;
            let row = st.row(m, Ns::Item);
            if row.open() || row.present != row.definite {
                return None;
            }
            let entries: Option<Vec<(String, KeyId)>> = row.present.iter().map(|k| {
                let v = st.slot(m, Ns::Item, k);
                Some((intern::key_as_str(k)?.to_string(), v.kv.as_lit().filter(|_| v.pts.is_empty())?))
            }).collect();
            pyvalues::str_format(&text, &[], &entries?).map(|s0| Value::lit(intern::key_str_lit(&s0)))
        }
        "str" => {
            // a str method on a literal receiver (each of several, joined)
            let recv = c.recv.clone()?;
            let rs = choices(&recv)?;
            let (args, kwargs) = (args?, kwargs?);
            let kw: Vec<(&str, KeyId)> = kwargs.iter().map(|(k, v)| (k.as_str(), *v)).collect();
            let mut outs: Vec<Value> = Vec::new();
            for r in rs {
                let text = intern::key_as_str(r)?;
                let res = pyvalues::str_method(&text, name, &args, &kw)?;
                outs.push(result_value(an, st, c, res));
            }
            Some(join_values(outs))
        }
        _ => None,
    }
}
