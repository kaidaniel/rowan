//! Builtin exceptions: the classes and their bases, exception objects with their `args`,
//! their `str()` / `repr()`.

use std::cell::RefCell;
use std::rc::Rc;

use rustc_hash::FxHashMap;

use crate::domain::{Ns, Row, Value, DATA};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit};
use crate::interp::{Analyzer, ExcNames, Payload};
use crate::state::{Kind, State};

/// Builtin (and modelled library) exception classes and their bases.
pub const EXC_BASES: &[(&str, &str)] = &[
    ("Exception", "BaseException"), ("SystemExit", "BaseException"), ("KeyboardInterrupt", "BaseException"), ("GeneratorExit", "BaseException"),
    ("BaseExceptionGroup", "BaseException"),
    ("ArithmeticError", "Exception"), ("ZeroDivisionError", "ArithmeticError"), ("OverflowError", "ArithmeticError"), ("FloatingPointError", "ArithmeticError"),
    ("LookupError", "Exception"), ("KeyError", "LookupError"), ("IndexError", "LookupError"),
    ("ValueError", "Exception"), ("UnicodeError", "ValueError"), ("UnicodeDecodeError", "UnicodeError"), ("UnicodeEncodeError", "UnicodeError"),
    ("UnicodeTranslateError", "UnicodeError"),
    ("TypeError", "Exception"), ("AttributeError", "Exception"), ("NameError", "Exception"), ("UnboundLocalError", "NameError"),
    ("RuntimeError", "Exception"), ("NotImplementedError", "RuntimeError"), ("RecursionError", "RuntimeError"), ("AssertionError", "Exception"),
    ("ImportError", "Exception"), ("ModuleNotFoundError", "ImportError"), ("EOFError", "Exception"), ("MemoryError", "Exception"),
    ("StopIteration", "Exception"), ("StopAsyncIteration", "Exception"), ("SyntaxError", "Exception"), ("IndentationError", "SyntaxError"),
    ("SystemError", "Exception"), ("BufferError", "Exception"), ("ReferenceError", "Exception"), ("Warning", "Exception"),
    ("OSError", "Exception"), ("IOError", "Exception"), ("EnvironmentError", "Exception"),
    ("FileNotFoundError", "OSError"), ("PermissionError", "OSError"), ("TimeoutError", "OSError"), ("ConnectionError", "OSError"),
    ("BlockingIOError", "OSError"), ("ChildProcessError", "OSError"), ("FileExistsError", "OSError"), ("InterruptedError", "OSError"),
    ("IsADirectoryError", "OSError"), ("NotADirectoryError", "OSError"), ("ProcessLookupError", "OSError"),
    ("BrokenPipeError", "ConnectionError"), ("ConnectionAbortedError", "ConnectionError"), ("ConnectionRefusedError", "ConnectionError"),
    ("ConnectionResetError", "ConnectionError"),
    // (library exceptions the models raise)
    ("JSONDecodeError", "ValueError"), ("ValidationError", "ValueError"), ("FrozenInstanceError", "AttributeError"),
    // (module-qualified: a bare `Error` names different classes in different modules)
    ("configparser.Error", "Exception"), ("configparser.NoSectionError", "configparser.Error"), ("configparser.NoOptionError", "configparser.Error"),
    ("configparser.DuplicateSectionError", "configparser.Error"), ("configparser.DuplicateOptionError", "configparser.Error"),
    ("configparser.InterpolationError", "configparser.Error"), ("configparser.ParsingError", "configparser.Error"),
    ("configparser.MissingSectionHeaderError", "configparser.ParsingError"),
];

/// The names of a builtin exception class and its bases (`IOError` is `OSError`), computed
/// once per name.
pub fn builtin_exc_names(name: &str) -> ExcNames {
    thread_local! {
        static CACHE: RefCell<FxHashMap<String, ExcNames>> = RefCell::new(FxHashMap::default());
    }
    if let Some(e) = CACHE.with(|c| c.borrow().get(name).cloned()) {
        return e;
    }
    let (names, full) = builtin_exc_names_uncached(name);
    let e: ExcNames = (Rc::new(names), full);
    CACHE.with(|c| c.borrow_mut().insert(name.to_string(), e.clone()));
    e
}

fn builtin_exc_names_uncached(name: &str) -> (Vec<String>, bool) {
    // a module-qualified class of the table: its chain, by the classes' own names
    if EXC_BASES.iter().any(|(c, _)| *c == name && c.contains('.')) {
        let mut out: Vec<String> = Vec::new();
        let mut n = name.to_string();
        while n.contains('.') {
            out.push(n.rsplit('.').next().unwrap_or("").to_string());
            match EXC_BASES.iter().find(|(c, _)| *c == n) {
                Some((_, b)) => n = b.to_string(),
                None => return (out, false),
            }
        }
        let (rest, full) = builtin_exc_names_uncached(&n);
        out.extend(rest);
        return (out, full);
    }
    let mut n = name.rsplit('.').next().unwrap_or(name).to_string();
    let mut out: Vec<String> = Vec::new();
    loop {
        if n == "IOError" || n == "EnvironmentError" || n == "OSError" {
            out.extend(["OSError", "IOError", "EnvironmentError"].iter().map(|s| s.to_string()));
            n = "Exception".into();
            continue;
        }
        out.push(n.clone());
        if n == "BaseException" {
            return (out, true);
        }
        match EXC_BASES.iter().find(|(c, _)| *c == n) {
            Some((_, b)) => n = b.to_string(),
            None => return (out, false),
        }
    }
}

/// A builtin exception (`KeyError(key)`, …) as a value: open attributes, known `args`.
pub fn exception_object(an: &mut Analyzer, cls_name: &str, args: Vec<Value>, node: &Node, state: &mut State) -> Value {
    let t = an.alloc_positional(Kind::Tuple, node, Some(args), Value::new(), DATA, state);
    let row = Row::with_source(crate::source::Source::new(cls_name, crate::source::SourceKind::Data));
    let oid = an.alloc_at(Kind::Namespace, node, state, &format!("{}@L{}", cls_name, node.line), Payload::None, Some(row), Ns::Attr, None, None);
    let strong = an.heap.obj(oid).single;
    an.store_key(oid, Ns::Attr, &Value::lit(key_str_lit("args")), &t, strong, node, state);
    an.m.exc_objs.insert(oid, cls_name.to_string());
    Value::obj(oid)
}

/// `str(e)` / `repr(e)` of exception objects with known args.
pub fn exc_text(an: &Analyzer, v: &Value, repr: bool, state: &State) -> Option<Value> {
    if v.pts.is_empty() || !v.kv.is_bot() {
        return None;
    }
    let mut out = Vec::new();
    for o in v.pts.iter() {
        let name = an.m.exc_objs.get(&o)?;
        let t = state.slot(o, Ns::Attr, key_str_lit("args"));
        let tid = t.single()?;
        let n = an.heap.llen(tid, state)?;
        let mut parts = Vec::new();
        for i in 0..n {
            let a = state.slot(tid, Ns::Pos, intern::key_int(i as i64));
            parts.push(a.kv.as_lit().filter(|_| a.pts.is_empty())?);
        }
        let reprs: Vec<String> = parts.iter().map(|k| intern::key_repr(*k)).collect();
        let s = if repr {
            format!("{}({})", name, reprs.join(", "))
        } else if parts.len() == 1 {
            // (a KeyError shows its key's repr)
            if name == "KeyError" { reprs[0].clone() } else { intern::key_py_str(parts[0]) }
        } else if parts.is_empty() {
            String::new()
        } else if parts.len() == 1 {
            reprs[0].clone()
        } else {
            format!("({})", reprs.join(", "))
        };
        out.push(Value::lit(key_str_lit(&s)));
    }
    Some(crate::domain::join_values(out))
}

/// `super().__init__(…)` reaching a builtin exception base: the instance's `args`, and its
/// `str()` / `repr()` as the base class gives them.
pub fn m_excinit(an: &mut crate::interp::Analyzer, st: &mut State, c: &crate::interp::Call) -> Value {
    let recv = c.recv.clone().unwrap_or_default();
    for o in recv.pts.iter().filter(|o| an.heap.kind(*o) == Kind::Instance).collect::<Vec<_>>() {
        let name = an.heap.instance_class(o).and_then(|cl| an.heap.class_info_opt(cl)).map(|ci| ci.name.clone()).unwrap_or_else(|| "Exception".into());
        let t = an.alloc_positional(Kind::Tuple, &c.node, Some(c.args.clone()), Value::new(), DATA, st);
        let strong = recv.pts.len() == 1 && an.heap.obj(o).single;
        an.store_key(o, Ns::Attr, &Value::lit(key_str_lit("args")), &t, strong, &c.node, st);
        an.m.exc_objs.insert(o, name);
    }
    Value::none()
}

/// The class names a handler for `caught` catches: those and their subclasses in the table.
pub fn expand_caught(mut caught: rustc_hash::FxHashSet<String>) -> rustc_hash::FxHashSet<String> {
    let subs: Vec<String> = EXC_BASES.iter().filter(|(c, _)| !c.contains('.') && builtin_exc_names(c).0.iter().any(|n| caught.contains(n))).map(|(c, _)| c.to_string()).collect();
    caught.extend(subs);
    caught
}
