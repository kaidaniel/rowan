//! PyTorch model (TORCH_PLAN.md): tensors carry symbolic shapes (`shape.rs`), torch functions,
//! tensor methods, `nn` layers and `nn.Module` subclasses compute them; operations check
//! their conditions: decided false → a shape error (the op raises), undecided → a
//! requirement recorded at the line and learned as a fact on the path.

use indexmap::IndexMap;

use crate::domain::{join_values, KeyVal, Ns, Value, DATA};
use crate::frontend::Node;
use crate::intern::{self, key_int, key_str_lit, s, KeyId, StrId, STAR};
use crate::interp::{Analyzer, Call, Payload};
use crate::shape::{self as sh, Cons, DimId, RankRel, Rest, Shape};
use rustc_hash::FxHashMap;
use crate::state::{Kind, State};

pub const SIZE_PREFIX: &str = "\u{1}S";

#[derive(Clone, Debug)]
pub struct ShapeNote {
    pub line: u32,
    pub chain: Vec<u32>,
    pub entry: bool,
    pub text: String,
    /// the requirement and how to print it (the hover renders it with its dim names)
    pub cons: Option<(Cons, sh::PrintCtx)>,
}

// ---------------------------------------------------------------------------
// values
// ---------------------------------------------------------------------------

fn label(v: &Value) -> Option<std::sync::Arc<str>> {
    match &v.kv {
        KeyVal::Data(l) if *l != intern::EMPTY_STR => Some(intern::str_get(*l)),
        _ => None,
    }
}

pub fn tensor_value(s: Shape) -> Value {
    Value::data(&sh::tensor_label(sh::shape_id(sh::canon(s))))
}

pub fn bool_tensor_value(s: Shape) -> Value {
    Value::data(&sh::bool_tensor_label(sh::shape_id(sh::canon(s))))
}

pub fn dim_value(d: DimId) -> Value {
    match sh::as_lit(d) {
        Some(n) => Value::lit(key_int(n)),
        None => Value::data(&sh::dim_label(d)),
    }
}

pub fn num_value() -> Value {
    Value::data(sh::NUM_LABEL)
}

pub fn size_label_value(s: &Shape) -> Value {
    Value::data(&format!("{}{}", SIZE_PREFIX, sh::shape_id(s.clone()).0))
}

/// The shape a tensor value has (None: not a tensor), normalised under the path's facts.
pub fn tensor_shape(v: &Value) -> Option<Shape> {
    let l = label(v)?;
    sh::parse_tensor_label(&l).map(sh::shape_get)
}

pub fn is_tensor(v: &Value) -> bool {
    v.pts.is_empty() && tensor_shape(v).is_some()
}

pub fn size_shape(v: &Value) -> Option<Shape> {
    let l = label(v)?;
    l.strip_prefix(SIZE_PREFIX)?.parse().ok().map(|i| sh::shape_get(sh::ShapeId(i)))
}

fn expr_sym(l: &str) -> Option<sh::Sym> {
    let rest = l.strip_prefix("expr:")?;
    let (scope, text) = rest.split_once(':')?;
    Some(sh::sym(scope.parse().ok()?, text))
}

impl Analyzer {
    /// The symbol of an unknown object (parameter, attribute read, unknown call result).
    pub fn t_sym_of_obj(&self, o: u32) -> Option<sh::Sym> {
        if !matches!(self.kind(o), Kind::Ph | Kind::Unknown) {
            return None;
        }
        if let Some(sy) = self.t_var_names.get(&o) {
            return Some(*sy);
        }
        // the result of an unknown call that no variable names: nothing to name its dims by
        if self.kind(o) == Kind::Unknown && self.origins.contains_key(&o) && self.torch_on {
            return None;
        }
        let text = self.symbols.get(&o)?;
        let scope = self.symbol_scope.get(&o).copied().unwrap_or(0);
        Some(sh::sym(scope, text))
    }

    /// Shape of a value used as a tensor: a tensor's shape, `[]` for numbers, the shape
    /// symbol of a named unknown, `[*]` for an unnamed unknown. None: surely not a tensor.
    pub fn t_shape(&self, v: &Value, st: &State) -> Option<Shape> {
        let mut acc: Option<Shape> = None;
        let tf = st.tf.clone();
        let mut add = |s: Shape| {
            let s = sh::canon(sh::norm_shape(&s, &tf));
            acc = Some(match acc.take() {
                None => s,
                Some(a) => sh::join_shapes(&a, &s),
            })
        };
        match &v.kv {
            KeyVal::Data(l) => {
                let t = intern::str_get(*l);
                if let Some(id) = sh::parse_tensor_label(&t) {
                    add(sh::shape_get(id));
                } else if t.starts_with(sh::DIM_PREFIX) || &*t == sh::NUM_LABEL || t.starts_with(QUOT_PREFIX) {
                    add(Shape::known(vec![]));
                } else if t.starts_with(SIZE_PREFIX) {
                } else if let Some(sy) = expr_sym(&t) {
                    add(Shape::of_sym(sy));
                } else {
                    add(Shape::any());
                }
            }
            KeyVal::Lit(_) | KeyVal::Lits(_) => {
                if crate::domain::kv_literals(&v.kv).iter().any(intern::key_is_number) || crate::domain::kv_literals(&v.kv).iter().any(|k| matches!(intern::key_get(k), intern::Key::Bool(_))) {
                    add(Shape::known(vec![]));
                }
            }
            KeyVal::Param(p) => match self.t_sym_of_obj(*p) {
                Some(sy) => add(Shape::of_sym(sy)),
                None => add(Shape::any()),
            },
            _ => {}
        }
        for o in v.pts.iter() {
            match self.kind(o) {
                Kind::Ph | Kind::Unknown => match self.t_sym_of_obj(o) {
                    Some(sy) => add(Shape::of_sym(sy)),
                    None => add(Shape::any()),
                },
                _ => {}
            }
        }
        acc.map(|s| sh::norm_shape(&s, &st.tf))
    }

    /// An int-valued argument as a dim (None: not an int).
    pub fn t_dim(&self, v: &Value, st: &State) -> Option<DimId> {
        let mut out: Option<DimId> = None;
        let mut put = |d: DimId| {
            out = Some(match out {
                None => d,
                Some(x) => sh::join_dim(x, d),
            })
        };
        match &v.kv {
            KeyVal::Lit(k) => {
                if let intern::Key::Int(i) = intern::key_get(*k) {
                    put(sh::lit(i));
                } else if let intern::Key::Bool(b) = intern::key_get(*k) {
                    put(sh::lit(b as i64));
                } else {
                    return None;
                }
            }
            KeyVal::Lits(ks) => {
                for k in ks.iter() {
                    match intern::key_get(k) {
                        intern::Key::Int(i) => put(sh::lit(i)),
                        intern::Key::None => {}
                        _ => return None,
                    }
                }
            }
            KeyVal::Data(l) => {
                let t = intern::str_get(*l);
                if let Some(d) = sh::parse_dim_label(&t) {
                    put(d);
                } else if let Some(d) = sh::dual_dim(&t) {
                    put(d);
                } else if let Some(sy) = expr_sym(&t) {
                    put(sh::named(sy));
                } else if t.starts_with(sh::TENSOR_PREFIX) || t.starts_with(SIZE_PREFIX) || &*t == sh::NUM_LABEL || t.starts_with(QUOT_PREFIX) {
                    return None;
                } else {
                    put(sh::unknown());
                }
            }
            KeyVal::Param(p) => put(self.t_sym_of_obj(*p).map(sh::named).unwrap_or_else(sh::unknown)),
            _ => {}
        }
        for o in v.pts.iter() {
            match self.kind(o) {
                Kind::Ph | Kind::Unknown => put(self.t_sym_of_obj(o).map(sh::named).unwrap_or_else(sh::unknown)),
                _ => return None,
            }
        }
        out.map(|d| sh::norm(d, &st.tf))
    }

    pub fn t_int(&self, v: &Value) -> Option<i64> {
        if !v.pts.is_empty() {
            return None;
        }
        v.kv.as_lit().and_then(|k| match intern::key_get(k) {
            intern::Key::Int(i) => Some(i),
            intern::Key::Bool(b) => Some(b as i64),
            _ => None,
        })
    }

    pub fn t_is_none(&self, v: &Value) -> bool {
        v.pts.is_empty() && v.kv.as_lit().map(|k| matches!(intern::key_get(k), intern::Key::None)).unwrap_or(false)
    }

    pub fn t_bool(&self, v: &Value) -> Option<bool> {
        if !v.pts.is_empty() {
            return None;
        }
        v.kv.as_lit().and_then(|k| match intern::key_get(k) {
            intern::Key::Bool(b) => Some(b),
            intern::Key::Int(i) => Some(i != 0),
            intern::Key::None => Some(false),
            _ => None,
        })
    }

    pub fn t_str(&self, v: &Value) -> Option<String> {
        if !v.pts.is_empty() {
            return None;
        }
        v.kv.as_lit().and_then(intern::key_as_str).map(|x| x.to_string())
    }

    /// Positions of a tuple / list value (None: not a sequence with known positions).
    pub fn t_seq(&self, v: &Value, st: &State) -> Option<Vec<Value>> {
        let o = v.single()?;
        if !matches!(self.kind(o), Kind::Tuple | Kind::List) {
            return None;
        }
        let n = self.obj(o).literal_len?;
        Some((0..n).map(|i| st.slot(o, Ns::Pos, key_int(i as i64))).collect())
    }

    /// A shape given as a sequence of ints (tuple / list / torch.Size / unknown-rank size).
    pub fn t_shape_arg(&self, v: &Value, st: &State) -> Option<Shape> {
        if let Some(s) = size_shape(v) {
            return Some(sh::norm_shape(&s, &st.tf));
        }
        if let Some(items) = self.t_seq(v, st) {
            let mut parts: Vec<DimId> = Vec::new();
            for it in items {
                parts.push(self.t_dim(&it, st)?);
            }
            return Some(Shape::known(parts));
        }
        if let Some(d) = self.t_dim(v, st) {
            return Some(Shape::known(vec![d]));
        }
        None
    }

    /// Sizes given as varargs (`zeros(2, 3)`, `zeros(*s)`, `zeros(3, *x.shape)`) or one
    /// sequence (`zeros((2, 3))`, `zeros(x.shape)`).
    pub fn t_sizes(&self, args: &[Value], star: &[Value], st: &State) -> Option<Shape> {
        if args.len() == 1 && star.is_empty() {
            if self.t_seq(&args[0], st).is_some() || size_shape(&args[0]).is_some() {
                return self.t_shape_arg(&args[0], st);
            }
        }
        let mut s = Shape::known(vec![]);
        for a in args {
            let d = self.t_dim(a, st)?;
            s = append_dims(&s, &[d]);
        }
        for a in star {
            let part = self.t_shape_arg(a, st).unwrap_or_else(Shape::any);
            s = concat(&s, &part);
        }
        Some(s)
    }

    pub fn t_size_value(&mut self, s: &Shape, node: &Node, st: &mut State) -> Value {
        match s.dims() {
            Some(d) => {
                let vals: Vec<Value> = d.iter().map(|x| dim_value(*x)).collect();
                let v = self.alloc_positional(Kind::Tuple, node, Some(vals), Value::new(), DATA, st);
                if let Some(o) = v.single() {
                    self.obj_mut(o).payload = Payload::Str("torch.Size".into());
                }
                v
            }
            None => size_label_value(s),
        }
    }

    pub fn t_tuple(&mut self, vals: Vec<Value>, node: &Node, st: &mut State, marker: Option<&str>) -> Value {
        self.t_tuple_role(vals, node, st, marker, "t")
    }

    /// A result tuple; `role` keeps tuples made at one call distinct (`out, (h, c)`).
    pub fn t_tuple_role(&mut self, vals: Vec<Value>, node: &Node, st: &mut State, marker: Option<&str>, role: &str) -> Value {
        let site = format!("{}#{}", crate::interp::site_string(node), role);
        let oid = self.alloc(Kind::Tuple, &site, st, &format!("tuple@L{}", node.line), Payload::None, None, Ns::Item, None, None);
        let single = self.obj(oid).single;
        let acc = join_values(vals.clone());
        st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(acc.pts.clone(), DATA), single);
        st.elems.insert(oid, acc.kv.clone());
        if single {
            self.obj_mut(oid).literal_len = Some(vals.len());
            for (i, v) in vals.iter().enumerate() {
                st.set_slot(oid, Ns::Pos, key_int(i as i64), v.clone(), true);
            }
        } else {
            self.obj_mut(oid).literal_len = None;
        }
        if let Some(m) = marker {
            self.obj_mut(oid).payload = Payload::Str(m.into());
        }
        Value::obj(oid)
    }

    /// A tuple of unknown length whose elements are like `elem` (`count`: its length as a dim).
    pub fn t_tuple_n(&mut self, elem: Value, node: &Node, st: &mut State) -> Value {
        self.t_tuple_count(elem, None, node, st)
    }

    pub fn t_tuple_count(&mut self, elem: Value, count: Option<DimId>, node: &Node, st: &mut State) -> Value {
        let site = format!("{}#n", crate::interp::site_string(node));
        let oid = self.alloc(Kind::Tuple, &site, st, &format!("tuple@L{}", node.line), Payload::None, None, Ns::Item, None, None);
        let single = self.obj(oid).single;
        st.set_slot(oid, Ns::Elem, STAR, elem.clone(), single);
        st.elems.insert(oid, elem.kv.clone());
        self.obj_mut(oid).literal_len = None;
        self.obj_mut(oid).payload = Payload::Str(match count {
            Some(d) => format!("torch.n:{}", d.0),
            None => "torch.n".into(),
        });
        Value::obj(oid)
    }

    /// The length of a tuple made by `unbind` / `chunk` with a symbolic count.
    pub fn t_tuple_len(&self, v: &Value) -> Option<DimId> {
        let o = v.single()?;
        match &self.obj(o).payload {
            Payload::Str(q) => q.strip_prefix("torch.n:").and_then(|d| d.parse().ok()).map(DimId),
            _ => None,
        }
    }

    // ---------------------------------------------------------------- notes
    fn t_note_ctx(&self) -> (Vec<u32>, bool) {
        let entry = self.entry_run.is_some();
        let mut chain = self.call_chain.clone();
        if entry && !chain.is_empty() {
            chain.remove(0);
        }
        (chain, entry)
    }

    /// Run `f` with the printing context of the current function (its symbols, those of
    /// its class's `__init__`, aliases through self attributes).
    pub fn t_print<R>(&self, st: &State, f: impl FnOnce() -> R) -> R {
        let ctx = self.t_print_ctx(st);
        sh::with_ctx(ctx, f)
    }

    pub fn t_print_ctx(&self, st: &State) -> sh::PrintCtx {
        let scope = self.frames.iter().rev().find_map(|fr| fr.func).unwrap_or(0);
        self.t_print_ctx_at(scope, &st.env, st)
    }

    /// The printing context of the run's outermost function (an entry run's function): a
    /// requirement found in a callee is stated in the caller's terms at the call line.
    pub fn t_outer_print_ctx(&self, st: &State) -> sh::PrintCtx {
        let funcs: Vec<u32> = self.frames.iter().filter(|f| f.kind == crate::interp::FrameKind::Function).filter_map(|f| f.func).collect();
        let target = match self.entry_run {
            Some(e) => funcs.iter().position(|f| *f == e),
            None => if funcs.is_empty() { None } else { Some(0) },
        };
        let Some(j) = target else { return self.t_print_ctx(st) };
        let total = funcs.len();
        if j + 1 == total {
            return self.t_print_ctx(st);
        }
        let back = total - 1 - j;
        if back > self.env_stack.len() {
            return self.t_print_ctx(st);
        }
        let env = &self.env_stack[self.env_stack.len() - back];
        self.t_print_ctx_at(funcs[j], env, st)
    }

    pub fn t_print_ctx_at(&self, scope: u32, env: &IndexMap<StrId, Value>, st: &State) -> sh::PrintCtx {
        let mut ctx = sh::PrintCtx { all: false, scope, ..Default::default() };
        if scope != 0 && self.has_obj(scope) && self.kind(scope) == Kind::Function {
            // a closure sees the names of the functions around it
            let mut q = self.func_info(scope).qualname.clone();
            while let Some(i) = q.rfind('.') {
                q.truncate(i);
                if let Some(fs) = self.t_funcs_by_qual.get(&q) {
                    ctx.extra.extend(fs.iter().copied());
                }
            }
            if let Some(cls) = self.func_info(scope).cls {
                for c in self.mro(cls) {
                    if let Some(v) = self.cls_slots.get(&(c, Ns::Attr, key_str_lit("__init__"))) {
                        for f in v.pts.iter() {
                            ctx.extra.push(f);
                        }
                    }
                }
                // aliases: the shortest attribute path from self (through instances, not
                // layer configs) holding an `__init__` value
                if let Some(selfv) = env.get(&s("self")) {
                    let mut queue: std::collections::VecDeque<(u32, String, u32)> = selfv.pts.iter().filter(|o| self.kind(*o) == Kind::Instance).map(|o| (o, "self".to_string(), 0)).collect();
                    let mut seen: rustc_hash::FxHashSet<u32> = rustc_hash::FxHashSet::default();
                    while let Some((o, path, depth)) = queue.pop_front() {
                        if !seen.insert(o) || queue.len() > 256 {
                            continue;
                        }
                        for (k, v) in st.obj_slots(o, Ns::Attr) {
                            let Some(name) = intern::key_as_str(k) else { continue };
                            let p = format!("{}.{}", path, name);
                            if !v.pts.is_empty() && !v.kv.is_bot() {
                                // several possibilities: the joined dim
                                if let Some(d) = self.t_dim(&v, st) {
                                    ctx.dim_alias.entry(d).or_insert_with(|| p.clone());
                                }
                                continue;
                            }
                            if let Some(d) = dim_of_label(&v).or_else(|| label(&v).and_then(|l| sh::dual_dim(&l))) {
                                ctx.dim_alias.entry(d).or_insert_with(|| p.clone());
                                continue;
                            }
                            if let KeyVal::Data(l) = &v.kv {
                                if let Some(sy) = expr_sym(&intern::str_get(*l)) {
                                    ctx.sym_alias.entry(sy).or_insert_with(|| p.clone());
                                }
                            }
                            if v.pts.len() == 1 {
                                let x = v.pts.first().unwrap();
                                if let Some(sy) = self.t_sym_of_obj(x) {
                                    ctx.sym_alias.entry(sy).or_insert_with(|| p.clone());
                                } else if self.kind(x) == Kind::Instance && depth < 3 {
                                    queue.push_back((x, p.clone(), depth + 1));
                                }
                            }
                        }
                    }
                }
            }
        }
        ctx
    }

    pub fn t_require_note(&mut self, node: &Node, text: String, cons: Option<(Cons, sh::PrintCtx)>) {
        let (chain, entry) = self.t_note_ctx();
        let n = ShapeNote { line: node.line, chain, entry, text, cons };
        if !self.shape_reqs.iter().any(|x| x.line == n.line && x.chain == n.chain && x.entry == n.entry && x.text == n.text) {
            self.shape_reqs.push(n);
        }
    }

    pub fn t_error(&mut self, node: &Node, msg: String, st: &mut State) {
        if self.t_quiet {
            return;
        }
        let (chain, entry) = self.t_note_ctx();
        let n = ShapeNote { line: node.line, chain, entry, text: msg, cons: None };
        if !self.shape_errors.iter().any(|x| x.line == n.line && x.chain == n.chain && x.text == n.text) {
            self.shape_errors.push(n);
        }
        self.raise_here("RuntimeError", st);
    }

    /// Check a condition: true → fine; false → shape error (the op raises, the path ends);
    /// undecided → a requirement here, a fact from here on. Returns false on an error.
    pub fn t_require(&mut self, c: Cons, node: &Node, st: &mut State, msg: impl FnOnce() -> String) -> bool {
        if self.t_quiet {
            return true;
        }
        match sh::decide(&c, &st.tf) {
            Some(true) => true,
            Some(false) => {
                let m = self.t_print(st, msg);
                self.t_error(node, m, st);
                false
            }
            None => {
                let ctx = self.t_outer_print_ctx(st);
                let text = sh::with_ctx(ctx.clone(), || sh::cons_text(&c));
                if !text.contains('?') {
                    self.t_require_note(node, text, Some((c.clone(), ctx)));
                }
                let mut f = (*st.tf).clone();
                sh::learn(&mut f, &c);
                st.tf = std::sync::Arc::new(f);
                true
            }
        }
    }

    pub fn t_require_eq(&mut self, a: DimId, b: DimId, node: &Node, st: &mut State, what: &str) -> bool {
        let w = what.to_string();
        self.t_require(Cons::Eq(a, b), node, st, move || format!("{}: {} != {}", w, sh::dim_text(a), sh::dim_text(b)))
    }

    /// The rank of `s` must be in `rel`. Unknown rank: a requirement on the symbol whose
    /// shape is the rest (`x.dim() …`), when expressible.
    pub fn t_require_rank(&mut self, s: &Shape, rel: RankRel, node: &Node, st: &mut State, what: &str) -> bool {
        if self.t_quiet {
            return true;
        }
        let rel_text = |r: &RankRel| match r {
            RankRel::Eq(n) => format!("{} dims", n),
            RankRel::Ge(n) => format!("at least {} dims", n),
            RankRel::In(v) => format!("{} dims", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" or ")),
        };
        if let Some(r) = s.rank() {
            let ok = match &rel {
                RankRel::Eq(n) => r as u32 == *n,
                RankRel::Ge(n) => r as u32 >= *n,
                RankRel::In(v) => v.contains(&(r as u32)),
            };
            if !ok {
                let m = format!("{} needs {}, got {} ({})", what, rel_text(&rel), r, self.t_print(st, || sh::shape_text(s)));
                self.t_error(node, m, st);
            }
            return ok;
        }
        let fixed = s.prefix.len() + s.suffix.len();
        let Some(Rest::Slice(sym, from, back)) = &s.rest else {
            // not expressible; still impossible when the known part is already too long
            let too_long = match &rel {
                RankRel::Eq(n) => fixed as u32 > *n,
                RankRel::In(v) => v.iter().all(|n| fixed as u32 > *n),
                RankRel::Ge(_) => false,
            };
            if too_long {
                let m = format!("{} needs {}, got at least {}", what, rel_text(&rel), fixed);
                self.t_error(node, m, st);
                return false;
            }
            return true;
        };
        // rank(shape) = fixed + rank(sym) - from - back
        let off = *from as i64 + *back as i64 - fixed as i64;
        let tr = |n: u32| -> Option<u32> {
            let k = n as i64 + off;
            if k < (*from + *back) as i64 { None } else { Some(k as u32) }
        };
        let rel2 = match &rel {
            RankRel::Eq(n) => match tr(*n) {
                Some(k) => RankRel::Eq(k),
                None => {
                    let m = format!("{} needs {}, got at least {}", what, rel_text(&rel), fixed);
                    self.t_error(node, m, st);
                    return false;
                }
            },
            RankRel::In(v) => {
                let w: Vec<u32> = v.iter().filter_map(|n| tr(*n)).collect();
                match w.len() {
                    0 => {
                        let m = format!("{} needs {}, got at least {}", what, rel_text(&rel), fixed);
                        self.t_error(node, m, st);
                        return false;
                    }
                    1 => RankRel::Eq(w[0]),
                    _ => RankRel::In(w),
                }
            }
            RankRel::Ge(n) => {
                let k = *n as i64 + off;
                if k <= (*from + *back) as i64 || k < 2 {
                    // implied by the shape itself, or a bound below 2: not recorded
                    return true;
                }
                RankRel::Ge(k as u32)
            }
        };
        let w = what.to_string();
        let rt = rel_text(&rel);
        self.t_require(Cons::Rank(*sym, rel2), node, st, move || format!("{} needs {}", w, rt))
    }

    /// Position `i` of `s` (negative: from the end), the shape with that dim spelled out
    /// (requires enough dims). None when it fails (error reported).
    pub fn t_locate(&mut self, s: &Shape, i: i64, node: &Node, st: &mut State, what: &str) -> Option<(Shape, Pos)> {
        if let Some(r) = s.rank() {
            let j = if i < 0 { r as i64 + i } else { i };
            if j < 0 || j >= r as i64 {
                let m = format!("{}: dim {} out of range for {} dims", what, i, r);
                self.t_error(node, m, st);
                return None;
            }
            let d = s.dims().unwrap();
            return Some((Shape::known(d), Pos::F(j as usize)));
        }
        let need = if i < 0 { (-i) as u32 } else { i as u32 + 1 };
        if !self.t_require_rank(s, RankRel::Ge(need), node, st, what) {
            return None;
        }
        let s = sh::norm_shape(s, &st.tf);
        if s.rank().is_some() {
            return self.t_locate(&s, i, node, st, what);
        }
        if i >= 0 {
            let e = sh::expose_f(&s, i as usize + 1, 0, &st.tf);
            Some((e, Pos::F(i as usize)))
        } else {
            let k = (-i) as usize;
            let e = sh::expose(&s, 0, k);
            let n = e.suffix.len();
            Some((e, Pos::B(n - k)))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pos {
    /// index in the prefix
    F(usize),
    /// index in the suffix
    B(usize),
}

pub fn get_at(s: &Shape, p: Pos) -> DimId {
    match p {
        Pos::F(i) => s.prefix[i],
        Pos::B(i) => s.suffix[i],
    }
}

/// Replace the dim at `p` by `new` (empty: remove it).
pub fn replace_at(s: &Shape, p: Pos, new: &[DimId]) -> Shape {
    let mut s = s.clone();
    match p {
        Pos::F(i) => {
            s.prefix.splice(i..i + 1, new.iter().copied());
        }
        Pos::B(i) => {
            s.suffix.splice(i..i + 1, new.iter().copied());
        }
    }
    s
}

pub fn append_dims(s: &Shape, d: &[DimId]) -> Shape {
    let mut s = s.clone();
    if s.rest.is_none() {
        s.prefix.extend(d.iter().copied());
    } else {
        s.suffix.extend(d.iter().copied());
    }
    s
}

/// Concatenate two shapes' dim lists (sizes `(*a, *b)`).
pub fn concat(a: &Shape, b: &Shape) -> Shape {
    match (&a.rest, &b.rest) {
        (_, None) => append_dims(a, &b.dims().unwrap()),
        (None, Some(_)) => {
            let mut s = b.clone();
            let mut p = a.dims().unwrap();
            p.extend(s.prefix.iter().copied());
            s.prefix = p;
            s
        }
        (Some(_), Some(_)) => {
            let mut s = Shape::any();
            s.prefix = a.prefix.clone();
            s.suffix = b.suffix.clone();
            s
        }
    }
}

pub fn all_dims_known(s: &Shape) -> bool {
    s.rest.is_none()
}

fn kw<'a>(c: &'a Call, name: &str) -> Option<&'a Value> {
    c.kwargs.get(&s(name))
}


#[allow(dead_code)]
fn unused(_: StrId, _: KeyId, _: IndexMap<u8, u8>, _: Vec<Value>) {
    let _ = join_values(vec![]);
    let _ = STAR;
}

// ---------------------------------------------------------------------------
// operators, attributes, indexing
// ---------------------------------------------------------------------------

impl Analyzer {
    /// `a op b` where tensors or symbolic dims are involved (None: not ours).
    pub fn t_binop(&mut self, op: ruff_python_ast::Operator, a: &Value, b: &Value, node: &Node, st: &mut State) -> Option<Value> {
        use ruff_python_ast::Operator as O;
        if op == O::MatMult {
            let sa = self.t_shape(a, st).unwrap_or_else(Shape::any);
            let sb = self.t_shape(b, st).unwrap_or_else(Shape::any);
            return Some(self.t_matmul(&sa, &sb, node, st, "matmul"));
        }
        // sizes: `x.shape[:-1] + (n,)`
        if op == O::Add {
            let size_of = |an: &Analyzer, v: &Value| -> Option<Shape> {
                if let Some(s) = size_shape(v) {
                    return Some(s);
                }
                let o = v.single()?;
                if matches!(an.kind(o), Kind::Tuple) {
                    return an.t_shape_arg(v, st);
                }
                None
            };
            let is_size = |an: &Analyzer, v: &Value| size_shape(v).is_some() || v.single().map(|o| matches!(&an.obj(o).payload, Payload::Str(q) if q == "torch.Size")).unwrap_or(false);
            if (is_size(self, a) || is_size(self, b)) && self.torch_on {
                if let (Some(x), Some(y)) = (size_of(self, a), size_of(self, b)) {
                    let r = concat(&x, &y);
                    return Some(self.t_size_value(&r, node, st));
                }
            }
        }
        if !matches!(op, O::Add | O::Sub | O::Mult | O::FloorDiv | O::Mod | O::Div | O::Pow | O::BitAnd | O::BitOr | O::BitXor) {
            return None;
        }
        #[derive(PartialEq)]
        enum K {
            Tensor,
            Dual,
            Int,
            Num,
            Named,
        }
        let classify = |an: &Analyzer, v: &Value| -> Option<K> {
            // a join of a named unknown and a tensor (a loop variable): the tensor counts
            if !v.pts.is_empty() {
                if let KeyVal::Data(l) = &v.kv {
                    let t = intern::str_get(*l);
                    if t.starts_with(sh::TENSOR_PREFIX) && v.pts.iter().all(|o| an.t_sym_of_obj(o).is_some()) {
                        return Some(if sh::dual_dim(&t).is_some() { K::Dual } else { K::Tensor });
                    }
                }
            }
            if v.pts.is_empty() {
                match &v.kv {
                    KeyVal::Data(l) => {
                        let t = intern::str_get(*l);
                        if t.starts_with(sh::TENSOR_PREFIX) {
                            return Some(if sh::dual_dim(&t).is_some() { K::Dual } else { K::Tensor });
                        }
                        if t.starts_with(sh::DIM_PREFIX) {
                            return Some(K::Int);
                        }
                        if &*t == sh::NUM_LABEL || t.starts_with(QUOT_PREFIX) {
                            return Some(K::Num);
                        }
                        if t.starts_with("expr:") && an.torch_on {
                            return Some(K::Named);
                        }
                        if t.starts_with(SIZE_PREFIX) {
                            return None;
                        }
                        // an unnamed unknown number (a loop counter, `float(...)`)
                        if an.torch_on {
                            return Some(K::Int);
                        }
                        None
                    }
                    KeyVal::Lit(k) => match intern::key_get(*k) {
                        intern::Key::Int(_) | intern::Key::Bool(_) => Some(K::Int),
                        intern::Key::Float(_) => Some(K::Num),
                        _ => None,
                    },
                    KeyVal::Param(p) if an.torch_on && an.t_sym_of_obj(*p).is_some() => Some(K::Named),
                    _ => None,
                }
            } else if an.torch_on && v.kv.is_bot() && v.pts.len() == 1 && an.t_sym_of_obj(v.pts.first().unwrap()).is_some() {
                Some(K::Named)
            } else if an.torch_on && v.kv.is_bot() && !v.pts.is_empty() && v.pts.iter().all(|o| matches!(an.kind(o), Kind::Ph | Kind::Unknown)) {
                // an unknown without a name (an unknown call's result): any shape
                Some(K::Named)
            } else {
                None
            }
        };
        // `32 ** -0.5`: folded
        if op == O::Pow {
            let num = |v: &Value| -> Option<f64> {
                if !v.pts.is_empty() {
                    return None;
                }
                match intern::key_get(v.kv.as_lit()?) {
                    intern::Key::Int(i) => Some(i as f64),
                    intern::Key::Float(b) => Some(f64::from_bits(b)),
                    _ => None,
                }
            };
            if let (Some(x), Some(y)) = (num(a), num(b)) {
                let r = x.powf(y);
                let both_int = matches!(intern::key_get(a.kv.as_lit().unwrap()), intern::Key::Int(_)) && matches!(intern::key_get(b.kv.as_lit().unwrap()), intern::Key::Int(_)) && y >= 0.0;
                if r.is_finite() {
                    return Some(if both_int && r.abs() < 9.0e15 { Value::lit(key_int(r as i64)) } else { Value::lit(intern::key_float(r)) });
                }
            }
        }
        let (ka, kb) = (classify(self, a)?, classify(self, b)?);
        let bitop = matches!(op, O::BitAnd | O::BitOr | O::BitXor);
        if ka == K::Tensor || kb == K::Tensor {
            let sa = if matches!(ka, K::Int | K::Num) { Shape::known(vec![]) } else { self.t_shape(a, st).unwrap_or_else(Shape::any) };
            let sb = if matches!(kb, K::Int | K::Num) { Shape::known(vec![]) } else { self.t_shape(b, st).unwrap_or_else(Shape::any) };
            let r = self.t_broadcast(&sa, &sb, node, st, "elementwise op");
            if bitop {
                let both_bool = [a, b].iter().all(|v| label(v).map(|l| sh::is_bool_label(&l)).unwrap_or(true));
                if both_bool {
                    if let Some(s) = tensor_shape(&r) {
                        return Some(bool_tensor_value(s));
                    }
                }
            }
            return Some(r);
        }
        let small_pow = op == O::Pow && self.t_int(b).map(|k| (0..=8).contains(&k)).unwrap_or(false);
        let int_op = matches!(op, O::Add | O::Sub | O::Mult | O::FloorDiv | O::Mod) || small_pow;
        let dims = |an: &Analyzer| -> Option<DimId> {
            let (x, y) = (an.t_dim(a, st)?, an.t_dim(b, st)?);
            Some(sh::norm(
                match op {
                    O::Add => sh::add(x, y),
                    O::Sub => sh::sub(x, y),
                    O::Mult => sh::mul(x, y),
                    O::FloorDiv => sh::floordiv(x, y),
                    O::Pow => (0..sh::as_lit(y).unwrap_or(0)).fold(sh::lit(1), |acc, _| sh::mul(acc, x)),
                    _ => sh::modulo(x, y),
                },
                &st.tf,
            ))
        };
        if ka == K::Int && kb == K::Int {
            // plain ints are the interpreter's; a symbolic one makes a symbolic dim
            if dim_of_label(a).is_none() && dim_of_label(b).is_none() {
                return None;
            }
            if int_op {
                return Some(dim_value(dims(self)?));
            }
            if bitop {
                return None;
            }
            if op == O::Div {
                if let (Some(d), Some(k)) = (dim_of_label(a), self.t_int(b)) {
                    if k > 0 {
                        return Some(quotient_value(sh::norm(d, &st.tf), k));
                    }
                }
            }
            return Some(num_value());
        }
        if matches!((&ka, &kb), (K::Int | K::Num, K::Int | K::Num)) {
            let special = |v: &Value| label(v).map(|l| l.starts_with('\u{1}')).unwrap_or(false);
            if !special(a) && !special(b) {
                return None;
            }
            return Some(num_value());
        }
        if bitop {
            return None;
        }
        // a named unknown / dual: a tensor (broadcast) or, for ints, the dim
        let scalar = |k: &K| matches!(k, K::Int | K::Num);
        let sa = if scalar(&ka) { Shape::known(vec![]) } else { self.t_shape(a, st).unwrap_or_else(Shape::any) };
        let sb = if scalar(&kb) { Shape::known(vec![]) } else { self.t_shape(b, st).unwrap_or_else(Shape::any) };
        let shape = match sh::broadcast(&sa, &sb, &st.tf) {
            Ok(s) => s,
            Err(_) => return Some(self.t_broadcast(&sa, &sb, node, st, "elementwise op")),
        };
        let sid = sh::shape_id(sh::tidy(shape));
        if int_op && ka != K::Num && kb != K::Num {
            if let Some(d) = dims(self) {
                return Some(Value::data(&sh::dual_label(sid, d)));
            }
        }
        Some(Value::data(&sh::tensor_label(sid)))
    }

    pub fn t_broadcast(&mut self, a: &Shape, b: &Shape, node: &Node, st: &mut State, what: &str) -> Value {
        match sh::broadcast(a, b, &st.tf) {
            Ok(s) => tensor_value(s),
            Err((x, y)) => {
                let m = self.t_print(st, || format!("{}: shapes {} and {} do not broadcast ({} vs {})", what, sh::shape_text(a), sh::shape_text(b), sh::dim_text(x), sh::dim_text(y)));
                self.t_error(node, m, st);
                Value::new()
            }
        }
    }

    /// `a @ b` (also `matmul`).
    pub fn t_matmul(&mut self, a: &Shape, b: &Shape, node: &Node, st: &mut State, what: &str) -> Value {
        let a = sh::norm_shape(a, &st.tf);
        let b = sh::norm_shape(b, &st.tf);
        if a.rank() == Some(0) || b.rank() == Some(0) {
            let m = format!("{}: both operands need at least 1 dim", what);
            self.t_error(node, m, st);
            return Value::new();
        }
        let mut assume: Vec<Cons> = a.assume.clone();
        assume.extend(b.assume.iter().cloned());
        // 1-D cases (known ranks)
        let ra = a.rank();
        let rb = b.rank();
        if ra == Some(1) && rb == Some(1) {
            let (x, y) = (a.prefix[0], b.prefix[0]);
            if !self.t_require_eq(x, y, node, st, what) {
                return Value::new();
            }
            return tensor_value(Shape::known(vec![]).with_assume(&assume));
        }
        // operand views: (batch rest, rows / cols)
        // a: [..., n, k] (or [k] when 1-D); b: [..., k, m] (or [k])
        let a_vec = ra == Some(1);
        let b_vec = rb == Some(1);
        let mut conds: Vec<Cons> = Vec::new();
        let split = |an: &mut Analyzer, s: &Shape, vec: bool, conds: &mut Vec<Cons>| -> Option<(Rest, Option<DimId>, DimId)> {
            // returns (batch, outer dim (rows of a / cols of b), inner dim)
            if vec {
                return Some((Rest::Dims(vec![]), None, s.prefix[0]));
            }
            if s.min_rank() < 2 {
                // assume >= 2 dims
                match &s.rest {
                    Some(Rest::Slice(sym, from, back)) => {
                        let need = 2 - s.min_rank() as u32 + from + back;
                        conds.push(Cons::Rank(*sym, RankRel::Ge(need)));
                    }
                    _ => return None,
                }
            }
            let _ = an;
            let e = sh::expose(s, 0, 2);
            let (batch, last2) = if let Some(d) = e.dims() {
                let n = d.len();
                (Rest::Dims(d[..n - 2].to_vec()), [d[n - 2], d[n - 1]])
            } else {
                let n = e.suffix.len();
                (sh::merge_rest(&e.prefix, e.rest.as_ref().unwrap(), &e.suffix[..n - 2]), [e.suffix[n - 2], e.suffix[n - 1]])
            };
            Some((batch, Some(last2[0]), last2[1]))
        };
        let Some((ba, outer_a, inner_a)) = split(self, &a, a_vec, &mut conds) else { return tensor_value(Shape::any()) };
        let a_conds = conds.clone();
        // for b, the inner dim is the second to last and the outer the last
        let Some((bb, x, y)) = split(self, &b, b_vec, &mut conds) else { return tensor_value(Shape::any()) };
        let (inner_b, outer_b) = if b_vec { (y, None) } else { (x.unwrap(), Some(y)) };
        let b_conds: Vec<Cons> = conds[a_conds.len()..].to_vec();
        // inner dims must agree
        let req = Cons::Eq(inner_a, inner_b);
        let req = if conds.is_empty() { req } else { Cons::If(Box::new(req), conds.clone()) };
        let w = what.to_string();
        let ok = self.t_require(req, node, st, move || format!("{}: inner dims {} and {} differ", w, sh::dim_text(inner_a), sh::dim_text(inner_b)));
        if !ok {
            return Value::new();
        }
        let _ = b_conds;
        // batch dims broadcast
        let batch = match (&ba, &bb) {
            (Rest::Dims(x), Rest::Dims(y)) => {
                match sh::broadcast(&Shape::known(x.clone()), &Shape::known(y.clone()), &st.tf) {
                    Ok(s) => Rest::Dims(s.dims().unwrap()),
                    Err((p, q)) => {
                        let m = self.t_print(st, || format!("{}: batch dims {} and {} do not broadcast", what, sh::dim_text(p), sh::dim_text(q)));
                        self.t_error(node, m, st);
                        return Value::new();
                    }
                }
            }
            (Rest::Dims(x), r) | (r, Rest::Dims(x)) if x.is_empty() => r.clone(),
            _ => {
                let mut v = vec![ba.clone(), bb.clone()];
                v.sort_by_key(sh::rest_text);
                Rest::Bcast(v)
            }
        };
        let mut tail: Vec<DimId> = Vec::new();
        if let Some(o) = outer_a {
            tail.push(o);
        }
        if let Some(o) = outer_b {
            tail.push(o);
        }
        let s = match batch {
            Rest::Dims(d) => {
                let mut p = d;
                p.extend(tail);
                Shape::known(p)
            }
            r => Shape { prefix: vec![], rest: Some(r), suffix: tail, assume: vec![] },
        };
        let s = s.with_assume(&assume).with_assume(&conds);
        tensor_value(sh::norm_shape(&s, &st.tf))
    }

    /// Attribute of a tensor (or of an unknown used as one); None: not ours.
    pub fn t_getattr(&mut self, obj: &Value, name: &str, node: &Node, st: &mut State) -> Option<Value> {
        // `torch.finfo(dt).min` & co.: numbers
        if obj.pts.is_empty() && matches!(&obj.kv, KeyVal::Data(l) if &*intern::str_get(*l) == sh::NUM_LABEL) {
            return Some(num_value());
        }
        // layer objects / return tuples
        if let Some(o) = obj.single() {
            if self.kind(o) == Kind::Dict && matches!(&self.obj(o).payload, Payload::Str(q) if q == "torch.nn.ModuleDict") && !matches!(name, "keys" | "values" | "items" | "get" | "update" | "pop" | "clear") {
                let k = Value::lit(key_str_lit(name));
                return Some(self.load_item(obj, Ns::Item, &k, node, st, true));
            }
            if let Payload::Str(q) = &self.obj(o).payload {
                if q == "torch.return_types" {
                    let i = match name {
                        "values" => 0,
                        "indices" => 1,
                        _ => return None,
                    };
                    return Some(st.slot(o, Ns::Pos, key_int(i)));
                }
            }
            if self.kind(o) == Kind::Instance && self.t_is_module_instance(o) {
                return self.t_module_attr(o, name, node, st);
            }
            // a named module of a Sequential(OrderedDict(...))
            if self.kind(o) == Kind::List && self.t_layer_type(o).is_some() {
                if let Some(v) = st.slot_opt(o, Ns::Attr, key_str_lit(name)) {
                    return Some(v);
                }
            }
        }
        let tensor = is_tensor(obj);
        let unknown_tensorish = !tensor && self.torch_on && obj.pts.iter().all(|o| matches!(self.kind(o), Kind::Ph | Kind::Unknown)) && (!obj.pts.is_empty() || matches!(&obj.kv, KeyVal::Data(l) if intern::str_get(*l).starts_with("expr:")));
        if !tensor && !unknown_tensorish {
            return None;
        }
        let tensor_attr = TENSOR_ATTRS.contains(&name) || TENSOR_METHODS.contains(&name);
        if !tensor_attr {
            return if tensor { Some(Value::data(&format!("L{}", node.line))) } else { None };
        }
        // an unknown that may be a str / list / dict: names those have keep their meaning
        // (`line.split(",")`, `xs.sort()`); only a known tensor gets the tensor method
        if !tensor && (crate::interp::STR_METHOD_NAMES.contains(&name) || crate::interp::LIST_METHOD_NAMES.contains(&name) || crate::interp::DICT_METHOD_NAMES.contains(&name) || crate::interp::SCALAR_EXTRA_NAMES.contains(&name)) {
            // known to be a tensor (shape facts): the tensor method
            if self.t_known_param_shape(obj, st).is_none() {
                if matches!(name, "split" | "sort") {
                    // decided by the arguments at the call
                    return Some(self.method_value(&format!("tensorq.{}", name), obj));
                }
                return None;
            }
        }
        if unknown_tensorish && !TENSOR_ONLY.contains(&name) && obj.pts.iter().any(|o| self.kind(o) == Kind::Ph && self.ph_info(o).cls.get().is_some()) {
            return None;
        }
        // an unknown may be an instance of a user class defining this name
        if unknown_tensorish && self.user_classes.iter().any(|c| self.cls_slots.contains_key(&(*c, Ns::Attr, key_str_lit(name)))) {
            return None;
        }
        let s = self.t_shape(obj, st).unwrap_or_else(Shape::any);
        match name {
            "shape" => return Some(self.t_size_value(&s, node, st)),
            "ndim" => return Some(self.t_rank_value(&s)),
            "T" => {
                return Some(match s.dims() {
                    Some(mut d) => {
                        d.reverse();
                        tensor_value(Shape::known(d))
                    }
                    None => tensor_value(Shape::any()),
                })
            }
            "mT" | "mH" => return Some(self.t_transpose(&s, -2, -1, node, st)),
            "data" | "real" | "grad" => return Some(tensor_value(s)),
            "device" | "dtype" | "requires_grad" | "is_cuda" | "layout" | "grad_fn" => return Some(Value::data(&format!("L{}", node.line))),
            _ => {}
        }
        let recv = tensor_value(s);
        Some(self.method_value(&format!("tensor.{}", name), &recv))
    }

    pub fn t_rank_value(&self, s: &Shape) -> Value {
        match s.rank() {
            Some(r) => Value::lit(key_int(r as i64)),
            None => match &s.rest {
                Some(Rest::Slice(sym, from, back)) if s.prefix.is_empty() && s.suffix.is_empty() && *from == 0 && *back == 0 => dim_value(sh::of_atom(sh::Atom::Named(sh::sym(sh::sym_scope(*sym), &format!("{}.dim()", sh::sym_text(*sym)))))),
                _ => Value::data("rank"),
            },
        }
    }

    /// `x[...]` on a tensor or a size (None: not ours). `idx` is the subscript expression.
    pub fn t_subscript(&mut self, obj: &Value, idx: &'static ruff_python_ast::Expr, node: &Node, st: &mut State) -> Option<Value> {
        use ruff_python_ast::Expr as E;
        // sizes: x.shape[i], x.shape[a:b]
        if let Some(s) = size_shape(obj).map(|s| sh::norm_shape(&s, &st.tf)) {
            return Some(self.t_size_index(&s, idx, node, st));
        }
        if let Some(o) = obj.single() {
            if matches!(&self.obj(o).payload, Payload::Str(q) if q == "torch.Size") && matches!(idx, E::Slice(_)) {
                if let Some(s) = self.t_shape_arg(obj, st) {
                    return Some(self.t_size_index(&s, idx, node, st));
                }
            }
        }
        // a slice of a Sequential / ModuleList: one of the same kind
        if let (Some(o), E::Slice(sl)) = (obj.single(), idx) {
            if let Some(ty) = self.t_layer_type(o).filter(|t| t == "Sequential" || t == "ModuleList") {
                if sl.step.is_none() {
                    if let Some(items) = self.t_seq(obj, st) {
                        let n = items.len() as i64;
                        let bound = |an: &mut Analyzer, e: &'static Option<Box<E>>, d: i64, st: &mut State| -> Option<i64> {
                            match e {
                                None => Some(d),
                                Some(x) => {
                                    let v = an.eval(x, st);
                                    an.t_int(&v).map(|i| if i < 0 { (n + i).max(0) } else { i.min(n) })
                                }
                            }
                        };
                        if let (Some(a), Some(b)) = (bound(self, &sl.lower, 0, st), bound(self, &sl.upper, n, st)) {
                            let part: Vec<Value> = if a < b { items[a as usize..b as usize].to_vec() } else { vec![] };
                            return Some(self.t_list_layer(&ty, Some(part), None, node, st));
                        }
                    }
                }
            }
        }
        // an element of a list of tensors without known positions (built by `append`)
        if let Some(o) = obj.single() {
            if matches!(self.kind(o), Kind::List | Kind::Tuple) && self.obj(o).literal_len.is_none() && self.t_layer_type(o).is_none() && !matches!(idx, E::Slice(_)) {
                let e = self.t_list_elem(o, st);
                if tensor_shape(&e).is_some() {
                    let _ = self.eval(idx, st);
                    return Some(e);
                }
            }
        }
        // an element of a result tuple of unknown length
        if let Some(o) = obj.single() {
            if matches!(&self.obj(o).payload, Payload::Str(q) if q == "torch.n" || q.starts_with("torch.n:")) {
                if !matches!(idx, E::Slice(_)) {
                    let _ = self.eval(idx, st);
                    return Some(st.slot(o, Ns::Elem, STAR));
                }
                return Some(obj.clone());
            }
        }
        let tensor = is_tensor(obj) || (tensor_shape(obj).is_some() && obj.pts.iter().all(|o| matches!(self.kind(o), Kind::Ph | Kind::Unknown)));
        let items: Vec<&'static E> = match idx {
            E::Tuple(t) => t.elts.iter().collect(),
            e => vec![e],
        };
        let named = !tensor && self.torch_on && obj.kv.is_bot() && obj.pts.len() == 1 && obj.pts.iter().all(|o| self.t_sym_of_obj(o).is_some());
        if !tensor && !named {
            return None;
        }
        let simple = |e: &E| matches!(e, E::NumberLiteral(_) | E::Name(_) | E::Attribute(_) | E::NoneLiteral(_) | E::EllipsisLiteral(_) | E::Slice(_) | E::UnaryOp(_) | E::List(_));
        if named && !items.iter().all(|e| simple(e)) {
            return None;
        }
        let mut vals: Vec<Option<Value>> = Vec::new();
        for e in &items {
            vals.push(match e {
                E::Slice(_) | E::NoneLiteral(_) | E::EllipsisLiteral(_) | E::List(_) => None,
                _ => Some(self.eval(e, st)),
            });
        }
        if named {
            let tensorish = matches!(idx, E::Tuple(_)) || items.iter().any(|e| matches!(e, E::NoneLiteral(_) | E::EllipsisLiteral(_)));
            if !tensorish {
                // one index: an int or an index tensor makes it tensor indexing; a slice or
                // anything else may be a list's / dict's
                let ok = match &vals[0] {
                    Some(v) => self.t_int(v).is_some() || is_tensor(v) || dim_of_label(v).is_some(),
                    None => false,
                };
                if !ok {
                    return None;
                }
            }
        }
        let s = self.t_shape(obj, st).unwrap_or_else(Shape::any);
        Some(self.t_index(&s, &items, &vals, node, st))
    }

    fn t_size_index(&mut self, s: &Shape, idx: &'static ruff_python_ast::Expr, node: &Node, st: &mut State) -> Value {
        use ruff_python_ast::Expr as E;
        if let E::Slice(sl) = idx {
            let lo = sl.lower.as_ref().map(|e| self.eval(e, st));
            let hi = sl.upper.as_ref().map(|e| self.eval(e, st));
            let lo = match lo {
                None => Some(0),
                Some(v) => self.t_int(&v),
            };
            let hi = match hi {
                None => None,
                Some(v) => match self.t_int(&v) {
                    Some(i) => Some(i),
                    None => return Value::data("size"),
                },
            };
            if sl.step.is_some() {
                return Value::data("size");
            }
            let Some(lo) = lo else { return Value::data("size") };
            if s.rank().is_none() {
                let need = match hi {
                    Some(h) if h > 0 => h as u32,
                    _ if lo < 0 => (-lo) as u32,
                    _ => 0,
                };
                if need >= 2 && !self.t_require_rank(s, RankRel::Ge(need), node, st, "shape slice") {
                    return Value::new();
                }
            }
            let s = &sh::norm_shape(s, &st.tf);
            let r = slice_shape(s, lo, hi);
            return self.t_size_value(&r, node, st);
        }
        let iv = self.eval(idx, st);
        let Some(i) = self.t_int(&iv) else { return Value::data("size") };
        match self.t_locate(s, i, node, st, "shape index") {
            Some((e, p)) => dim_value(get_at(&e, p)),
            None => Value::new(),
        }
    }

    fn t_index(&mut self, s: &Shape, items: &[&'static ruff_python_ast::Expr], vals: &[Option<Value>], node: &Node, st: &mut State) -> Value {
        use ruff_python_ast::Expr as E;
        let s = sh::norm_shape(s, &st.tf);
        // how many dims each item consumes (None: a bool mask of unknown rank)
        let mut cons: Vec<Option<usize>> = Vec::new();
        for (e, v) in items.iter().zip(vals.iter()) {
            cons.push(match e {
                E::NoneLiteral(_) | E::EllipsisLiteral(_) => Some(0),
                _ => match v {
                    Some(v) if self.t_bool_tensor(v) => {
                        let ms = self.t_shape(v, st).unwrap_or_else(Shape::any);
                        if items.len() == 1 && ms == s {
                            // a mask over the whole tensor: the selected elements
                            return tensor_value(Shape::known(vec![sh::unknown()]));
                        }
                        ms.rank()
                    }
                    _ => Some(1),
                },
            });
        }
        if cons.iter().any(|c| c.is_none()) {
            return tensor_value(Shape::any());
        }
        let ell = items.iter().position(|e| matches!(e, E::EllipsisLiteral(_)));
        let (fi, bi): (Vec<usize>, Vec<usize>) = match ell {
            Some(p) => ((0..p).collect(), (p + 1..items.len()).collect()),
            None => ((0..items.len()).collect(), vec![]),
        };
        let nf: usize = fi.iter().map(|i| cons[*i].unwrap()).sum();
        let nb: usize = bi.iter().map(|i| cons[*i].unwrap()).sum();
        if let Some(r) = s.rank() {
            if nf + nb > r {
                let m = format!("too many indices ({}) for a tensor of {} dims", nf + nb, r);
                self.t_error(node, m, st);
                return Value::new();
            }
        } else if !self.t_require_rank(&s, RankRel::Ge((nf + nb) as u32), node, st, "indexing") {
            return Value::new();
        }
        let s = sh::norm_shape(&s, &st.tf);
        let e = sh::expose(&s, nf, nb);
        let (pre, rest, suf) = match e.dims() {
            Some(d) => {
                let n = d.len();
                (d[..nf].to_vec(), Rest::Dims(d[nf..n - nb].to_vec()), d[n - nb..].to_vec())
            }
            None => {
                let n = e.suffix.len();
                (e.prefix[..nf].to_vec(), sh::merge_rest(&e.prefix[nf..], e.rest.as_ref().unwrap(), &e.suffix[..n - nb]), e.suffix[n - nb..].to_vec())
            }
        };
        let mut adv = 0usize;
        let mut front: Vec<DimId> = Vec::new();
        let mut j = 0;
        for &i in &fi {
            let c = cons[i].unwrap();
            let d: Vec<DimId> = pre[j..j + c].to_vec();
            j += c;
            match self.t_index_one(items[i], vals[i].as_ref(), &d, st, &mut adv) {
                Some(x) => front.extend(x),
                None => return tensor_value(Shape::any()),
            }
        }
        let mut back: Vec<DimId> = Vec::new();
        let mut j = 0;
        for &i in &bi {
            let c = cons[i].unwrap();
            let d: Vec<DimId> = suf[j..j + c].to_vec();
            j += c;
            match self.t_index_one(items[i], vals[i].as_ref(), &d, st, &mut adv) {
                Some(x) => back.extend(x),
                None => return tensor_value(Shape::any()),
            }
        }
        if adv > 1 {
            return tensor_value(Shape::any());
        }
        tensor_value(sh::tidy(Shape { prefix: front, rest: Some(rest), suffix: back, assume: vec![] }))
    }

    /// One index item over the dims it consumes: the dims it produces (None: unknown).
    fn t_index_one(&mut self, e: &'static ruff_python_ast::Expr, v: Option<&Value>, dims: &[DimId], st: &mut State, adv: &mut usize) -> Option<Vec<DimId>> {
        use ruff_python_ast::Expr as E;
        let d = dims.first().copied();
        match e {
            E::NoneLiteral(_) => Some(vec![sh::lit(1)]),
            E::EllipsisLiteral(_) => Some(vec![]),
            E::Slice(sl) => {
                let d = d?;
                if sl.step.is_some() {
                    let stp = self.eval(sl.step.as_ref().unwrap(), st);
                    if self.t_int(&stp) != Some(1) {
                        if let (None, None, Some(k), Some(n)) = (&sl.lower, &sl.upper, self.t_int(&stp), sh::as_lit(d)) {
                            if k > 0 {
                                return Some(vec![sh::lit((n + k - 1) / k)]);
                            }
                        }
                        return Some(vec![sh::unknown()]);
                    }
                }
                let lo = sl.lower.as_ref().map(|x| self.eval(x, st));
                let hi = sl.upper.as_ref().map(|x| self.eval(x, st));
                let lo_i = lo.as_ref().map(|v| self.t_int(v));
                let hi_i = hi.as_ref().map(|v| self.t_int(v));
                Some(vec![match (lo_i, hi_i) {
                    (None, None) => d,
                    // a symbolic bound: `min` when it is surely not negative (a negative
                    // one counts from the end)
                    (None, Some(None)) => match self.t_dim(hi.as_ref().unwrap(), st) {
                        Some(t) if !sh::has_unknown(t) && sh::poly(t).iter().all(|(_, c)| *c > 0) => sh::min(t, d),
                        _ => sh::unknown(),
                    },
                    (None, Some(Some(h))) => {
                        if h >= 0 {
                            sh::min(sh::lit(h), d)
                        } else {
                            match sh::as_lit(d) {
                                Some(n) => sh::lit((n + h).max(0)),
                                None => sh::sub(d, sh::lit(-h)),
                            }
                        }
                    }
                    (Some(Some(l)), None) => {
                        if l >= 0 {
                            match sh::as_lit(d) {
                                Some(n) => sh::lit((n - l).max(0)),
                                None => sh::sub(d, sh::lit(l)),
                            }
                        } else {
                            match sh::as_lit(d) {
                                Some(n) => sh::lit((-l).min(n)),
                                None => sh::min(sh::lit(-l), d),
                            }
                        }
                    }
                    (Some(Some(l)), Some(Some(h))) if l >= 0 && h >= 0 => match sh::as_lit(d) {
                        Some(n) => sh::lit((h.min(n) - l.min(n)).max(0)),
                        None => sh::sub(sh::min(sh::lit(h), d), sh::lit(l)),
                    },
                    (Some(Some(l)), Some(Some(h))) => match sh::as_lit(d) {
                        Some(n) => {
                            let norm = |x: i64| if x < 0 { (n + x).max(0) } else { x.min(n) };
                            sh::lit((norm(h) - norm(l)).max(0))
                        }
                        None => sh::unknown(),
                    },
                    _ => sh::unknown(),
                }])
            }
            E::List(_) => {
                let lv = self.eval(e, st);
                *adv += 1;
                match self.t_seq(&lv, st) {
                    Some(items) => Some(vec![sh::lit(items.len() as i64)]),
                    None => Some(vec![sh::unknown()]),
                }
            }
            _ => {
                let v = v?;
                if let Some(ts) = tensor_shape(v) {
                    *adv += 1;
                    if self.t_bool_tensor(v) {
                        // the mask's dims become one dim: the selected elements
                        return Some(vec![sh::unknown()]);
                    }
                    let ts = sh::norm_shape(&ts, &st.tf);
                    return ts.dims();
                }
                if self.t_int(v).is_some() || dim_of_label(v).is_some() {
                    return Some(vec![]);
                }
                if v.pts.iter().any(|o| matches!(self.kind(o), Kind::List | Kind::Tuple)) {
                    *adv += 1;
                    return Some(vec![sh::unknown()]);
                }
                None
            }
        }
    }

    fn t_bool_tensor(&self, v: &Value) -> bool {
        label(v).map(|l| sh::is_bool_label(&l)).unwrap_or(false)
    }
}

/// `s[lo:hi]` of a size (a dim list).
pub fn slice_shape(s: &Shape, lo: i64, hi: Option<i64>) -> Shape {
    if let Some(d) = s.dims() {
        let n = d.len() as i64;
        let norm = |x: i64| if x < 0 { (n + x).max(0) } else { x.min(n) };
        let (a, b) = (norm(lo), hi.map(norm).unwrap_or(n));
        return Shape::known(if a < b { d[a as usize..b as usize].to_vec() } else { vec![] });
    }
    // unknown rank: a slice of the symbol's shape when the size is one
    if let Some(Rest::Slice(sym, from, back)) = &s.rest {
        if s.prefix.is_empty() && s.suffix.is_empty() {
            match (lo >= 0, hi) {
                (true, None) => return Shape { prefix: vec![], rest: Some(Rest::Slice(*sym, from + lo as u32, *back)), suffix: vec![], assume: vec![] },
                (true, Some(h)) if h < 0 && *back == 0 => return Shape { prefix: vec![], rest: Some(Rest::Slice(*sym, from + lo as u32, (-h) as u32)), suffix: vec![], assume: vec![] },
                (true, Some(h)) if h >= lo && *from == 0 => {
                    // x.shape[:2] → [x.shape[0], x.shape[1]] (requires rank ≥ h)
                    return Shape::known((lo..h).map(|i| sh::sdim(*sym, i as i32)).collect());
                }
                (false, None) if *back == 0 => return Shape::known((0..(-lo)).map(|i| sh::sdim(*sym, (lo + i) as i32)).collect()),
                _ => {}
            }
        }
    }
    Shape::any()
}

pub const TENSOR_ATTRS: &[&str] = &["shape", "ndim", "T", "mT", "mH", "data", "real", "grad", "device", "dtype", "requires_grad", "is_cuda", "layout", "grad_fn"];
/// names that only tensors have (an unknown with a declared class still counts)
pub const TENSOR_ONLY: &[&str] = &["shape", "view", "view_as", "unsqueeze", "squeeze", "permute", "transpose", "contiguous", "masked_fill", "masked_fill_", "softmax", "log_softmax", "numel", "ndim", "mT", "T", "reshape", "flatten", "unflatten", "expand", "expand_as", "chunk", "unbind", "dim", "size"];
pub const TENSOR_METHODS: &[&str] = &[
    "view", "reshape", "view_as", "reshape_as", "expand", "expand_as", "repeat", "repeat_interleave", "permute", "transpose", "swapaxes", "swapdims", "t", "unsqueeze",
    "squeeze", "flatten", "unflatten", "chunk", "split", "tensor_split", "unbind", "narrow", "select", "index_select", "gather", "scatter", "scatter_", "scatter_add", "size", "dim",
    "ndimension", "numel", "nelement", "item", "tolist", "numpy", "contiguous", "clone", "detach", "detach_", "float", "double", "half", "bfloat16", "long", "int", "short", "bool",
    "byte", "char", "to", "cpu", "cuda", "type", "type_as", "requires_grad_", "retain_grad", "pin_memory", "relu", "relu_", "gelu", "sigmoid", "sigmoid_", "tanh", "tanh_", "exp",
    "exp_", "log", "log_", "log1p", "expm1", "sqrt", "rsqrt", "abs", "abs_", "neg", "neg_", "sin", "cos", "tan", "pow", "pow_", "clamp", "clamp_", "clamp_min", "clamp_max", "clip",
    "sign", "floor", "ceil", "round", "trunc", "frac", "square", "reciprocal", "erf", "sigmoid", "softmax", "log_softmax", "cumsum", "cumprod", "cummax", "cummin", "tril", "triu",
    "tril_", "triu_", "masked_fill", "masked_fill_", "fill_", "zero_", "add", "add_", "sub", "sub_", "mul", "mul_", "div", "div_", "true_divide", "floor_divide", "remainder", "fmod",
    "maximum", "minimum", "eq", "ne", "lt", "le", "gt", "ge", "logical_and", "logical_or", "logical_xor", "logical_not", "bitwise_not", "isnan", "isinf", "isfinite", "nan_to_num",
    "copy_", "normal_", "uniform_", "bernoulli", "bernoulli_", "exponential_", "random_", "lerp", "addcmul", "addcdiv", "where", "sum", "mean", "prod", "std", "var", "norm", "logsumexp", "amax",
    "amin", "all", "any", "count_nonzero", "nansum", "nanmean", "median", "mode", "max", "min", "argmax", "argmin", "topk", "kthvalue", "sort", "argsort", "msort", "matmul", "mm", "bmm",
    "mv", "dot", "outer", "new_zeros", "new_ones", "new_full", "new_empty", "new_tensor", "flip", "roll", "nonzero", "masked_select", "unfold", "diag", "diagonal", "trace", "softplus",
    "silu", "elu", "leaky_relu", "hardtanh", "apply_", "add_module", "float_power", "atan2", "fill_diagonal_", "index_fill", "index_fill_", "index_add", "index_add_", "index_copy",
    "squeeze_", "unsqueeze_", "t_", "transpose_", "resize_", "resize_as_", "multinomial", "sign_", "addmm", "baddbmm", "einsum", "cross", "masked_scatter", "bincount", "histc",
];

// ---------------------------------------------------------------------------
// tensor methods / torch functions
// ---------------------------------------------------------------------------

const IDENTITY: &[&str] = &[
    "contiguous", "clone", "detach", "detach_", "float", "double", "half", "bfloat16", "long", "int", "short", "byte", "char", "to", "cpu", "cuda", "type", "requires_grad_",
    "retain_grad", "pin_memory", "relu", "relu_", "gelu", "sigmoid", "sigmoid_", "tanh", "tanh_", "exp", "exp_", "log", "log_", "log1p", "expm1", "sqrt", "rsqrt", "abs", "abs_",
    "neg", "neg_", "sin", "cos", "tan", "clamp", "clamp_", "clamp_min", "clamp_max", "clip", "sign", "sign_", "floor", "ceil", "round", "trunc", "frac", "square", "reciprocal",
    "erf", "fill_", "zero_", "copy_", "normal_", "uniform_", "bernoulli", "bernoulli_", "exponential_", "random_", "nan_to_num", "flip", "roll", "softplus", "silu", "elu",
    "leaky_relu", "hardtanh", "apply_", "fill_diagonal_", "index_fill", "index_fill_", "index_add", "index_add_", "index_copy", "scatter", "scatter_", "scatter_add", "argsort",
    "msort", "bitwise_not", "logical_not", "relu6", "selu", "celu", "mish", "hardswish", "hardsigmoid", "logsigmoid", "softsign", "tanhshrink", "dropout", "dropout_",
    "alpha_dropout", "feature_alpha_dropout", "dropout1d", "dropout2d", "dropout3d", "gumbel_softmax", "rrelu", "glu_", "masked_scatter", "positive", "conj", "angle",
    "deg2rad", "rad2deg", "digamma", "lgamma", "sinh", "cosh", "asin", "acos", "atan", "sigmoid_", "exp2", "log2", "log10", "logit", "i0", "sinc",
];
const BOOL_OPS: &[&str] = &["eq", "ne", "lt", "le", "gt", "ge", "logical_and", "logical_or", "logical_xor", "isnan", "isinf", "isfinite", "greater", "less", "equal_", "isclose", "signbit"];
const BINARY: &[&str] = &["add", "sub", "subtract", "mul", "multiply", "div", "divide", "true_divide", "floor_divide", "remainder", "fmod", "pow", "float_power", "maximum", "minimum", "fmax", "fmin", "atan2", "lerp", "hypot", "xlogy", "copysign", "bitwise_and", "bitwise_or", "bitwise_xor"];
const INPLACE: &[&str] = &["add_", "sub_", "mul_", "div_", "pow_", "lerp_", "clamp_", "masked_fill_", "addcmul_", "addcdiv_"];
const REDUCE: &[&str] = &["sum", "mean", "prod", "std", "var", "norm", "logsumexp", "amax", "amin", "all", "any", "count_nonzero", "nansum", "nanmean"];
const CREATE: &[&str] = &["zeros", "ones", "empty", "rand", "randn", "full"];

impl Analyzer {
    fn t_kw(&self, c: &Call, args: &[Value], i: usize, name: &str) -> Option<Value> {
        args.get(i).cloned().or_else(|| kw(c, name).cloned())
    }

    /// `dim=` argument: None = absent / None; Some(list) of ints; Err = unknown.
    fn t_dims_arg(&self, v: Option<Value>, st: &State) -> Result<Option<Vec<i64>>, ()> {
        let Some(v) = v else { return Ok(None) };
        if self.t_is_none(&v) {
            return Ok(None);
        }
        if let Some(i) = self.t_int(&v) {
            return Ok(Some(vec![i]));
        }
        if let Some(items) = self.t_seq(&v, st) {
            let mut out = Vec::new();
            for it in items {
                out.push(self.t_int(&it).ok_or(())?);
            }
            return Ok(Some(out));
        }
        Err(())
    }

    pub fn t_method(&mut self, name: &str, recv: &Value, args: &[Value], c: &Call, st: &mut State) -> Value {
        let node = c.node;
        let s = self.t_shape(recv, st).unwrap_or_else(Shape::any);
        let a0 = self.t_kw(c, args, 0, "dim");
        let keepdim = kw(c, "keepdim").and_then(|v| self.t_bool(v)).unwrap_or(false);
        if IDENTITY.contains(&name) {
            return tensor_value(s);
        }
        if name == "bool" {
            return bool_tensor_value(s);
        }
        if BOOL_OPS.contains(&name) {
            return match args.first().and_then(|a| self.t_shape(a, st)) {
                Some(o) => match sh::broadcast(&s, &o, &st.tf) {
                    Ok(r) => bool_tensor_value(r),
                    Err(_) => self.t_broadcast(&s, &o, &node, st, name),
                },
                None => bool_tensor_value(s),
            };
        }
        if BINARY.contains(&name) {
            let o = args.first().cloned().or_else(|| kw(c, "other").cloned()).or_else(|| kw(c, "exponent").cloned());
            return match o.and_then(|a| self.t_shape(&a, st)) {
                Some(o) => self.t_broadcast(&s, &o, &node, st, name),
                None => tensor_value(s),
            };
        }
        if INPLACE.contains(&name) {
            let o = args.first().cloned();
            if let Some(os) = o.and_then(|a| self.t_shape(&a, st)) {
                if name.starts_with("masked_fill") || name.starts_with("clamp") || name.starts_with("addc") {
                    // the mask (or bound) broadcasts to the tensor
                }
                match sh::broadcast(&s, &os, &st.tf) {
                    Ok(r) => {
                        let grows = match (r.rank(), s.rank()) {
                            (Some(a), Some(b)) => a > b,
                            _ => false,
                        };
                        let differs = match (r.dims(), s.dims()) {
                            (Some(x), Some(y)) if x.len() == y.len() => x.iter().zip(y.iter()).any(|(p, q)| sh::dim_eq(*p, *q, &st.tf) == Some(false)),
                            _ => false,
                        };
                        if grows || differs {
                            let m = self.t_print(st, || format!("{}: in-place result {} would not fit {}", name, sh::shape_text(&r), sh::shape_text(&s)));
                            self.t_error(&node, m, st);
                            return Value::new();
                        }
                    }
                    Err(_) => return self.t_broadcast(&s, &os, &node, st, name),
                }
            }
            return tensor_value(s);
        }
        match name {
            "masked_fill" | "where" => {
                let m = args.first().and_then(|a| self.t_shape(a, st)).unwrap_or_else(Shape::any);
                let mut r = match sh::broadcast(&s, &m, &st.tf) {
                    Ok(r) => r,
                    Err(_) => return self.t_broadcast(&s, &m, &node, st, name),
                };
                if name == "where" {
                    if let Some(o) = args.get(1).and_then(|a| self.t_shape(a, st)) {
                        match sh::broadcast(&r, &o, &st.tf) {
                            Ok(x) => r = x,
                            Err(_) => return self.t_broadcast(&r, &o, &node, st, name),
                        }
                    }
                }
                return tensor_value(r);
            }
            "size" => {
                return match a0 {
                    None => self.t_size_value(&s, &node, st),
                    Some(v) => match self.t_int(&v) {
                        Some(i) => match self.t_locate(&s, i, &node, st, "size") {
                            Some((e, p)) => dim_value(get_at(&e, p)),
                            None => Value::new(),
                        },
                        None => Value::data("dim"),
                    },
                }
            }
            "dim" | "ndimension" => return self.t_rank_value(&s),
            "numel" | "nelement" => {
                return match s.dims() {
                    Some(d) => dim_value(sh::product(&d)),
                    None => Value::data("numel"),
                }
            }
            "item" => return num_value(),
            "tolist" | "numpy" | "data_ptr" | "storage" | "backward" | "register_hook" => return Value::data(&format!("L{}", node.line)),
            "softmax" | "log_softmax" | "cumsum" | "cumprod" | "softmin" | "logcumsumexp" | "normalize" => {
                if let Ok(Some(ds)) = self.t_dims_arg(a0, st) {
                    for d in ds {
                        if self.t_locate(&s, d, &node, st, name).is_none() {
                            return Value::new();
                        }
                    }
                }
                return tensor_value(sh::norm_shape(&s, &st.tf));
            }
            "cummax" | "cummin" | "sort" => {
                if let Ok(Some(ds)) = self.t_dims_arg(a0, st) {
                    for d in ds {
                        if self.t_locate(&s, d, &node, st, name).is_none() {
                            return Value::new();
                        }
                    }
                }
                let t = tensor_value(sh::norm_shape(&s, &st.tf));
                return self.t_tuple(vec![t.clone(), t], &node, st, Some("torch.return_types"));
            }
            "tril" | "triu" | "tril_" | "triu_" => {
                if !self.t_require_rank(&s, RankRel::Ge(2), &node, st, name) {
                    return Value::new();
                }
                return tensor_value(sh::norm_shape(&s, &st.tf));
            }
            _ => {}
        }
        if REDUCE.contains(&name) {
            let dims = match self.t_dims_arg(a0, st) {
                Ok(d) => d,
                Err(_) => return tensor_value(Shape::any()),
            };
            return match self.t_reduce(&s, dims, keepdim, &node, st, name) {
                Some(r) => tensor_value(r),
                None => Value::new(),
            };
        }
        match name {
            "max" | "min" | "median" | "mode" | "nanmedian" => {
                if let Some(o) = args.first() {
                    if let Some(os) = tensor_shape(o) {
                        return self.t_broadcast(&s, &os, &node, st, name);
                    }
                }
                let dims = match self.t_dims_arg(a0, st) {
                    Ok(d) => d,
                    Err(_) => return tensor_value(Shape::any()),
                };
                if dims.is_none() && name != "mode" {
                    return tensor_value(Shape::known(vec![]));
                }
                let dims = dims.or(Some(vec![-1]));
                return match self.t_reduce(&s, dims, keepdim, &node, st, name) {
                    Some(r) => {
                        let t = tensor_value(r);
                        self.t_tuple(vec![t.clone(), t], &node, st, Some("torch.return_types"))
                    }
                    None => Value::new(),
                };
            }
            "argmax" | "argmin" => {
                let dims = match self.t_dims_arg(a0, st) {
                    Ok(d) => d,
                    Err(_) => return tensor_value(Shape::any()),
                };
                if dims.is_none() {
                    return tensor_value(Shape::known(vec![]));
                }
                return match self.t_reduce(&s, dims, keepdim, &node, st, name) {
                    Some(r) => tensor_value(r),
                    None => Value::new(),
                };
            }
            "topk" | "kthvalue" => {
                let k = self.t_kw(c, args, 0, "k");
                let d = self.t_kw(c, args, 1, "dim").and_then(|v| self.t_int(&v)).unwrap_or(-1);
                let Some((e, p)) = self.t_locate(&s, d, &node, st, name) else { return Value::new() };
                let dd = get_at(&e, p);
                let r = if name == "topk" {
                    let kd = k.and_then(|v| self.t_dim(&v, st)).unwrap_or_else(sh::unknown);
                    if let (Some(kk), Some(n)) = (sh::as_lit(kd), sh::as_lit(dd)) {
                        if kk > n {
                            let m = format!("topk: k = {} is larger than the dim ({})", kk, n);
                            self.t_error(&node, m, st);
                            return Value::new();
                        }
                    }
                    replace_at(&e, p, &[kd])
                } else if keepdim {
                    replace_at(&e, p, &[sh::lit(1)])
                } else {
                    replace_at(&e, p, &[])
                };
                let t = tensor_value(r);
                return self.t_tuple(vec![t.clone(), t], &node, st, Some("torch.return_types"));
            }
            "view" | "reshape" => {
                let mut pos: Vec<Value> = args.to_vec();
                if pos.is_empty() {
                    if let Some(v) = kw(c, "shape").or_else(|| kw(c, "size")) {
                        pos.push(v.clone());
                    }
                }
                let target = if pos.len() == args.len() { self.t_sizes_call(c, &pos, st) } else { self.t_sizes(&pos, &c.star_args, st) };
                let Some(target) = target else { return tensor_value(Shape::any()) };
                return self.t_view(&s, &target, &node, st, name);
            }
            "view_as" | "reshape_as" | "expand_as" | "type_as" => {
                let o = args.first().and_then(|a| self.t_shape(a, st)).unwrap_or_else(Shape::any);
                if name == "type_as" {
                    return tensor_value(s);
                }
                if name == "expand_as" {
                    return self.t_expand(&s, &o, &node, st);
                }
                return self.t_view(&s, &o, &node, st, name);
            }
            "expand" | "broadcast_to" => {
                let Some(target) = self.t_sizes_call(c, args, st) else { return tensor_value(Shape::any()) };
                return self.t_expand(&s, &target, &node, st);
            }
            "repeat" | "tile" => {
                let Some(reps) = self.t_sizes_call(c, args, st) else { return tensor_value(Shape::any()) };
                return self.t_repeat(&s, &reps, &node, st);
            }
            "permute" => {
                let ints: Option<Vec<i64>> = if args.is_empty() {
                    None
                } else if args.len() == 1 { self.t_seq(&args[0], st).map(|v| v.iter().filter_map(|x| self.t_int(x)).collect()).or_else(|| self.t_int(&args[0]).map(|i| vec![i])) } else { args.iter().map(|a| self.t_int(a)).collect() };
                let ints = ints.or_else(|| kw(c, "dims").and_then(|v| self.t_seq(v, st)).map(|v| v.iter().filter_map(|x| self.t_int(x)).collect()));
                let Some(perm) = ints else { return tensor_value(Shape::any()) };
                return self.t_permute(&s, &perm, &node, st);
            }
            "transpose" | "swapaxes" | "swapdims" => {
                let a = self.t_kw(c, args, 0, "dim0").and_then(|v| self.t_int(&v));
                let b = self.t_kw(c, args, 1, "dim1").and_then(|v| self.t_int(&v));
                let (Some(a), Some(b)) = (a, b) else { return tensor_value(Shape::any()) };
                return self.t_transpose(&s, a, b, &node, st);
            }
            "t" | "t_" => {
                return match s.rank() {
                    Some(0) | Some(1) => tensor_value(s),
                    Some(2) => {
                        let mut d = s.dims().unwrap();
                        d.swap(0, 1);
                        tensor_value(Shape::known(d))
                    }
                    Some(r) => {
                        let m = format!("t() expects a tensor with <= 2 dims, got {}", r);
                        self.t_error(&node, m, st);
                        Value::new()
                    }
                    None => tensor_value(Shape::any()),
                };
            }
            "unsqueeze" | "unsqueeze_" => {
                let Some(d) = a0.and_then(|v| self.t_int(&v)) else { return tensor_value(Shape::any()) };
                return self.t_unsqueeze(&s, d, &node, st);
            }
            "squeeze" | "squeeze_" => {
                let dims = match self.t_dims_arg(a0, st) {
                    Ok(d) => d,
                    Err(_) => return tensor_value(Shape::any()),
                };
                return self.t_squeeze(&s, dims, &node, st);
            }
            "flatten" => {
                let a = self.t_kw(c, args, 0, "start_dim").and_then(|v| self.t_int(&v)).unwrap_or(0);
                let b = self.t_kw(c, args, 1, "end_dim").and_then(|v| self.t_int(&v)).unwrap_or(-1);
                return self.t_flatten(&s, a, b, &node, st);
            }
            "unflatten" => {
                let d = self.t_kw(c, args, 0, "dim").and_then(|v| self.t_int(&v));
                let sizes = self.t_kw(c, args, 1, "sizes").and_then(|v| self.t_shape_arg(&v, st));
                let (Some(d), Some(sizes)) = (d, sizes) else { return tensor_value(Shape::any()) };
                return self.t_unflatten(&s, d, &sizes, &node, st);
            }
            "chunk" => {
                let n = self.t_kw(c, args, 0, "chunks");
                let d = self.t_kw(c, args, 1, "dim").and_then(|v| self.t_int(&v)).unwrap_or(0);
                return self.t_chunk(&s, n, d, &node, st);
            }
            "split" | "tensor_split" => {
                let a = self.t_kw(c, args, 0, "split_size_or_sections").or_else(|| kw(c, "split_size").cloned());
                let d = self.t_kw(c, args, 1, "dim").and_then(|v| self.t_int(&v)).unwrap_or(0);
                return self.t_split(&s, a, d, &node, st);
            }
            "unbind" => {
                let d = a0.and_then(|v| self.t_int(&v)).unwrap_or(0);
                return self.t_unbind(&s, d, &node, st);
            }
            "narrow" => {
                let d = self.t_kw(c, args, 0, "dim").and_then(|v| self.t_int(&v));
                let len = self.t_kw(c, args, 2, "length").and_then(|v| self.t_dim(&v, st));
                let (Some(d), Some(len)) = (d, len) else { return tensor_value(Shape::any()) };
                let Some((e, p)) = self.t_locate(&s, d, &node, st, name) else { return Value::new() };
                return tensor_value(replace_at(&e, p, &[len]));
            }
            "select" => {
                let Some(d) = a0.and_then(|v| self.t_int(&v)) else { return tensor_value(Shape::any()) };
                let Some((e, p)) = self.t_locate(&s, d, &node, st, name) else { return Value::new() };
                return tensor_value(replace_at(&e, p, &[]));
            }
            "index_select" => {
                let Some(d) = a0.and_then(|v| self.t_int(&v)) else { return tensor_value(Shape::any()) };
                let idx = self.t_kw(c, args, 1, "index").and_then(|v| self.t_shape(&v, st));
                let Some((e, p)) = self.t_locate(&s, d, &node, st, name) else { return Value::new() };
                let n = idx.and_then(|i| i.dims()).filter(|d| d.len() == 1).map(|d| d[0]).unwrap_or_else(sh::unknown);
                return tensor_value(replace_at(&e, p, &[n]));
            }
            "gather" => {
                return self.t_kw(c, args, 1, "index").and_then(|v| self.t_shape(&v, st)).map(tensor_value).unwrap_or_else(|| tensor_value(Shape::any()));
            }
            "matmul" => {
                let o = args.first().cloned().or_else(|| kw(c, "other").cloned()).and_then(|a| self.t_shape(&a, st)).unwrap_or_else(Shape::any);
                return self.t_matmul(&s, &o, &node, st, "matmul");
            }
            "mm" | "bmm" | "mv" | "dot" | "outer" | "ger" | "inner" => {
                let o = args.first().cloned().or_else(|| kw(c, "mat2").cloned()).or_else(|| kw(c, "vec").cloned()).or_else(|| kw(c, "other").cloned()).and_then(|a| self.t_shape(&a, st)).unwrap_or_else(Shape::any);
                return self.t_mm_family(name, &s, &o, &node, st);
            }
            "new_zeros" | "new_ones" | "new_empty" | "new_full" => {
                let pos: Vec<Value> = if name == "new_full" { args.iter().take(1).cloned().collect() } else { args.to_vec() };
                return self.t_sizes(&pos, &c.star_args, st).map(tensor_value).unwrap_or_else(|| tensor_value(Shape::any()));
            }
            "new_tensor" => return self.t_literal_tensor(args.first(), st),
            "nonzero" => {
                return match s.rank() {
                    Some(r) => tensor_value(Shape::known(vec![sh::unknown(), sh::lit(r as i64)])),
                    None => tensor_value(Shape::known(vec![sh::unknown(), sh::unknown()])),
                }
            }
            "masked_select" => return tensor_value(Shape::known(vec![sh::unknown()])),
            "trace" => return tensor_value(Shape::known(vec![])),
            "multinomial" => {
                let n = self.t_kw(c, args, 0, "num_samples").and_then(|v| self.t_dim(&v, st)).unwrap_or_else(sh::unknown);
                return match s.rank() {
                    Some(1) => tensor_value(Shape::known(vec![n])),
                    Some(2) => tensor_value(Shape::known(vec![s.prefix[0], n])),
                    _ => tensor_value(Shape::any()),
                };
            }
            "einsum" => {}
            _ => {}
        }
        tensor_value(Shape::any())
    }

    pub fn t_reduce(&mut self, s: &Shape, dims: Option<Vec<i64>>, keepdim: bool, node: &Node, st: &mut State, what: &str) -> Option<Shape> {
        let Some(ds) = dims else {
            return Some(if keepdim {
                match s.rank() {
                    Some(r) => Shape::known(vec![sh::lit(1); r]),
                    None => Shape::any(),
                }
            } else {
                Shape::known(vec![])
            });
        };
        if ds.is_empty() {
            return Some(Shape::known(vec![]));
        }
        let all_neg = ds.iter().all(|d| *d < 0);
        let all_pos = ds.iter().all(|d| *d >= 0);
        let s = sh::norm_shape(s, &st.tf);
        if s.rank().is_none() && !all_neg && !all_pos {
            return Some(Shape::any());
        }
        // locate the extreme one (requires enough dims), then work on the exposed shape
        let ext = if all_neg { *ds.iter().min().unwrap() } else { *ds.iter().max().unwrap() };
        let (e, _) = self.t_locate(&s, ext, node, st, what)?;
        let mut positions: Vec<Pos> = Vec::new();
        for d in &ds {
            let p = if let Some(r) = e.rank() {
                let j = if *d < 0 { r as i64 + d } else { *d };
                if j < 0 || j >= r as i64 {
                    let m = format!("{}: dim {} out of range for {} dims", what, d, r);
                    self.t_error(node, m, st);
                    return None;
                }
                Pos::F(j as usize)
            } else if *d < 0 {
                Pos::B((e.suffix.len() as i64 + d) as usize)
            } else {
                Pos::F(*d as usize)
            };
            if !positions.contains(&p) {
                positions.push(p);
            }
        }
        // replace from the highest index down so earlier positions stay valid
        positions.sort_by(|a, b| match (a, b) {
            (Pos::F(x), Pos::F(y)) | (Pos::B(x), Pos::B(y)) => y.cmp(x),
            (Pos::B(_), Pos::F(_)) => std::cmp::Ordering::Less,
            (Pos::F(_), Pos::B(_)) => std::cmp::Ordering::Greater,
        });
        let mut r = if e.rank().is_some() { Shape::known(e.dims().unwrap()) } else { e };
        for p in positions {
            let one = [sh::lit(1)];
            r = replace_at(&r, p, if keepdim { &one[..] } else { &[] });
        }
        Some(r)
    }

    pub fn t_view(&mut self, s: &Shape, target: &Shape, node: &Node, st: &mut State, what: &str) -> Value {
        let s = sh::norm_shape(s, &st.tf);
        let Some(t) = target.dims() else { return tensor_value(target.clone()) };
        let minus: Vec<usize> = t.iter().enumerate().filter(|(_, d)| sh::as_lit(**d) == Some(-1)).map(|(i, _)| i).collect();
        if minus.len() > 1 {
            let m = format!("{}: only one dimension can be inferred", what);
            self.t_error(node, m, st);
            return Value::new();
        }
        let mut out = t.clone();
        if let Some(&i) = minus.first() {
            let others: Vec<DimId> = t.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, d)| *d).collect();
            let prod = sh::product(&others);
            out[i] = match s.dims() {
                Some(d) => {
                    let numel = sh::product(&d);
                    if let (Some(n), Some(p)) = (sh::as_lit(numel), sh::as_lit(prod)) {
                        if p == 0 || n % p != 0 {
                            let m = format!("{}: shape {} is invalid for input of size {}", what, self.t_print(st, || sh::shape_text(target)), n);
                            self.t_error(node, m, st);
                            return Value::new();
                        }
                        sh::lit(n / p)
                    } else {
                        sh::div_exact(numel, prod).filter(|q| !sh::has_unknown(*q)).unwrap_or_else(sh::unknown)
                    }
                }
                None => sh::unknown(),
            };
        } else if let Some(d) = s.dims() {
            if let (Some(n), Some(p)) = (sh::as_lit(sh::product(&d)), sh::as_lit(sh::product(&t))) {
                if n != p {
                    let m = format!("{}: shape {} is invalid for input of size {}", what, self.t_print(st, || sh::shape_text(target)), n);
                    self.t_error(node, m, st);
                    return Value::new();
                }
            }
        }
        tensor_value(Shape::known(out))
    }

    pub fn t_expand(&mut self, s: &Shape, target: &Shape, node: &Node, st: &mut State) -> Value {
        let s = sh::norm_shape(s, &st.tf);
        let Some(t) = target.dims() else { return tensor_value(target.clone()) };
        let n = t.len();
        if let Some(r) = s.rank() {
            if r > n {
                let m = format!("expand: {} sizes for a tensor of {} dims", n, r);
                self.t_error(node, m, st);
                return Value::new();
            }
        }
        let mut out = Vec::new();
        for (i, d) in t.iter().enumerate() {
            let j = (n - i) as i64; // position from the end
            let src = if s.rank().map(|r| j as usize <= r).unwrap_or(j as usize <= s.suffix.len()) { sh::dim_at(&s, -j) } else { None };
            if sh::as_lit(*d) == Some(-1) {
                match src {
                    Some(x) => out.push(x),
                    None => {
                        if s.rank().is_some() {
                            let m = "expand: -1 is not allowed for a new leading dim".to_string();
                            self.t_error(node, m, st);
                            return Value::new();
                        }
                        out.push(sh::unknown());
                    }
                }
                continue;
            }
            if let Some(x) = src {
                if sh::is_one(x, &st.tf) != Some(true) && sh::dim_eq(x, *d, &st.tf) == Some(false) {
                    let m = self.t_print(st, || format!("expand: dim {} cannot expand to {}", sh::dim_text(x), sh::dim_text(*d)));
                    self.t_error(node, m, st);
                    return Value::new();
                }
            }
            out.push(*d);
        }
        tensor_value(Shape::known(out))
    }

    pub fn t_repeat(&mut self, s: &Shape, reps: &Shape, node: &Node, st: &mut State) -> Value {
        let s = sh::norm_shape(s, &st.tf);
        let Some(rp) = reps.dims() else { return tensor_value(Shape::any()) };
        let n = rp.len();
        if let Some(r) = s.rank() {
            if n < r {
                let m = format!("repeat: {} repeat dims for a tensor of {} dims", n, r);
                self.t_error(node, m, st);
                return Value::new();
            }
        }
        let mut out = Vec::new();
        for (i, k) in rp.iter().enumerate() {
            let j = (n - i) as i64;
            match s.rank() {
                Some(r) if j as usize > r => out.push(*k),
                Some(_) => out.push(sh::mul(*k, sh::dim_at(&s, -j).unwrap())),
                None => match (j as usize <= s.suffix.len()).then(|| sh::dim_at(&s, -j)).flatten() {
                    Some(x) => out.push(sh::mul(*k, x)),
                    None => out.push(sh::unknown()),
                },
            }
        }
        tensor_value(Shape::known(out))
    }

    pub fn t_permute(&mut self, s: &Shape, perm: &[i64], node: &Node, st: &mut State) -> Value {
        let n = perm.len();
        if s.rank().is_none() && !self.t_require_rank(s, RankRel::Eq(n as u32), node, st, "permute") {
            return Value::new();
        }
        let s = sh::norm_shape(s, &st.tf);
        let Some(d) = s.dims() else { return tensor_value(Shape::any()) };
        if d.len() != n {
            let m = format!("permute: {} dims given for a tensor of {} dims", n, d.len());
            self.t_error(node, m, st);
            return Value::new();
        }
        let mut out = Vec::new();
        let mut seen = vec![false; n];
        for p in perm {
            let j = if *p < 0 { n as i64 + p } else { *p };
            if j < 0 || j >= n as i64 || seen[j as usize] {
                let m = "permute: not a permutation".to_string();
                self.t_error(node, m, st);
                return Value::new();
            }
            seen[j as usize] = true;
            out.push(d[j as usize]);
        }
        tensor_value(Shape::known(out))
    }

    pub fn t_transpose(&mut self, s: &Shape, a: i64, b: i64, node: &Node, st: &mut State) -> Value {
        let s = sh::norm_shape(s, &st.tf);
        if s.rank().is_none() && (a < 0) != (b < 0) {
            let ext = if a < 0 { a } else { b };
            let ext2 = if a < 0 { b } else { a };
            if self.t_locate(&s, ext, node, st, "transpose").is_none() || self.t_locate(&s, ext2, node, st, "transpose").is_none() {
                return Value::new();
            }
            return tensor_value(Shape::any());
        }
        let ext = if a < 0 { a.min(b) } else { a.max(b) };
        let Some((e, _)) = self.t_locate(&s, ext, node, st, "transpose") else { return Value::new() };
        let pos = |d: i64| -> Pos {
            match e.rank() {
                Some(r) => Pos::F(if d < 0 { (r as i64 + d) as usize } else { d as usize }),
                None if d < 0 => Pos::B((e.suffix.len() as i64 + d) as usize),
                None => Pos::F(d as usize),
            }
        };
        if let Some(r) = e.rank() {
            let other = if ext == a { b } else { a };
            let j = if other < 0 { r as i64 + other } else { other };
            if j < 0 || j >= r as i64 {
                let m = format!("transpose: dim {} out of range for {} dims", other, r);
                self.t_error(node, m, st);
                return Value::new();
            }
        }
        let (pa, pb) = (pos(a), pos(b));
        let e = if e.rank().is_some() { Shape::known(e.dims().unwrap()) } else { e };
        let (da, db) = (get_at(&e, pa), get_at(&e, pb));
        let r = replace_at(&replace_at(&e, pa, &[db]), pb, &[da]);
        tensor_value(r)
    }

    pub fn t_unsqueeze(&mut self, s: &Shape, d: i64, node: &Node, st: &mut State) -> Value {
        let s = sh::norm_shape(s, &st.tf);
        if let Some(r) = s.rank() {
            let r = r as i64;
            if d < -r - 1 || d > r {
                let m = format!("unsqueeze: dim {} out of range for {} dims", d, r);
                self.t_error(node, m, st);
                return Value::new();
            }
            let p = if d < 0 { d + r + 1 } else { d } as usize;
            let mut v = s.dims().unwrap();
            v.insert(p, sh::lit(1));
            return tensor_value(Shape::known(v));
        }
        if d >= 0 {
            if d > 0 && !self.t_require_rank(&s, RankRel::Ge(d as u32), node, st, "unsqueeze") {
                return Value::new();
            }
            let e = sh::expose(&sh::norm_shape(&s, &st.tf), d as usize, 0);
            let mut e = e;
            if e.rank().is_some() {
                return self.t_unsqueeze(&e, d, node, st);
            }
            e.prefix.insert(d as usize, sh::lit(1));
            tensor_value(e)
        } else {
            let k = (-d - 1) as usize;
            if k > 0 && !self.t_require_rank(&s, RankRel::Ge(k as u32), node, st, "unsqueeze") {
                return Value::new();
            }
            let mut e = sh::expose(&sh::norm_shape(&s, &st.tf), 0, k);
            if e.rank().is_some() {
                return self.t_unsqueeze(&e, d, node, st);
            }
            let n = e.suffix.len();
            e.suffix.insert(n - k, sh::lit(1));
            tensor_value(e)
        }
    }

    pub fn t_squeeze(&mut self, s: &Shape, dims: Option<Vec<i64>>, node: &Node, st: &mut State) -> Value {
        let s = sh::norm_shape(s, &st.tf);
        match dims {
            None => {
                let Some(d) = s.dims() else { return tensor_value(Shape::any()) };
                if d.iter().any(|x| sh::is_one(*x, &st.tf).is_none()) {
                    return tensor_value(Shape::any());
                }
                tensor_value(Shape::known(d.into_iter().filter(|x| sh::is_one(*x, &st.tf) != Some(true)).collect()))
            }
            Some(ds) => {
                let mut cur = s.clone();
                let mut ds = ds;
                // highest index first
                if let Some(r) = cur.rank() {
                    for d in ds.iter_mut() {
                        if *d < -(r as i64) || *d >= r as i64 {
                            let m = format!("squeeze: dim {} out of range for {} dims", d, r);
                            self.t_error(node, m, st);
                            return Value::new();
                        }
                        if *d < 0 {
                            *d += r as i64;
                        }
                    }
                    ds.sort();
                    ds.dedup();
                    ds.reverse();
                }
                for d in ds {
                    let Some((e, p)) = self.t_locate(&cur, d, node, st, "squeeze") else { return Value::new() };
                    let x = get_at(&e, p);
                    cur = match sh::is_one(x, &st.tf) {
                        Some(true) => replace_at(&e, p, &[]),
                        Some(false) => e,
                        None => sh::join_shapes(&e, &replace_at(&e, p, &[])),
                    };
                }
                tensor_value(cur)
            }
        }
    }

    pub fn t_flatten(&mut self, s: &Shape, a: i64, b: i64, node: &Node, st: &mut State) -> Value {
        let s = sh::norm_shape(s, &st.tf);
        if let Some(r) = s.rank() {
            if r == 0 {
                return tensor_value(Shape::known(vec![sh::lit(1)]));
            }
            let r = r as i64;
            let (x, y) = (if a < 0 { a + r } else { a }, if b < 0 { b + r } else { b });
            if x < 0 || y >= r || x >= r || y < 0 || x > y {
                let m = format!("flatten: dims ({}, {}) out of range for {} dims", a, b, r);
                self.t_error(node, m, st);
                return Value::new();
            }
            let d = s.dims().unwrap();
            let mut out: Vec<DimId> = d[..x as usize].to_vec();
            out.push(sh::product(&d[x as usize..=y as usize]));
            out.extend(d[y as usize + 1..].iter().copied());
            return tensor_value(Shape::known(out));
        }
        if a >= 0 && b >= 0 {
            let Some((e, _)) = self.t_locate(&s, b, node, st, "flatten") else { return Value::new() };
            if e.rank().is_some() {
                return self.t_flatten(&e, a, b, node, st);
            }
            let mut e = e;
            let prod = sh::product(&e.prefix[a as usize..=b as usize]);
            e.prefix.splice(a as usize..=b as usize, [prod]);
            return tensor_value(e);
        }
        if a < 0 && b < 0 {
            let Some((e, _)) = self.t_locate(&s, a, node, st, "flatten") else { return Value::new() };
            if e.rank().is_some() {
                return self.t_flatten(&e, a, b, node, st);
            }
            let mut e = e;
            let n = e.suffix.len() as i64;
            let (x, y) = ((n + a) as usize, (n + b) as usize);
            let prod = sh::product(&e.suffix[x..=y]);
            e.suffix.splice(x..=y, [prod]);
            return tensor_value(e);
        }
        if a >= 0 {
            // [0..a) kept, the rest one unknown dim
            if a > 0 && !self.t_require_rank(&s, RankRel::Ge(a as u32), node, st, "flatten") {
                return Value::new();
            }
            let e = sh::expose_f(&sh::norm_shape(&s, &st.tf), a as usize, 0, &st.tf);
            if e.prefix.len() < a as usize || e.prefix[..a as usize].iter().any(|d| sh::has_unknown(*d)) && e.suffix.is_empty() && e.rest.is_none() {
                return tensor_value(Shape::any());
            }
            let mut p = e.prefix[..a as usize].to_vec();
            p.push(sh::unknown());
            return tensor_value(Shape::known(p));
        }
        tensor_value(Shape::any())
    }

    pub fn t_unflatten(&mut self, s: &Shape, d: i64, sizes: &Shape, node: &Node, st: &mut State) -> Value {
        let Some(sz) = sizes.dims() else { return tensor_value(Shape::any()) };
        let Some((e, p)) = self.t_locate(s, d, node, st, "unflatten") else { return Value::new() };
        let x = get_at(&e, p);
        let minus: Vec<usize> = sz.iter().enumerate().filter(|(_, v)| sh::as_lit(**v) == Some(-1)).map(|(i, _)| i).collect();
        let mut out = sz.clone();
        if let Some(&i) = minus.first() {
            let others: Vec<DimId> = sz.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, v)| *v).collect();
            out[i] = sh::div_exact(x, sh::product(&others)).unwrap_or_else(sh::unknown);
        } else {
            let prod = sh::product(&sz);
            if !self.t_require_eq(x, prod, node, st, "unflatten") {
                return Value::new();
            }
        }
        let e = sh::norm_shape(&e, &st.tf);
        tensor_value(replace_at(&e, p, &out))
    }

    fn t_pieces(&mut self, e: &Shape, p: Pos, sizes: Vec<DimId>, node: &Node, st: &mut State) -> Value {
        let vals: Vec<Value> = sizes.iter().map(|z| tensor_value(replace_at(e, p, &[*z]))).collect();
        self.t_tuple(vals, node, st, None)
    }

    pub fn t_chunk(&mut self, s: &Shape, n: Option<Value>, d: i64, node: &Node, st: &mut State) -> Value {
        let Some((e, p)) = self.t_locate(s, d, node, st, "chunk") else { return Value::new() };
        let x = get_at(&e, p);
        let n = n.and_then(|v| self.t_int(&v));
        if let Some(n) = n {
            if let Some(len) = sh::as_lit(x) {
                let size = (len + n - 1) / n.max(1);
                let mut sizes = Vec::new();
                let mut left = len;
                while left > 0 && size > 0 {
                    sizes.push(sh::lit(size.min(left)));
                    left -= size;
                }
                return self.t_pieces(&e, p, sizes, node, st);
            }
            if let Some(q) = sh::div_exact(x, sh::lit(n)) {
                return self.t_pieces(&e, p, vec![q; n as usize], node, st);
            }
        }
        let elem = tensor_value(replace_at(&e, p, &[sh::unknown()]));
        self.t_tuple_n(elem, node, st)
    }

    pub fn t_split(&mut self, s: &Shape, a: Option<Value>, d: i64, node: &Node, st: &mut State) -> Value {
        let Some((e, p)) = self.t_locate(s, d, node, st, "split") else { return Value::new() };
        let x = get_at(&e, p);
        let Some(a) = a else { return tensor_value(Shape::any()) };
        if let Some(items) = self.t_seq(&a, st) {
            let sizes: Vec<DimId> = items.iter().map(|v| self.t_dim(v, st).unwrap_or_else(sh::unknown)).collect();
            let total = sizes.iter().fold(sh::lit(0), |acc, z| sh::add(acc, *z));
            if !self.t_require_eq(x, total, node, st, "split sizes") {
                return Value::new();
            }
            return self.t_pieces(&e, p, sizes, node, st);
        }
        if let Some(size) = self.t_dim(&a, st) {
            if let (Some(len), Some(k)) = (sh::as_lit(x), sh::as_lit(size)) {
                if k > 0 {
                    let mut sizes = Vec::new();
                    let mut left = len;
                    while left > 0 {
                        sizes.push(sh::lit(k.min(left)));
                        left -= k;
                    }
                    return self.t_pieces(&e, p, sizes, node, st);
                }
            }
            if let Some(q) = sh::div_exact(x, size).and_then(sh::as_lit) {
                if q > 0 && q <= 64 {
                    return self.t_pieces(&e, p, vec![size; q as usize], node, st);
                }
            }
        }
        let elem = tensor_value(replace_at(&e, p, &[sh::unknown()]));
        self.t_tuple_n(elem, node, st)
    }

    pub fn t_unbind(&mut self, s: &Shape, d: i64, node: &Node, st: &mut State) -> Value {
        let Some((e, p)) = self.t_locate(s, d, node, st, "unbind") else { return Value::new() };
        let x = get_at(&e, p);
        let elem = tensor_value(replace_at(&e, p, &[]));
        match sh::as_lit(x) {
            Some(n) if (0..=64).contains(&n) => {
                let vals = vec![elem; n as usize];
                self.t_tuple(vals, node, st, None)
            }
            _ => self.t_tuple_count(elem, Some(x), node, st),
        }
    }

    pub fn t_mm_family(&mut self, name: &str, a: &Shape, b: &Shape, node: &Node, st: &mut State) -> Value {
        let (ra, rb) = match name {
            "mm" => (2, 2),
            "bmm" => (3, 3),
            "mv" => (2, 1),
            "dot" | "inner" => (1, 1),
            _ => (1, 1), // outer / ger
        };
        if !self.t_require_rank(a, RankRel::Eq(ra), node, st, name) || !self.t_require_rank(b, RankRel::Eq(rb), node, st, name) {
            return Value::new();
        }
        let a = sh::norm_shape(a, &st.tf);
        let b = sh::norm_shape(b, &st.tf);
        let (Some(da), Some(db)) = (a.dims(), b.dims()) else { return tensor_value(Shape::any()) };
        match name {
            "outer" | "ger" => tensor_value(Shape::known(vec![da[0], db[0]])),
            "bmm" => {
                let (x, y) = (da[0], db[0]);
                if !self.t_require(Cons::Eq(x, y), node, st, move || format!("bmm: batch {} vs {}", sh::dim_text(x), sh::dim_text(y))) {
                    return Value::new();
                }
                if !self.t_require_eq(da[2], db[1], node, st, "bmm") {
                    return Value::new();
                }
                tensor_value(sh::norm_shape(&Shape::known(vec![da[0], da[1], db[2]]), &st.tf))
            }
            _ => self.t_matmul(&a, &b, node, st, name),
        }
    }

    fn t_literal_tensor(&mut self, data: Option<&Value>, st: &State) -> Value {
        fn lit_shape(an: &Analyzer, v: &Value, st: &State, depth: u32) -> Option<Vec<i64>> {
            if depth > 8 {
                return None;
            }
            if v.pts.is_empty() {
                return match &v.kv {
                    KeyVal::Lit(k) if intern::key_is_number(*k) || matches!(intern::key_get(*k), intern::Key::Bool(_)) => Some(vec![]),
                    _ => None,
                };
            }
            let items = an.t_seq(v, st)?;
            if items.is_empty() {
                return Some(vec![0]);
            }
            let inner = lit_shape(an, &items[0], st, depth + 1)?;
            let mut out = vec![items.len() as i64];
            out.extend(inner);
            Some(out)
        }
        match data.and_then(|d| lit_shape(self, d, st, 0)) {
            Some(d) => tensor_value(Shape::known(d.into_iter().map(sh::lit).collect())),
            None => match data.and_then(|d| tensor_shape(d)) {
                Some(s) => tensor_value(s),
                None => tensor_value(Shape::any()),
            },
        }
    }

    /// `torch.<name>(…)` (not `nn`).
    pub fn t_torch_fn(&mut self, name: &str, c: &Call, st: &mut State) -> Value {
        let node = c.node;
        let args = c.args.clone();
        if CREATE.contains(&name) || matches!(name, "zeros_like" | "ones_like" | "empty_like" | "rand_like" | "randn_like" | "full_like" | "randint_like") {
            if name.ends_with("_like") {
                return args.first().and_then(|a| self.t_shape(a, st)).map(tensor_value).unwrap_or_else(|| tensor_value(Shape::any()));
            }
            let pos: Vec<Value> = if name == "full" { args.iter().take(1).cloned().collect() } else { args.clone() };
            let pos = if pos.is_empty() { kw(c, "size").cloned().into_iter().collect() } else { pos };
            // a float size (a literal float, a true division) raises
            let is_float = |v: &Value| quotient_of(v).is_some() || (v.pts.is_empty() && v.kv.as_lit().map(|k| matches!(intern::key_get(k), intern::Key::Float(_))).unwrap_or(false));
            if pos.iter().any(|v| is_float(v)) {
                let m = format!("torch.{}: sizes must be ints, got a float", name);
                self.t_error(&node, m, st);
                return Value::new();
            }
            let sz = if pos.len() == c.args.len() { self.t_sizes_call(c, &pos, st) } else { self.t_sizes(&pos, &c.star_args, st) };
            return sz.map(tensor_value).unwrap_or_else(|| tensor_value(Shape::any()));
        }
        match name {
            "tensor" | "as_tensor" | "Tensor" | "FloatTensor" | "LongTensor" => return self.t_literal_tensor(args.first(), st),
            "from_numpy" | "load" | "frombuffer" => return tensor_value(Shape::any()),
            "arange" => {
                let get = |an: &Analyzer, v: &Value| -> Option<(Option<f64>, Option<DimId>)> {
                    if let Some(k) = v.kv.as_lit().filter(|_| v.pts.is_empty()) {
                        if let intern::Key::Float(b) = intern::key_get(k) {
                            return Some((Some(f64::from_bits(b)), None));
                        }
                    }
                    let d = an.t_dim(v, st)?;
                    Some((sh::as_lit(d).map(|x| x as f64), Some(d)))
                };
                let mut vals: Vec<Value> = args.clone();
                for k in ["start", "end", "step"] {
                    if let Some(v) = kw(c, k) {
                        let _ = k;
                        vals.push(v.clone());
                    }
                }
                let parsed: Option<Vec<(Option<f64>, Option<DimId>)>> = vals.iter().map(|v| get(self, v)).collect();
                let Some(p) = parsed else { return tensor_value(Shape::known(vec![sh::unknown()])) };
                let (start, end, step) = match p.len() {
                    1 => ((Some(0.0), Some(sh::lit(0))), p[0], (Some(1.0), Some(sh::lit(1)))),
                    2 => (p[0], p[1], (Some(1.0), Some(sh::lit(1)))),
                    3 => (p[0], p[1], p[2]),
                    _ => return tensor_value(Shape::known(vec![sh::unknown()])),
                };
                if let (Some(a), Some(b), Some(s)) = (start.0, end.0, step.0) {
                    if s != 0.0 {
                        let n = ((b - a) / s).ceil().max(0.0) as i64;
                        return tensor_value(Shape::known(vec![sh::lit(n)]));
                    }
                }
                if step.0 == Some(1.0) {
                    if let (Some(a), Some(b)) = (start.1, end.1) {
                        return tensor_value(Shape::known(vec![sh::norm(sh::sub(b, a), &st.tf)]));
                    }
                }
                return tensor_value(Shape::known(vec![sh::unknown()]));
            }
            "linspace" | "logspace" => {
                let n = args.get(2).cloned().or_else(|| kw(c, "steps").cloned()).and_then(|v| self.t_dim(&v, st)).unwrap_or_else(sh::unknown);
                return tensor_value(Shape::known(vec![n]));
            }
            "eye" => {
                let n = args.first().cloned().or_else(|| kw(c, "n").cloned()).and_then(|v| self.t_dim(&v, st)).unwrap_or_else(sh::unknown);
                let m = args.get(1).cloned().or_else(|| kw(c, "m").cloned()).and_then(|v| self.t_dim(&v, st)).unwrap_or(n);
                return tensor_value(Shape::known(vec![n, m]));
            }
            "randint" => {
                let size = kw(c, "size").cloned().or_else(|| args.iter().rev().find(|a| self.t_seq(a, st).is_some() || size_shape(a).is_some()).cloned());
                return size.and_then(|v| self.t_shape_arg(&v, st)).map(tensor_value).unwrap_or_else(|| tensor_value(Shape::any()));
            }
            "randperm" => {
                let n = args.first().and_then(|v| self.t_dim(v, st)).unwrap_or_else(sh::unknown);
                return tensor_value(Shape::known(vec![n]));
            }
            "cat" | "concat" | "concatenate" | "stack" | "hstack" | "vstack" => {
                let Some(list) = args.first().cloned().or_else(|| kw(c, "tensors").cloned()) else { return tensor_value(Shape::any()) };
                let d = args.get(1).cloned().or_else(|| kw(c, "dim").cloned()).and_then(|v| self.t_int(&v)).unwrap_or(0);
                return if name == "stack" { self.t_stack(&list, d, &node, st) } else if name == "vstack" || name == "hstack" { tensor_value(Shape::any()) } else { self.t_cat(&list, d, &node, st) };
            }
            "einsum" => return self.t_einsum(c, st),
            "where" => {
                if args.len() == 1 {
                    return tensor_value(Shape::any());
                }
                let mut r = args.first().and_then(|a| self.t_shape(a, st)).unwrap_or_else(Shape::any);
                for a in args.iter().skip(1) {
                    if let Some(o) = self.t_shape(a, st) {
                        match sh::broadcast(&r, &o, &st.tf) {
                            Ok(x) => r = x,
                            Err(_) => return self.t_broadcast(&r, &o, &node, st, "where"),
                        }
                    }
                }
                return tensor_value(r);
            }
            "broadcast_tensors" => {
                let shapes: Vec<Shape> = args.iter().filter_map(|a| self.t_shape(a, st)).collect();
                let mut r = shapes.first().cloned().unwrap_or_else(Shape::any);
                for o in shapes.iter().skip(1) {
                    match sh::broadcast(&r, o, &st.tf) {
                        Ok(x) => r = x,
                        Err(_) => return self.t_broadcast(&r, o, &node, st, "broadcast_tensors"),
                    }
                }
                let vals = vec![tensor_value(r); args.len()];
                return self.t_tuple(vals, &node, st, None);
            }
            "broadcast_shapes" => return Value::data("size"),
            "finfo" | "iinfo" => return num_value(),
            "manual_seed" | "set_grad_enabled" | "no_grad" | "inference_mode" | "enable_grad" | "device" | "save" | "set_default_dtype" | "is_tensor" | "numel_" | "compile" | "jit" => {
                return Value::data(&format!("L{}", node.line));
            }
            "numel" => {
                return match args.first().and_then(|a| self.t_shape(a, st)).and_then(|s| s.dims()) {
                    Some(d) => dim_value(sh::product(&d)),
                    None => Value::data("numel"),
                }
            }
            _ => {}
        }
        // torch.f(x, …) ≡ x.f(…)
        if let Some(first) = args.first() {
            if self.t_shape(first, st).is_some() && (TENSOR_METHODS.contains(&name) || IDENTITY.contains(&name) || BOOL_OPS.contains(&name) || BINARY.contains(&name)) {
                let rest: Vec<Value> = args[1..].to_vec();
                let recv = first.clone();
                return self.t_method(name, &recv, &rest, c, st);
            }
        }
        if let Some(first) = kw(c, "input").cloned() {
            if self.t_shape(&first, st).is_some() && TENSOR_METHODS.contains(&name) {
                return self.t_method(name, &first, &args, c, st);
            }
        }
        Value::data(&format!("L{}", node.line))
    }

    /// The tensors of a list / tuple argument: known shapes, or (None, elem) for an unknown count.
    fn t_tensor_list(&mut self, list: &Value, st: &State) -> (Option<Vec<Shape>>, Option<Shape>) {
        if let Some(items) = self.t_seq(list, st) {
            let shapes: Vec<Shape> = items.iter().map(|v| self.t_shape(v, st).unwrap_or_else(Shape::any)).collect();
            return (Some(shapes), None);
        }
        if let Some(o) = list.single() {
            if matches!(self.kind(o), Kind::List | Kind::Tuple) {
                let e = self.t_list_elem(o, st);
                return (None, Some(self.t_shape(&e, st).unwrap_or_else(Shape::any)));
            }
        }
        (None, None)
    }

    pub fn t_cat(&mut self, list: &Value, d: i64, node: &Node, st: &mut State) -> Value {
        let (shapes, elem) = self.t_tensor_list(list, st);
        let Some(shapes) = shapes else {
            return match elem {
                Some(e) => match self.t_locate(&e, d, node, st, "cat") {
                    Some((e2, p)) => tensor_value(replace_at(&e2, p, &[sh::unknown()])),
                    None => Value::new(),
                },
                None => tensor_value(Shape::any()),
            };
        };
        if shapes.is_empty() {
            let m = "cat: expected a non-empty list of tensors".to_string();
            self.t_error(node, m, st);
            return Value::new();
        }
        // rank: known for one means known for all
        let rank = shapes.iter().find_map(|s| s.rank());
        if let Some(r) = rank {
            for s in &shapes {
                if !self.t_require_rank(s, RankRel::Eq(r as u32), node, st, "cat") {
                    return Value::new();
                }
            }
        }
        let shapes: Vec<Shape> = shapes.iter().map(|s| sh::norm_shape(s, &st.tf)).collect();
        let mut located: Vec<(Shape, Pos)> = Vec::new();
        for s in &shapes {
            match self.t_locate(s, d, node, st, "cat") {
                Some(x) => located.push(x),
                None => return Value::new(),
            }
        }
        let (first, p0) = located[0].clone();
        let mut total = get_at(&first, p0);
        for (s, p) in located.iter().skip(1) {
            total = sh::add(total, get_at(s, *p));
            // the other dims must agree
            if let (Some(a), Some(b)) = (first.dims(), s.dims()) {
                for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
                    if Pos::F(i) == p0 {
                        continue;
                    }
                    if !self.t_require_eq(*x, *y, node, st, &format!("cat (dim {})", i)) {
                        return Value::new();
                    }
                }
            }
        }
        let first = sh::norm_shape(&first, &st.tf);
        tensor_value(replace_at(&first, p0, &[sh::norm(total, &st.tf)]))
    }

    pub fn t_stack(&mut self, list: &Value, d: i64, node: &Node, st: &mut State) -> Value {
        let (shapes, elem) = self.t_tensor_list(list, st);
        let (n, base) = match shapes {
            Some(v) if !v.is_empty() => {
                let first = sh::norm_shape(&v[0], &st.tf);
                for s in v.iter().skip(1) {
                    let s = sh::norm_shape(s, &st.tf);
                    match (first.dims(), s.dims()) {
                        (Some(a), Some(b)) => {
                            if a.len() != b.len() {
                                let m = "stack: tensors of different ranks".to_string();
                                self.t_error(node, m, st);
                                return Value::new();
                            }
                            for (x, y) in a.iter().zip(b.iter()) {
                                if !self.t_require_eq(*x, *y, node, st, "stack") {
                                    return Value::new();
                                }
                            }
                        }
                        _ => {}
                    }
                }
                (sh::lit(v.len() as i64), sh::norm_shape(&first, &st.tf))
            }
            Some(_) => {
                let m = "stack: expected a non-empty list of tensors".to_string();
                self.t_error(node, m, st);
                return Value::new();
            }
            None => (self.t_tuple_len(list).unwrap_or_else(sh::unknown), elem.unwrap_or_else(Shape::any)),
        };
        let mut nv = vec![n];
        nv.truncate(1);
        // insert a dim at d (counted over rank + 1)
        let r = base.rank();
        match r {
            Some(r) => {
                let r = r as i64;
                let p = if d < 0 { d + r + 1 } else { d };
                if p < 0 || p > r {
                    let m = format!("stack: dim {} out of range", d);
                    self.t_error(node, m, st);
                    return Value::new();
                }
                let mut dd = base.dims().unwrap();
                dd.insert(p as usize, n);
                tensor_value(Shape::known(dd))
            }
            None => {
                if d >= 0 {
                    let mut e = sh::expose(&base, d as usize, 0);
                    if e.rest.is_none() {
                        let mut dd = e.dims().unwrap();
                        dd.insert(d as usize, n);
                        return tensor_value(Shape::known(dd));
                    }
                    e.prefix.insert(d as usize, n);
                    tensor_value(e)
                } else {
                    let k = (-d - 1) as usize;
                    let mut e = sh::expose(&base, 0, k);
                    let len = e.suffix.len();
                    e.suffix.insert(len - k, n);
                    tensor_value(e)
                }
            }
        }
    }

    pub fn t_einsum(&mut self, c: &Call, st: &mut State) -> Value {
        let node = c.node;
        let Some(eq) = c.args.first().and_then(|v| self.t_str(v)) else { return tensor_value(Shape::any()) };
        let mut ops: Vec<Value> = c.args[1..].to_vec();
        if ops.len() == 1 {
            if let Some(items) = self.t_seq(&ops[0], st) {
                ops = items;
            }
        }
        let eq: String = eq.chars().filter(|ch| !ch.is_whitespace()).collect();
        let (lhs, rhs) = match eq.split_once("->") {
            Some((l, r)) => (l.to_string(), Some(r.to_string())),
            None => (eq.clone(), None),
        };
        let terms: Vec<&str> = lhs.split(',').collect();
        if terms.len() != ops.len() {
            return tensor_value(Shape::any());
        }
        let mut letters: IndexMap<char, DimId> = IndexMap::new();
        let mut counts: IndexMap<char, usize> = IndexMap::new();
        let mut ell: Option<Shape> = None;
        for (t, v) in terms.iter().zip(ops.iter()) {
            let s = self.t_shape(v, st).unwrap_or_else(Shape::any);
            let has_ell = t.contains("...");
            let (front, back) = match t.split_once("...") {
                Some((f, b)) => (f.to_string(), b.to_string()),
                None => (t.to_string(), String::new()),
            };
            let (fl, bl): (Vec<char>, Vec<char>) = (front.chars().collect(), back.chars().collect());
            let s = if has_ell {
                if !self.t_require_rank(&s, RankRel::Ge((fl.len() + bl.len()) as u32), &node, st, "einsum") {
                    return Value::new();
                }
                sh::norm_shape(&s, &st.tf)
            } else {
                if !self.t_require_rank(&s, RankRel::Eq(fl.len() as u32), &node, st, "einsum") {
                    return Value::new();
                }
                sh::norm_shape(&s, &st.tf)
            };
            let e = sh::expose(&s, fl.len(), bl.len());
            let (fd, mid, bd): (Vec<DimId>, Shape, Vec<DimId>) = match e.dims() {
                Some(d) => {
                    let n = d.len();
                    (d[..fl.len()].to_vec(), Shape::known(d[fl.len()..n - bl.len()].to_vec()), d[n - bl.len()..].to_vec())
                }
                None => {
                    let n = e.suffix.len();
                    (e.prefix[..fl.len()].to_vec(), sh::tidy(Shape { prefix: e.prefix[fl.len()..].to_vec(), rest: e.rest.clone(), suffix: e.suffix[..n - bl.len()].to_vec(), assume: vec![] }), e.suffix[n - bl.len()..].to_vec())
                }
            };
            if has_ell {
                ell = Some(match ell {
                    None => mid,
                    Some(prev) => match sh::broadcast(&prev, &mid, &st.tf) {
                        Ok(x) => x,
                        Err(_) => Shape::any(),
                    },
                });
            }
            for (ch, d) in fl.iter().chain(bl.iter()).zip(fd.iter().chain(bd.iter())) {
                *counts.entry(*ch).or_insert(0) += 1;
                match letters.get(ch).copied() {
                    None => {
                        letters.insert(*ch, *d);
                    }
                    Some(prev) => {
                        if sh::dim_eq(prev, *d, &st.tf) == Some(false) && sh::is_one(prev, &st.tf) == Some(false) && sh::is_one(*d, &st.tf) == Some(false) {
                            let m = self.t_print(st, || format!("einsum: '{}' is {} in one operand and {} in another", ch, sh::dim_text(prev), sh::dim_text(*d)));
                            self.t_error(&node, m, st);
                            return Value::new();
                        }
                        // einsum broadcasts a letter's size-1 dims: the result is the larger
                        if let Ok(b) = sh::bdim(prev, *d, &st.tf) {
                            letters.insert(*ch, b);
                        }
                    }
                }
            }
        }
        let out: String = match rhs {
            Some(r) => r,
            None => {
                let mut v: Vec<char> = counts.iter().filter(|(_, n)| **n == 1).map(|(c, _)| *c).collect();
                v.sort();
                let s: String = v.into_iter().collect();
                if ell.is_some() { format!("...{}", s) } else { s }
            }
        };
        let (f, b) = match out.split_once("...") {
            Some((f, b)) => (f.to_string(), Some(b.to_string())),
            None => (out.clone(), None),
        };
        let fd: Option<Vec<DimId>> = f.chars().map(|ch| letters.get(&ch).copied()).collect();
        let Some(fd) = fd else { return tensor_value(Shape::any()) };
        match b {
            None => tensor_value(Shape::known(fd)),
            Some(b) => {
                let bd: Option<Vec<DimId>> = b.chars().map(|ch| letters.get(&ch).copied()).collect();
                let Some(bd) = bd else { return tensor_value(Shape::any()) };
                let mid = ell.unwrap_or_else(|| Shape::known(vec![]));
                tensor_value(concat(&concat(&Shape::known(fd), &mid), &Shape::known(bd)))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// nn layers
// ---------------------------------------------------------------------------

/// Constructor parameters in positional order (the config becomes the layer's attributes).
fn layer_params(t: &str) -> &'static [&'static str] {
    match t {
        "Linear" | "LazyLinear" => &["in_features", "out_features", "bias"],
        "Bilinear" => &["in1_features", "in2_features", "out_features", "bias"],
        "Embedding" => &["num_embeddings", "embedding_dim", "padding_idx"],
        "EmbeddingBag" => &["num_embeddings", "embedding_dim"],
        "LayerNorm" | "RMSNorm" => &["normalized_shape", "eps", "elementwise_affine"],
        "BatchNorm1d" | "BatchNorm2d" | "BatchNorm3d" | "InstanceNorm1d" | "InstanceNorm2d" | "InstanceNorm3d" | "SyncBatchNorm" => &["num_features", "eps", "momentum", "affine"],
        "GroupNorm" => &["num_groups", "num_channels", "eps", "affine"],
        "Softmax" | "LogSoftmax" | "Softmin" | "GLU" => &["dim"],
        "Flatten" => &["start_dim", "end_dim"],
        "Unflatten" => &["dim", "unflattened_size"],
        "Conv1d" | "Conv2d" | "Conv3d" => &["in_channels", "out_channels", "kernel_size", "stride", "padding", "dilation", "groups", "bias", "padding_mode"],
        "ConvTranspose1d" | "ConvTranspose2d" | "ConvTranspose3d" => &["in_channels", "out_channels", "kernel_size", "stride", "padding", "output_padding", "groups", "bias", "dilation"],
        "MaxPool1d" | "MaxPool2d" | "MaxPool3d" => &["kernel_size", "stride", "padding", "dilation", "return_indices", "ceil_mode"],
        "AvgPool1d" | "AvgPool2d" | "AvgPool3d" => &["kernel_size", "stride", "padding", "ceil_mode"],
        "AdaptiveAvgPool1d" | "AdaptiveAvgPool2d" | "AdaptiveAvgPool3d" | "AdaptiveMaxPool1d" | "AdaptiveMaxPool2d" => &["output_size"],
        "Upsample" | "UpsamplingNearest2d" | "UpsamplingBilinear2d" => &["size", "scale_factor", "mode"],
        "RNN" => &["input_size", "hidden_size", "num_layers", "nonlinearity", "bias", "batch_first", "dropout", "bidirectional"],
        "LSTM" | "GRU" => &["input_size", "hidden_size", "num_layers", "bias", "batch_first", "dropout", "bidirectional", "proj_size"],
        "RNNCell" | "LSTMCell" | "GRUCell" => &["input_size", "hidden_size", "bias"],
        "MultiheadAttention" => &["embed_dim", "num_heads", "dropout", "bias", "add_bias_kv", "add_zero_attn", "kdim", "vdim", "batch_first"],
        "TransformerEncoderLayer" | "TransformerDecoderLayer" => &["d_model", "nhead", "dim_feedforward", "dropout", "activation", "layer_norm_eps", "batch_first", "norm_first"],
        "TransformerEncoder" | "TransformerDecoder" => &["encoder_layer", "num_layers", "norm"],
        "CrossEntropyLoss" | "NLLLoss" | "MSELoss" | "L1Loss" | "BCELoss" | "BCEWithLogitsLoss" | "SmoothL1Loss" | "HuberLoss" | "KLDivLoss" => &["weight", "size_average", "ignore_index", "reduce", "reduction"],
        "PixelShuffle" => &["upscale_factor"],
        "ZeroPad2d" | "ConstantPad1d" | "ConstantPad2d" | "ReflectionPad2d" | "ReplicationPad2d" => &["padding"],
        _ => &[],
    }
}

const ELEMENTWISE_LAYERS: &[&str] = &[
    "ReLU", "ReLU6", "LeakyReLU", "PReLU", "GELU", "SiLU", "Mish", "ELU", "SELU", "CELU", "Sigmoid", "Tanh", "Softplus", "Softsign", "Hardswish", "Hardsigmoid", "Hardtanh",
    "LogSigmoid", "Tanhshrink", "Identity", "Dropout", "Dropout1d", "Dropout2d", "Dropout3d", "AlphaDropout", "FeatureAlphaDropout", "Threshold", "RReLU",
];

impl Analyzer {
    pub fn t_is_module_instance(&self, o: u32) -> bool {
        let Some(cls) = self.instance_class(o) else { return false };
        self.mro(cls).iter().any(|c| self.class_info_opt(*c).map(|ci| ci.nn_module).unwrap_or(false))
    }

    pub fn t_is_module_class(&self, cls: u32) -> bool {
        self.mro(cls).iter().any(|c| self.class_info_opt(*c).map(|ci| ci.nn_module).unwrap_or(false))
    }

    /// `self.register_buffer` & co. on a user module (None: a user attribute).
    pub fn t_module_attr(&mut self, o: u32, name: &str, _node: &Node, _st: &mut State) -> Option<Value> {
        const MODULE_METHODS: &[&str] = &[
            "register_buffer", "register_parameter", "add_module", "register_module", "parameters", "named_parameters", "buffers", "named_buffers", "modules", "named_modules",
            "children", "named_children", "to", "cuda", "cpu", "eval", "train", "apply", "zero_grad", "state_dict", "load_state_dict", "requires_grad_", "half", "float", "double",
            "bfloat16", "type", "share_memory", "get_submodule", "get_parameter", "register_forward_hook", "register_forward_pre_hook", "register_full_backward_hook", "extra_repr",
        ];
        if !MODULE_METHODS.contains(&name) {
            return None;
        }
        let cls = self.instance_class(o)?;
        if self.class_lookup(cls, name).is_some() {
            return None;
        }
        Some(self.method_value(&format!("nnmodule.{}", name), &Value::obj(o)))
    }

    pub fn t_layer_type(&self, o: u32) -> Option<String> {
        match &self.obj(o).payload {
            Payload::Str(q) if self.kind(o) == Kind::Namespace => q.strip_prefix("torch.nn.").map(|x| x.to_string()),
            Payload::Str(q) if matches!(self.kind(o), Kind::List | Kind::Dict) && q.starts_with("torch.nn.") => q.strip_prefix("torch.nn.").map(|x| x.to_string()),
            _ => None,
        }
    }

    fn t_cfg(&self, o: u32, name: &str, st: &State) -> Value {
        st.slot(o, Ns::Attr, key_str_lit(name))
    }

    fn t_cfg_dim(&self, o: u32, name: &str, st: &State) -> DimId {
        self.t_dim(&self.t_cfg(o, name, st), st).unwrap_or_else(sh::unknown)
    }

    /// `n` dims from an int-or-tuple config value.
    fn t_tuple_dims(&self, v: &Value, n: usize, default: i64, st: &State) -> Vec<DimId> {
        if v.kv.is_bot() && v.pts.is_empty() || self.t_is_none(v) {
            return vec![sh::lit(default); n];
        }
        if let Some(items) = self.t_seq(v, st) {
            if items.len() == n {
                return items.iter().map(|x| self.t_dim(x, st).unwrap_or_else(sh::unknown)).collect();
            }
            return vec![sh::unknown(); n];
        }
        vec![self.t_dim(v, st).unwrap_or_else(sh::unknown); n]
    }

    /// `nn.<Type>(…)`: a layer object holding its config.
    pub fn t_layer_new(&mut self, ty: &str, c: &Call, st: &mut State) -> Value {
        let node = c.node;
        if ty == "Parameter" || ty == "Buffer" || ty == "parameter.Parameter" {
            return c.arg(0, Some("data")).unwrap_or_else(|| tensor_value(Shape::any()));
        }
        if ty == "Sequential" || ty == "ModuleList" {
            let items: Vec<Value> = if ty == "Sequential" {
                let mut v = c.args.clone();
                for sa in &c.star_args {
                    match self.t_seq(sa, st) {
                        Some(xs) => v.extend(xs),
                        None => {
                            let elem = sa.single().map(|o| self.t_list_elem(o, st));
                            return self.t_list_layer(ty, None, elem, &node, st);
                        }
                    }
                }
                // Sequential(OrderedDict(...)): its values, in order, also as attributes
                if v.len() == 1 && c.star_args.is_empty() {
                    if let Some(d) = v[0].single().filter(|d| self.kind(*d) == Kind::Dict) {
                        if let Some(keys) = st.row(d, Ns::Item).ordered_keys() {
                            let vals: Vec<Value> = keys.iter().map(|k| st.slot(d, Ns::Item, *k)).collect();
                            let lv = self.t_list_layer(ty, Some(vals.clone()), None, &node, st);
                            if let Some(lo) = lv.single() {
                                for (k, val) in keys.iter().zip(vals.iter()) {
                                    st.set_slot(lo, Ns::Attr, *k, val.clone(), true);
                                }
                            }
                            return lv;
                        }
                    }
                }
                v
            } else {
                match c.args.first().cloned().or_else(|| kw(c, "modules").cloned()) {
                    None => vec![],
                    Some(l) => match self.t_seq(&l, st) {
                        Some(xs) => xs,
                        None => {
                            let elem = l.single().map(|o| st.slot(o, Ns::Elem, STAR));
                            return self.t_list_layer(ty, None, elem, &node, st);
                        }
                    },
                }
            };
            // Sequential(OrderedDict(...)): its values
            return self.t_list_layer(ty, Some(items), None, &node, st);
        }
        if ty == "ModuleDict" || ty == "ParameterDict" {
            let v = c.args.first().cloned().unwrap_or_else(|| {
                let o = self.alloc_at(Kind::Dict, &node, st, &format!("dict@L{}", node.line), Payload::None, None, Ns::Item, None, None);
                Value::obj(o)
            });
            // its items are also its attributes (`self.transformer.wte`)
            for o in v.pts.iter() {
                if self.kind(o) == Kind::Dict {
                    self.obj_mut(o).payload = Payload::Str("torch.nn.ModuleDict".into());
                }
            }
            return v;
        }
        if ty == "ParameterList" {
            return c.args.first().cloned().unwrap_or_else(|| self.alloc_positional(Kind::List, &node, Some(vec![]), Value::new(), DATA, st));
        }
        let params = layer_params(ty);
        let known = !params.is_empty() || ELEMENTWISE_LAYERS.contains(&ty);
        if !known {
            return Value::data(&format!("L{}", node.line));
        }
        let o = self.alloc_at(Kind::Namespace, &node, st, &format!("{}@L{}", ty, node.line), Payload::Str(format!("torch.nn.{}", ty)), Some(crate::domain::Row::new()), Ns::Attr, None, None);
        let strong = self.obj(o).single;
        let set = |an: &mut Analyzer, st: &mut State, name: &str, v: Value| {
            an.store_key(o, Ns::Attr, &KeyVal::Lit(key_str_lit(name)), &v, strong, &node, st);
        };
        for (i, p) in params.iter().enumerate() {
            if let Some(v) = c.arg(i, Some(p)) {
                set(self, st, p, v);
            }
        }
        // derived attributes people read
        let g = |an: &Analyzer, st: &State, n: &str| an.t_dim(&st.slot(o, Ns::Attr, key_str_lit(n)), st).unwrap_or_else(sh::unknown);
        match ty {
            "Linear" => {
                let (i, out) = (g(self, st, "in_features"), g(self, st, "out_features"));
                set(self, st, "weight", tensor_value(Shape::known(vec![out, i])));
                let has_bias = self.t_cfg(o, "bias", st);
                let b = if self.t_bool(&has_bias) == Some(false) { self.none_v.clone() } else { tensor_value(Shape::known(vec![out])) };
                set(self, st, "bias", b);
            }
            "Bilinear" => {
                let (a, b2, out) = (g(self, st, "in1_features"), g(self, st, "in2_features"), g(self, st, "out_features"));
                set(self, st, "weight", tensor_value(Shape::known(vec![out, a, b2])));
                set(self, st, "bias", tensor_value(Shape::known(vec![out])));
            }
            "Embedding" | "EmbeddingBag" => {
                let (n, d) = (g(self, st, "num_embeddings"), g(self, st, "embedding_dim"));
                set(self, st, "weight", tensor_value(Shape::known(vec![n, d])));
            }
            "Conv1d" | "Conv2d" | "Conv3d" | "ConvTranspose1d" | "ConvTranspose2d" | "ConvTranspose3d" => {
                let n = ty.chars().rev().nth(1).and_then(|c| c.to_digit(10)).unwrap_or(2) as usize;
                let (ci, co) = (g(self, st, "in_channels"), g(self, st, "out_channels"));
                let k = self.t_tuple_dims(&self.t_cfg(o, "kernel_size", st), n, 1, st);
                let groups = self.t_cfg(o, "groups", st);
                let gd = self.t_dim(&groups, st).filter(|_| !groups.kv.is_bot()).unwrap_or(sh::lit(1));
                let mut w = if ty.starts_with("ConvT") { vec![ci, sh::floordiv(co, gd)] } else { vec![co, sh::floordiv(ci, gd)] };
                w.extend(k);
                set(self, st, "weight", tensor_value(Shape::known(w)));
            }
            "LayerNorm" | "RMSNorm" => {
                let ns = self.t_cfg(o, "normalized_shape", st);
                if let Some(s) = self.t_shape_arg(&ns, st) {
                    set(self, st, "weight", tensor_value(s));
                }
            }
            "BatchNorm1d" | "BatchNorm2d" | "BatchNorm3d" => {
                let n = g(self, st, "num_features");
                set(self, st, "weight", tensor_value(Shape::known(vec![n])));
            }
            _ => {}
        }
        Value::obj(o)
    }

    fn t_list_layer(&mut self, ty: &str, items: Option<Vec<Value>>, elem: Option<Value>, node: &Node, st: &mut State) -> Value {
        let v = match items {
            Some(xs) => self.alloc_positional(Kind::List, node, Some(xs), Value::new(), DATA, st),
            None => {
                let e = elem.unwrap_or_default();
                self.alloc_positional(Kind::List, node, None, e, DATA, st)
            }
        };
        if let Some(o) = v.single() {
            self.obj_mut(o).payload = Payload::Str(format!("torch.nn.{}", ty));
        }
        v
    }

    /// Call a module value (a layer, a Sequential, a user module): its output.
    pub fn t_call_module(&mut self, m: &Value, args: Vec<Value>, node: &Node, st: &mut State) -> Value {
        self.call_value(m, node, None, st, Some(args))
    }

    pub fn t_layer_call(&mut self, o: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, node: &Node, st: &mut State) -> Value {
        let Some(ty) = self.t_layer_type(o) else { return Value::data("layer") };
        let x = args.first().cloned().or_else(|| kwargs.get(&s("input")).cloned()).or_else(|| kwargs.get(&s("src")).cloned()).or_else(|| kwargs.get(&s("query")).cloned()).unwrap_or_default();
        let s0 = self.t_shape(&x, st).unwrap_or_else(Shape::any);
        let what = ty.clone();
        if ELEMENTWISE_LAYERS.contains(&ty.as_str()) {
            return tensor_value(s0);
        }
        match ty.as_str() {
            "Sequential" => {
                let mut cur = x;
                let items = self.t_seq(&Value::obj(o), st);
                match items {
                    Some(mods) => {
                        for m in mods {
                            cur = self.t_call_module(&m, vec![cur], node, st);
                            if !st.reachable {
                                return Value::new();
                            }
                        }
                        cur
                    }
                    None => {
                        // an unknown number of modules: the output of any number of them (the
                        // call of "some module" is quiet: no particular module must accept
                        // this input, so nothing is an error, a requirement or a fact)
                        let e = self.t_list_elem(o, st);
                        let mut quiet_st = st.copy();
                        let saved = std::mem::replace(&mut self.t_quiet, true);
                        let once = self.t_call_module(&e, vec![cur.clone()], node, &mut quiet_st);
                        self.t_quiet = saved;
                        let a = self.t_shape(&cur, st);
                        let b = self.t_shape(&once, st);
                        match (a, b) {
                            (Some(a), Some(b)) => tensor_value(sh::join_shapes(&a, &b)),
                            _ => tensor_value(Shape::any()),
                        }
                    }
                }
            }
            "Linear" | "LazyLinear" => {
                // the weight's shape (it may have been replaced: weight tying)
                let w = self.t_cfg(o, "weight", st);
                let (i, out) = match tensor_shape(&w).and_then(|ws| sh::norm_shape(&ws, &st.tf).dims()) {
                    Some(d) if d.len() == 2 => (d[1], d[0]),
                    _ => (self.t_cfg_dim(o, "in_features", st), self.t_cfg_dim(o, "out_features", st)),
                };
                self.t_linear(&s0, i, out, node, st, &what)
            }
            "Bilinear" => {
                let x2 = args.get(1).and_then(|v| self.t_shape(v, st)).unwrap_or_else(Shape::any);
                let (i1, i2, out) = (self.t_cfg_dim(o, "in1_features", st), self.t_cfg_dim(o, "in2_features", st), self.t_cfg_dim(o, "out_features", st));
                let Some((e1, p1)) = self.t_locate(&s0, -1, node, st, &what) else { return Value::new() };
                if !self.t_require_eq(get_at(&e1, p1), i1, node, st, "Bilinear input1") {
                    return Value::new();
                }
                let Some((e2, p2)) = self.t_locate(&x2, -1, node, st, &what) else { return Value::new() };
                if !self.t_require_eq(get_at(&e2, p2), i2, node, st, "Bilinear input2") {
                    return Value::new();
                }
                if let (Some(a), Some(b)) = (e1.dims(), e2.dims()) {
                    if a.len() == b.len() {
                        for (p, q) in a[..a.len() - 1].iter().zip(b[..b.len() - 1].iter()) {
                            if !self.t_require_eq(*p, *q, node, st, "Bilinear batch") {
                                return Value::new();
                            }
                        }
                    }
                }
                let e1 = sh::norm_shape(&e1, &st.tf);
                tensor_value(replace_at(&e1, p1, &[out]))
            }
            "Embedding" | "EmbeddingBag" => {
                let d = self.t_cfg_dim(o, "embedding_dim", st);
                tensor_value(append_dims(&s0, &[d]))
            }
            "LayerNorm" | "RMSNorm" => {
                let ns = self.t_cfg(o, "normalized_shape", st);
                let shp = self.t_shape_arg(&ns, st).unwrap_or_else(Shape::any);
                self.t_layer_norm(&s0, &shp, node, st, &what)
            }
            "BatchNorm1d" | "BatchNorm2d" | "BatchNorm3d" | "InstanceNorm1d" | "InstanceNorm2d" | "InstanceNorm3d" | "SyncBatchNorm" => {
                let c = self.t_cfg_dim(o, "num_features", st);
                let rel = match ty.as_str() {
                    "BatchNorm1d" => RankRel::In(vec![2, 3]),
                    "BatchNorm2d" => RankRel::Eq(4),
                    "BatchNorm3d" => RankRel::Eq(5),
                    "InstanceNorm1d" => RankRel::In(vec![2, 3]),
                    "InstanceNorm2d" => RankRel::In(vec![3, 4]),
                    "InstanceNorm3d" => RankRel::In(vec![4, 5]),
                    _ => RankRel::Ge(2),
                };
                if !self.t_require_rank(&s0, rel, node, st, &what) {
                    return Value::new();
                }
                let s1 = sh::norm_shape(&s0, &st.tf);
                let idx = if ty.starts_with("Instance") { if ty.ends_with("1d") { -2 } else if ty.ends_with("2d") { -3 } else { -4 } } else { 1 };
                let Some((e, p)) = self.t_locate(&s1, idx, node, st, &what) else { return Value::new() };
                if !self.t_require_eq(get_at(&e, p), c, node, st, &format!("{} channels", what)) {
                    return Value::new();
                }
                tensor_value(sh::norm_shape(&s1, &st.tf))
            }
            "GroupNorm" => {
                let c = self.t_cfg_dim(o, "num_channels", st);
                let Some((e, p)) = self.t_locate(&s0, 1, node, st, &what) else { return Value::new() };
                if !self.t_require_eq(get_at(&e, p), c, node, st, "GroupNorm channels") {
                    return Value::new();
                }
                tensor_value(sh::norm_shape(&e, &st.tf))
            }
            "Softmax" | "LogSoftmax" | "Softmin" => {
                let d = self.t_cfg(o, "dim", st);
                if let Some(i) = self.t_int(&d) {
                    if self.t_locate(&s0, i, node, st, &what).is_none() {
                        return Value::new();
                    }
                }
                tensor_value(sh::norm_shape(&s0, &st.tf))
            }
            "Flatten" => {
                let a = self.t_int(&self.t_cfg(o, "start_dim", st)).unwrap_or(1);
                let b = self.t_int(&self.t_cfg(o, "end_dim", st)).unwrap_or(-1);
                self.t_flatten(&s0, a, b, node, st)
            }
            "Unflatten" => {
                let d = self.t_int(&self.t_cfg(o, "dim", st));
                let sz = self.t_shape_arg(&self.t_cfg(o, "unflattened_size", st), st);
                match (d, sz) {
                    (Some(d), Some(sz)) => self.t_unflatten(&s0, d, &sz, node, st),
                    _ => tensor_value(Shape::any()),
                }
            }
            t if t.starts_with("Conv") => {
                let n = t.chars().rev().nth(1).and_then(|c| c.to_digit(10)).unwrap_or(2) as usize;
                let transposed = t.starts_with("ConvTranspose");
                let ci = self.t_cfg_dim(o, "in_channels", st);
                let co = self.t_cfg_dim(o, "out_channels", st);
                let k = self.t_tuple_dims(&self.t_cfg(o, "kernel_size", st), n, 1, st);
                let stride = self.t_tuple_dims(&self.t_cfg(o, "stride", st), n, 1, st);
                let dil = self.t_tuple_dims(&self.t_cfg(o, "dilation", st), n, 1, st);
                let pv = self.t_cfg(o, "padding", st);
                let pad = match self.t_str(&pv).as_deref() {
                    Some("same") => None,
                    Some("valid") => Some(vec![sh::lit(0); n]),
                    _ => Some(self.t_tuple_dims(&pv, n, 0, st)),
                };
                let op = self.t_tuple_dims(&self.t_cfg(o, "output_padding", st), n, 0, st);
                self.t_conv(&s0, n, Some(ci), Some(co), &k, &stride, pad.as_deref(), &dil, if transposed { Some(&op) } else { None }, false, node, st, &what)
            }
            t if t.starts_with("MaxPool") || t.starts_with("AvgPool") => {
                let n = t.chars().rev().nth(1).and_then(|c| c.to_digit(10)).unwrap_or(2) as usize;
                let kv = self.t_cfg(o, "kernel_size", st);
                let k = self.t_tuple_dims(&kv, n, 1, st);
                let sv = self.t_cfg(o, "stride", st);
                let stride = if sv.kv.is_bot() && sv.pts.is_empty() || self.t_is_none(&sv) { k.clone() } else { self.t_tuple_dims(&sv, n, 1, st) };
                let pad = self.t_tuple_dims(&self.t_cfg(o, "padding", st), n, 0, st);
                let dil = if t.starts_with("Max") { self.t_tuple_dims(&self.t_cfg(o, "dilation", st), n, 1, st) } else { vec![sh::lit(1); n] };
                let ceil = self.t_bool(&self.t_cfg(o, "ceil_mode", st)).unwrap_or(false);
                self.t_conv(&s0, n, None, None, &k, &stride, Some(&pad), &dil, None, ceil, node, st, &what)
            }
            t if t.starts_with("Adaptive") => {
                let n = t.chars().rev().nth(1).and_then(|c| c.to_digit(10)).unwrap_or(2) as usize;
                let out = self.t_cfg(o, "output_size", st);
                self.t_adaptive(&s0, n, &out, node, st, &what)
            }
            t if t.starts_with("Upsampl") => {
                let size = self.t_cfg(o, "size", st);
                let sf = self.t_cfg(o, "scale_factor", st);
                self.t_interpolate(&s0, &size, &sf, node, st)
            }
            "RNN" | "LSTM" | "GRU" => self.t_rnn(o, &ty, &s0, node, st),
            "RNNCell" | "GRUCell" | "LSTMCell" => {
                let i = self.t_cfg_dim(o, "input_size", st);
                let h = self.t_cfg_dim(o, "hidden_size", st);
                let Some((e, p)) = self.t_locate(&s0, -1, node, st, &what) else { return Value::new() };
                if !self.t_require_eq(get_at(&e, p), i, node, st, &what) {
                    return Value::new();
                }
                let e = sh::norm_shape(&e, &st.tf);
                let out = tensor_value(replace_at(&e, p, &[h]));
                if ty == "LSTMCell" {
                    return self.t_tuple(vec![out.clone(), out], node, st, None);
                }
                out
            }
            "MultiheadAttention" => self.t_mha(o, args, kwargs, node, st),
            "TransformerEncoderLayer" | "TransformerDecoderLayer" => {
                let d = self.t_cfg_dim(o, "d_model", st);
                if !self.t_require_rank(&s0, RankRel::In(vec![2, 3]), node, st, &what) {
                    return Value::new();
                }
                let s0 = sh::norm_shape(&s0, &st.tf);
                let Some((e, p)) = self.t_locate(&s0, -1, node, st, &what) else { return Value::new() };
                if !self.t_require_eq(get_at(&e, p), d, node, st, &format!("{} d_model", what)) {
                    return Value::new();
                }
                if ty == "TransformerDecoderLayer" {
                    if let Some(m) = args.get(1).and_then(|v| self.t_shape(v, st)) {
                        let Some((e2, p2)) = self.t_locate(&m, -1, node, st, &what) else { return Value::new() };
                        if !self.t_require_eq(get_at(&e2, p2), d, node, st, "memory d_model") {
                            return Value::new();
                        }
                    }
                }
                tensor_value(sh::norm_shape(&s0, &st.tf))
            }
            "TransformerEncoder" | "TransformerDecoder" => {
                let layer = self.t_cfg(o, "encoder_layer", st);
                let out = self.t_call_module(&layer, args.to_vec(), node, st);
                if !st.reachable {
                    return Value::new();
                }
                let _ = out;
                tensor_value(sh::norm_shape(&s0, &st.tf))
            }
            "CrossEntropyLoss" | "NLLLoss" | "MSELoss" | "L1Loss" | "BCELoss" | "BCEWithLogitsLoss" | "SmoothL1Loss" | "HuberLoss" | "KLDivLoss" => {
                let red = self.t_str(&self.t_cfg(o, "reduction", st)).unwrap_or_else(|| "mean".into());
                let t = args.get(1).and_then(|v| self.t_shape(v, st)).unwrap_or_else(Shape::any);
                let f = match ty.as_str() {
                    "CrossEntropyLoss" => "cross_entropy",
                    "NLLLoss" => "nll_loss",
                    _ => "mse_loss",
                };
                self.t_loss(f, &s0, &t, &red, node, st)
            }
            "PixelShuffle" => tensor_value(Shape::any()),
            _ => tensor_value(Shape::any()),
        }
    }

    pub fn t_linear(&mut self, s: &Shape, i: DimId, out: DimId, node: &Node, st: &mut State, what: &str) -> Value {
        let Some((e, p)) = self.t_locate(s, -1, node, st, what) else { return Value::new() };
        let d = get_at(&e, p);
        let w = what.to_string();
        if !self.t_require(Cons::Eq(d, i), node, st, move || format!("{}({} -> {}) given last dim {}", w, sh::dim_text(i), sh::dim_text(out), sh::dim_text(d))) {
            return Value::new();
        }
        let e = sh::norm_shape(&e, &st.tf);
        tensor_value(replace_at(&e, p, &[out]))
    }

    pub fn t_layer_norm(&mut self, s: &Shape, shp: &Shape, node: &Node, st: &mut State, what: &str) -> Value {
        let Some(d) = shp.dims() else { return tensor_value(s.clone()) };
        let k = d.len();
        if k == 0 {
            return tensor_value(s.clone());
        }
        let Some((e, _)) = self.t_locate(s, -(k as i64), node, st, what) else { return Value::new() };
        for (j, want) in d.iter().enumerate() {
            let pos = -((k - j) as i64);
            let got = sh::dim_at(&e, pos).unwrap_or_else(sh::unknown);
            let w = what.to_string();
            let (g2, w2) = (got, *want);
            if !self.t_require(Cons::Eq(got, *want), node, st, move || format!("{} expects trailing dim {}, got {}", w, sh::dim_text(w2), sh::dim_text(g2))) {
                return Value::new();
            }
        }
        // the output has the normalised trailing dims (what the input's were required to be)
        let r = sh::norm_shape(s, &st.tf);
        let r = match r.dims() {
            Some(mut v) => {
                let n = v.len();
                v.splice(n - k.., d.iter().copied());
                Shape::known(v)
            }
            None => {
                let e = sh::expose(&r, 0, k);
                let mut e = e;
                let n = e.suffix.len();
                e.suffix.splice(n - k.., d.iter().copied());
                e
            }
        };
        tensor_value(sh::norm_shape(&r, &st.tf))
    }

    /// Convolution / pooling over the last `n` dims; `cin`/`cout` None: channels kept (pools).
    #[allow(clippy::too_many_arguments)]
    pub fn t_conv(&mut self, s: &Shape, n: usize, cin: Option<DimId>, cout: Option<DimId>, k: &[DimId], stride: &[DimId], pad: Option<&[DimId]>, dil: &[DimId], out_pad: Option<&[DimId]>, ceil: bool, node: &Node, st: &mut State, what: &str) -> Value {
        let rel = RankRel::In(vec![n as u32 + 1, n as u32 + 2]);
        if !self.t_require_rank(s, rel, node, st, what) {
            return Value::new();
        }
        let s = sh::norm_shape(s, &st.tf);
        // convolutions look at the channel dim too; pools only at the spatial ones
        let first = if cin.is_some() { n as i64 + 1 } else { n as i64 };
        let Some((e, pc)) = self.t_locate(&s, -first, node, st, what) else { return Value::new() };
        if let Some(ci) = cin {
            let got = get_at(&e, pc);
            let w = what.to_string();
            let co = cout.unwrap_or_else(sh::unknown);
            if !self.t_require(Cons::Eq(got, ci), node, st, move || {
                let g = sh::dim_text(got);
                format!("{}({} -> {}) given {} channel{}", w, sh::dim_text(ci), sh::dim_text(co), g, if g == "1" { "" } else { "s" })
            }) {
                return Value::new();
            }
        }
        let e = sh::norm_shape(&e, &st.tf);
        let mut out: Vec<DimId> = Vec::new();
        if cin.is_some() {
            out.push(cout.unwrap_or_else(|| get_at(&e, pc)));
        }
        for j in 0..n {
            let h = sh::dim_at(&e, -((n - j) as i64)).unwrap_or_else(sh::unknown);
            let o = match (out_pad, pad) {
                (Some(op), p) => {
                    // (H-1)s - 2p + d(k-1) + op + 1
                    let p = p.map(|p| p[j]).unwrap_or(sh::lit(0));
                    let t = sh::mul(sh::sub(h, sh::lit(1)), stride[j]);
                    let t = sh::sub(t, sh::mul(sh::lit(2), p));
                    let t = sh::add(t, sh::mul(dil[j], sh::sub(k[j], sh::lit(1))));
                    sh::add(sh::add(t, op[j]), sh::lit(1))
                }
                (None, None) => h, // padding="same"
                (None, Some(p)) => {
                    // (H + 2p - d(k-1) - 1) // s + 1
                    let num = sh::sub(sh::sub(sh::add(h, sh::mul(sh::lit(2), p[j])), sh::mul(dil[j], sh::sub(k[j], sh::lit(1)))), sh::lit(1));
                    if ceil {
                        sh::add(sh::ceildiv(num, stride[j]), sh::lit(1))
                    } else {
                        sh::add(sh::floordiv(num, stride[j]), sh::lit(1))
                    }
                }
            };
            let o = sh::norm(o, &st.tf);
            if let Some(v) = sh::as_lit(o) {
                if v < 1 {
                    let m = format!("{}: output size {} is too small (input {})", what, v, self.t_print(st, || sh::dim_text(h)));
                    self.t_error(node, m, st);
                    return Value::new();
                }
            }
            out.push(o);
        }
        // replace channel + spatial dims (the last n+1)
        let mut r = e.clone();
        match pc {
            Pos::F(i) => {
                r.prefix.truncate(i);
                r.prefix.extend(out);
                if r.rest.is_some() {
                    r.suffix.clear();
                }
            }
            Pos::B(i) => {
                r.suffix.truncate(i);
                r.suffix.extend(out);
            }
        }
        tensor_value(r)
    }

    pub fn t_adaptive(&mut self, s: &Shape, n: usize, out: &Value, node: &Node, st: &mut State, what: &str) -> Value {
        let rel = RankRel::In(vec![n as u32 + 1, n as u32 + 2]);
        if !self.t_require_rank(s, rel, node, st, what) {
            return Value::new();
        }
        let s = sh::norm_shape(s, &st.tf);
        let Some((e, _)) = self.t_locate(&s, -(n as i64), node, st, what) else { return Value::new() };
        let items: Vec<Option<DimId>> = match self.t_seq(out, st) {
            Some(xs) => xs.iter().map(|x| if self.t_is_none(x) { None } else { Some(self.t_dim(x, st).unwrap_or_else(sh::unknown)) }).collect(),
            None => vec![Some(self.t_dim(out, st).unwrap_or_else(sh::unknown)); n],
        };
        let mut r = e;
        for j in 0..n {
            let pos = -((n - j) as i64);
            let Some(d) = items.get(j).copied().flatten() else { continue };
            r = match r.rank() {
                Some(rk) => replace_at(&Shape::known(r.dims().unwrap()), Pos::F((rk as i64 + pos) as usize), &[d]),
                None => {
                    let l = r.suffix.len();
                    replace_at(&r, Pos::B((l as i64 + pos) as usize), &[d])
                }
            };
        }
        tensor_value(r)
    }

    pub fn t_interpolate(&mut self, s: &Shape, size: &Value, sf: &Value, node: &Node, st: &mut State) -> Value {
        let s = sh::norm_shape(s, &st.tf);
        let n = match (self.t_seq(size, st), self.t_seq(sf, st), s.rank()) {
            (Some(v), _, _) => v.len(),
            (_, Some(v), _) => v.len(),
            (_, _, Some(r)) if r >= 3 => r - 2,
            _ => 2,
        };
        let Some((e, _)) = self.t_locate(&s, -(n as i64), node, st, "interpolate") else { return Value::new() };
        let new: Vec<DimId> = if !(size.kv.is_bot() && size.pts.is_empty()) && !self.t_is_none(size) {
            self.t_tuple_dims(size, n, 1, st)
        } else {
            let factors: Vec<Value> = self.t_seq(sf, st).unwrap_or_else(|| vec![sf.clone(); n]);
            (0..n)
                .map(|j| {
                    let h = sh::dim_at(&e, -((n - j) as i64)).unwrap_or_else(sh::unknown);
                    let f = &factors[j.min(factors.len() - 1)];
                    if let Some(k) = self.t_int(f) {
                        return sh::mul(h, sh::lit(k));
                    }
                    if let Some(fl) = f.kv.as_lit().and_then(|k| if let intern::Key::Float(b) = intern::key_get(k) { Some(f64::from_bits(b)) } else { None }) {
                        if fl.fract() == 0.0 {
                            return sh::mul(h, sh::lit(fl as i64));
                        }
                        if let Some(v) = sh::as_lit(h) {
                            return sh::lit((v as f64 * fl).floor() as i64);
                        }
                        let inv = 1.0 / fl;
                        if inv.fract() == 0.0 {
                            return sh::floordiv(h, sh::lit(inv as i64));
                        }
                    }
                    sh::unknown()
                })
                .collect()
        };
        let mut r = e;
        for (j, d) in new.into_iter().enumerate() {
            let pos = -((n - j) as i64);
            r = match r.rank() {
                Some(rk) => replace_at(&Shape::known(r.dims().unwrap()), Pos::F((rk as i64 + pos) as usize), &[d]),
                None => {
                    let l = r.suffix.len();
                    replace_at(&r, Pos::B((l as i64 + pos) as usize), &[d])
                }
            };
        }
        tensor_value(r)
    }

    fn t_rnn(&mut self, o: u32, ty: &str, s0: &Shape, node: &Node, st: &mut State) -> Value {
        let i = self.t_cfg_dim(o, "input_size", st);
        let h = self.t_cfg_dim(o, "hidden_size", st);
        let lv = self.t_cfg(o, "num_layers", st);
        let layers = if lv.kv.is_bot() && lv.pts.is_empty() { sh::lit(1) } else { self.t_dim(&lv, st).unwrap_or_else(sh::unknown) };
        let bf = self.t_bool(&self.t_cfg(o, "batch_first", st)).unwrap_or(false);
        let bv = self.t_cfg(o, "bidirectional", st);
        let bi = if bv.kv.is_bot() && bv.pts.is_empty() { Some(false) } else { self.t_bool(&bv) };
        let dirs = match bi {
            Some(true) => sh::lit(2),
            Some(false) => sh::lit(1),
            None => sh::unknown(),
        };
        if !self.t_require_rank(s0, RankRel::In(vec![2, 3]), node, st, ty) {
            return Value::new();
        }
        let s1 = sh::norm_shape(s0, &st.tf);
        let Some((e, p)) = self.t_locate(&s1, -1, node, st, ty) else { return Value::new() };
        let w = ty.to_string();
        let got = get_at(&e, p);
        if !self.t_require(Cons::Eq(got, i), node, st, move || format!("{} expects input size {}, got {}", w, sh::dim_text(i), sh::dim_text(got))) {
            return Value::new();
        }
        let e = sh::norm_shape(&e, &st.tf);
        let out = tensor_value(replace_at(&e, p, &[sh::mul(dirs, h)]));
        let dl = sh::mul(dirs, layers);
        let hn_shape = match e.rank() {
            Some(3) => {
                let d = e.dims().unwrap();
                Shape::known(vec![dl, if bf { d[0] } else { d[1] }, h])
            }
            Some(_) => Shape::known(vec![dl, h]),
            None => {
                // 2 or 3 dims: [*R, T, I] (batch first) or [T, *R, I], R the batch (or nothing):
                // h_n is [D*L, *R, H]
                let r = if bf {
                    let x = sh::expose(&e, 0, 2);
                    let n = x.suffix.len();
                    sh::merge_rest(&x.prefix, x.rest.as_ref().unwrap(), &x.suffix[..n - 2])
                } else {
                    match &e.rest {
                        Some(Rest::Slice(sym, 0, back)) if e.prefix.is_empty() => Rest::Slice(*sym, 1, *back + e.suffix.len() as u32 - 1).clone(),
                        _ => Rest::Any,
                    }
                };
                match r {
                    Rest::Any => sh::join_shapes(&Shape::known(vec![dl, sh::unknown(), h]), &Shape::known(vec![dl, h])),
                    r => Shape { prefix: vec![dl], rest: Some(r), suffix: vec![h], assume: vec![] },
                }
            }
        };
        let hn = tensor_value(hn_shape);
        if ty == "LSTM" {
            let pair = self.t_tuple_role(vec![hn.clone(), hn], node, st, None, "hc");
            return self.t_tuple(vec![out, pair], node, st, None);
        }
        self.t_tuple(vec![out, hn], node, st, None)
    }

    fn t_mha(&mut self, o: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, node: &Node, st: &mut State) -> Value {
        let get = |i: usize, n: &str| args.get(i).cloned().or_else(|| kwargs.get(&s(n)).cloned());
        let e_dim = self.t_cfg_dim(o, "embed_dim", st);
        let heads = self.t_cfg_dim(o, "num_heads", st);
        let bf = self.t_bool(&self.t_cfg(o, "batch_first", st)).unwrap_or(false);
        let q = get(0, "query").and_then(|v| self.t_shape(&v, st)).unwrap_or_else(Shape::any);
        let k = get(1, "key").and_then(|v| self.t_shape(&v, st)).unwrap_or_else(Shape::any);
        let need = get(4, "need_weights").and_then(|v| self.t_bool(&v)).unwrap_or(true);
        let avg = kwargs.get(&s("average_attn_weights")).and_then(|v| self.t_bool(v)).unwrap_or(true);
        if !self.t_require_rank(&q, RankRel::In(vec![2, 3]), node, st, "MultiheadAttention") {
            return Value::new();
        }
        let q1 = sh::norm_shape(&q, &st.tf);
        let Some((qe, qp)) = self.t_locate(&q1, -1, node, st, "MultiheadAttention") else { return Value::new() };
        let got = get_at(&qe, qp);
        if !self.t_require(Cons::Eq(got, e_dim), node, st, move || format!("MultiheadAttention expects embed dim {}, got {}", sh::dim_text(e_dim), sh::dim_text(got))) {
            return Value::new();
        }
        let qn = sh::norm_shape(&q, &st.tf);
        let kn = sh::norm_shape(&k, &st.tf);
        let out = tensor_value(qn.clone());
        let weights = if !need {
            self.none_v.clone()
        } else {
            let mk = |batched: bool| -> Shape {
                let qd = |i: i64| sh::dim_at(&qn, i).unwrap_or_else(sh::unknown);
                let kd = |i: i64| sh::dim_at(&kn, i).unwrap_or_else(sh::unknown);
                if batched {
                    let (n, l, s2) = if bf { (qd(-3), qd(-2), kd(-2)) } else { (qd(-2), qd(-3), kd(-3)) };
                    if avg { Shape::known(vec![n, l, s2]) } else { Shape::known(vec![n, heads, l, s2]) }
                } else {
                    let (l, s2) = (qd(-2), kd(-2));
                    if avg { Shape::known(vec![l, s2]) } else { Shape::known(vec![heads, l, s2]) }
                }
            };
            tensor_value(match qn.rank() {
                Some(3) => mk(true),
                Some(2) => mk(false),
                _ => sh::join_shapes(&mk(true), &mk(false)),
            })
        };
        self.t_tuple(vec![out, weights], node, st, None)
    }

    pub fn t_loss(&mut self, f: &str, inp: &Shape, tgt: &Shape, reduction: &str, node: &Node, st: &mut State) -> Value {
        let inp = sh::norm_shape(inp, &st.tf);
        let tgt = sh::norm_shape(tgt, &st.tf);
        let unreduced = match f {
            "cross_entropy" | "nll_loss" => {
                match (inp.dims(), tgt.dims()) {
                    (Some(a), Some(b)) if a.len() == b.len() + 1 && a.len() >= 2 => {
                        // class indices: target = input without the class dim (1)
                        let mut want = a.clone();
                        want.remove(1);
                        for (x, y) in want.iter().zip(b.iter()) {
                            if !self.t_require_eq(*x, *y, node, st, f) {
                                return Value::new();
                            }
                        }
                        Shape::known(sh::norm_shape(&Shape::known(want), &st.tf).dims().unwrap())
                    }
                    (Some(a), Some(b)) if a.len() == b.len() => {
                        for (x, y) in a.iter().zip(b.iter()) {
                            if !self.t_require_eq(*x, *y, node, st, f) {
                                return Value::new();
                            }
                        }
                        let mut w = a.clone();
                        if w.len() >= 2 {
                            w.remove(1);
                        } else {
                            w.clear();
                        }
                        Shape::known(w)
                    }
                    (Some(a), Some(b)) if a.len() == 1 && b.is_empty() => Shape::known(vec![]),
                    _ => tgt.clone(),
                }
            }
            _ => match sh::broadcast(&inp, &tgt, &st.tf) {
                Ok(x) => x,
                Err(_) => return self.t_broadcast(&inp, &tgt, node, st, f),
            },
        };
        if reduction == "none" {
            tensor_value(unreduced)
        } else {
            tensor_value(Shape::known(vec![]))
        }
    }

    /// `torch.nn.functional.<name>(…)`
    pub fn t_functional(&mut self, name: &str, c: &Call, st: &mut State) -> Value {
        let node = c.node;
        let x = c.arg(0, Some("input")).unwrap_or_default();
        let s0 = self.t_shape(&x, st).unwrap_or_else(Shape::any);
        let rest: Vec<Value> = c.args.iter().skip(1).cloned().collect();
        if IDENTITY.contains(&name) || ELEMENTWISE_LAYERS.iter().any(|l| l.to_lowercase() == name) || matches!(name, "relu" | "gelu" | "silu" | "mish" | "elu" | "selu" | "celu" | "leaky_relu" | "hardtanh" | "softplus" | "sigmoid" | "tanh" | "dropout" | "dropout1d" | "dropout2d" | "alpha_dropout" | "hardswish" | "logsigmoid" | "relu6" | "softsign" | "threshold" | "gumbel_softmax" | "prelu" | "rrelu") {
            return tensor_value(s0);
        }
        match name {
            "softmax" | "log_softmax" | "softmin" | "normalize" => {
                let d = c.arg(if name == "normalize" { 2 } else { 1 }, Some("dim")).and_then(|v| self.t_int(&v)).or(if name == "normalize" { Some(1) } else { None });
                if let Some(d) = d {
                    if self.t_locate(&s0, d, &node, st, name).is_none() {
                        return Value::new();
                    }
                }
                tensor_value(sh::norm_shape(&s0, &st.tf))
            }
            "linear" => {
                let w = c.arg(1, Some("weight")).and_then(|v| self.t_shape(&v, st)).unwrap_or_else(Shape::any);
                let w = sh::norm_shape(&w, &st.tf);
                let (out, i) = match (w.dims(), &w.rest) {
                    (Some(d), _) if d.len() == 2 => (d[0], d[1]),
                    (Some(d), _) if d.len() == 1 => {
                        return self.t_matmul(&s0, &w, &node, st, "linear");
                    }
                    (None, Some(Rest::Slice(sym, 0, 0))) if w.prefix.is_empty() && w.suffix.is_empty() => (sh::sdim(*sym, 0), sh::sdim(*sym, 1)),
                    _ => (sh::unknown(), sh::unknown()),
                };
                self.t_linear(&s0, i, out, &node, st, "linear")
            }
            "bilinear" => tensor_value(Shape::any()),
            "embedding" => {
                let w = c.arg(1, Some("weight")).and_then(|v| self.t_shape(&v, st)).unwrap_or_else(Shape::any);
                let w = sh::norm_shape(&w, &st.tf);
                let d = sh::dim_at(&w, -1).unwrap_or_else(sh::unknown);
                tensor_value(append_dims(&s0, &[d]))
            }
            "one_hot" => {
                let n = c.arg(1, Some("num_classes")).and_then(|v| self.t_dim(&v, st)).filter(|d| sh::as_lit(*d) != Some(-1)).unwrap_or_else(sh::unknown);
                tensor_value(append_dims(&s0, &[n]))
            }
            "layer_norm" | "rms_norm" => {
                let shp = c.arg(1, Some("normalized_shape")).and_then(|v| self.t_shape_arg(&v, st)).unwrap_or_else(Shape::any);
                self.t_layer_norm(&s0, &shp, &node, st, name)
            }
            "batch_norm" | "instance_norm" | "group_norm" | "local_response_norm" => tensor_value(s0),
            "pad" => {
                let Some(p) = c.arg(1, Some("pad")).and_then(|v| self.t_seq(&v, st)) else { return tensor_value(Shape::any()) };
                let pads: Vec<DimId> = p.iter().map(|v| self.t_dim(v, st).unwrap_or_else(sh::unknown)).collect();
                let k = pads.len() / 2;
                let Some((e, _)) = self.t_locate(&s0, -(k as i64), &node, st, "pad") else { return Value::new() };
                let mut r = if e.rank().is_some() { Shape::known(e.dims().unwrap()) } else { e };
                for j in 0..k {
                    let pos = -(j as i64 + 1);
                    let d = sh::dim_at(&r, pos).unwrap_or_else(sh::unknown);
                    let nd = sh::add(d, sh::add(pads[2 * j], pads[2 * j + 1]));
                    r = match r.rank() {
                        Some(rk) => replace_at(&r, Pos::F((rk as i64 + pos) as usize), &[nd]),
                        None => {
                            let l = r.suffix.len();
                            replace_at(&r, Pos::B((l as i64 + pos) as usize), &[nd])
                        }
                    };
                }
                tensor_value(r)
            }
            "interpolate" | "upsample" | "upsample_nearest" | "upsample_bilinear" => {
                let size = c.arg(1, Some("size")).unwrap_or_default();
                let sf = c.arg(2, Some("scale_factor")).unwrap_or_default();
                self.t_interpolate(&s0, &size, &sf, &node, st)
            }
            "conv1d" | "conv2d" | "conv3d" | "conv_transpose1d" | "conv_transpose2d" => {
                let n = name.chars().rev().nth(1).and_then(|c| c.to_digit(10)).unwrap_or(2) as usize;
                let w = c.arg(1, Some("weight")).and_then(|v| self.t_shape(&v, st)).unwrap_or_else(Shape::any);
                let mut w = sh::norm_shape(&w, &st.tf);
                if w.rank().is_none() {
                    if let Some(Rest::Slice(sym, 0, 0)) = &w.rest {
                        if w.prefix.is_empty() && w.suffix.is_empty() {
                            w = Shape::known((0..n as i32 + 2).map(|i| sh::sdim(*sym, i)).collect());
                        }
                    }
                }
                let Some(wd) = w.dims().filter(|d| d.len() == n + 2) else { return tensor_value(Shape::any()) };
                let transposed = name.contains("transpose");
                let stride = self.t_tuple_dims(&c.arg(if transposed { 3 } else { 3 }, Some("stride")).unwrap_or_default(), n, 1, st);
                let pv = c.arg(4, Some("padding")).unwrap_or_default();
                let pad = match self.t_str(&pv).as_deref() {
                    Some("same") => None,
                    Some("valid") => Some(vec![sh::lit(0); n]),
                    _ => Some(self.t_tuple_dims(&pv, n, 0, st)),
                };
                let dil = self.t_tuple_dims(&c.arg(if transposed { 7 } else { 5 }, Some("dilation")).unwrap_or_default(), n, 1, st);
                let gv = c.arg(6, Some("groups")).unwrap_or_default();
                let groups = if gv.kv.is_bot() && gv.pts.is_empty() { sh::lit(1) } else { self.t_dim(&gv, st).unwrap_or_else(sh::unknown) };
                let (cin, cout) = if transposed { (wd[0], sh::mul(wd[1], groups)) } else { (sh::mul(wd[1], groups), wd[0]) };
                let op = self.t_tuple_dims(&c.arg(5, Some("output_padding")).unwrap_or_default(), n, 0, st);
                self.t_conv(&s0, n, Some(cin), Some(cout), &wd[2..], &stride, pad.as_deref(), &dil, if transposed { Some(&op) } else { None }, false, &node, st, name)
            }
            "max_pool1d" | "max_pool2d" | "max_pool3d" | "avg_pool1d" | "avg_pool2d" | "avg_pool3d" => {
                let n = name.chars().rev().nth(1).and_then(|c| c.to_digit(10)).unwrap_or(2) as usize;
                let k = self.t_tuple_dims(&c.arg(1, Some("kernel_size")).unwrap_or_default(), n, 1, st);
                let sv = c.arg(2, Some("stride")).unwrap_or_default();
                let stride = if sv.kv.is_bot() && sv.pts.is_empty() || self.t_is_none(&sv) { k.clone() } else { self.t_tuple_dims(&sv, n, 1, st) };
                let pad = self.t_tuple_dims(&c.arg(3, Some("padding")).unwrap_or_default(), n, 0, st);
                let dil = if name.starts_with("max") { self.t_tuple_dims(&c.arg(4, Some("dilation")).unwrap_or_default(), n, 1, st) } else { vec![sh::lit(1); n] };
                let ceil = kw(c, "ceil_mode").and_then(|v| self.t_bool(v)).unwrap_or(false);
                self.t_conv(&s0, n, None, None, &k, &stride, Some(&pad), &dil, None, ceil, &node, st, name)
            }
            "adaptive_avg_pool1d" | "adaptive_avg_pool2d" | "adaptive_avg_pool3d" | "adaptive_max_pool1d" | "adaptive_max_pool2d" => {
                let n = name.chars().rev().nth(1).and_then(|c| c.to_digit(10)).unwrap_or(2) as usize;
                let out = c.arg(1, Some("output_size")).unwrap_or_default();
                self.t_adaptive(&s0, n, &out, &node, st, name)
            }
            "cross_entropy" | "nll_loss" | "mse_loss" | "l1_loss" | "smooth_l1_loss" | "huber_loss" | "binary_cross_entropy" | "binary_cross_entropy_with_logits" | "kl_div" | "soft_margin_loss" | "cosine_embedding_loss" => {
                let t = c.arg(1, Some("target")).and_then(|v| self.t_shape(&v, st)).unwrap_or_else(Shape::any);
                let red = kw(c, "reduction").and_then(|v| self.t_str(v)).unwrap_or_else(|| "mean".into());
                let f = if matches!(name, "cross_entropy" | "nll_loss") { name } else { "mse_loss" };
                self.t_loss(f, &s0, &t, &red, &node, st)
            }
            "scaled_dot_product_attention" => {
                let k = c.arg(1, Some("key")).and_then(|v| self.t_shape(&v, st)).unwrap_or_else(Shape::any);
                let v = c.arg(2, Some("value")).and_then(|v| self.t_shape(&v, st)).unwrap_or_else(Shape::any);
                let Some((qe, _)) = self.t_locate(&s0, -2, &node, st, name) else { return Value::new() };
                let Some((ke, _)) = self.t_locate(&k, -2, &node, st, name) else { return Value::new() };
                let Some((ve, _)) = self.t_locate(&v, -2, &node, st, name) else { return Value::new() };
                let (qd, kd) = (sh::dim_at(&qe, -1).unwrap(), sh::dim_at(&ke, -1).unwrap());
                if !self.t_require_eq(qd, kd, &node, st, "scaled_dot_product_attention head dim") {
                    return Value::new();
                }
                let (kl, vl) = (sh::dim_at(&ke, -2).unwrap(), sh::dim_at(&ve, -2).unwrap());
                if !self.t_require_eq(kl, vl, &node, st, "scaled_dot_product_attention key/value length") {
                    return Value::new();
                }
                let qe = sh::norm_shape(&qe, &st.tf);
                let ve = sh::norm_shape(&ve, &st.tf);
                let ev = sh::dim_at(&ve, -1).unwrap();
                let r = match qe.rank() {
                    Some(r) => replace_at(&Shape::known(qe.dims().unwrap()), Pos::F(r - 1), &[ev]),
                    None => {
                        let l = qe.suffix.len();
                        replace_at(&qe, Pos::B(l - 1), &[ev])
                    }
                };
                tensor_value(r)
            }
            "unfold" | "fold" | "grid_sample" | "affine_grid" | "pixel_shuffle" | "pixel_unshuffle" => tensor_value(Shape::any()),
            "cosine_similarity" | "pairwise_distance" => {
                let o = c.arg(1, Some("x2")).and_then(|v| self.t_shape(&v, st)).unwrap_or_else(Shape::any);
                let b = match sh::broadcast(&s0, &o, &st.tf) {
                    Ok(b) => b,
                    Err(_) => return self.t_broadcast(&s0, &o, &node, st, name),
                };
                let d = c.arg(2, Some("dim")).and_then(|v| self.t_int(&v)).unwrap_or(1);
                match self.t_reduce(&b, Some(vec![d]), false, &node, st, name) {
                    Some(r) => tensor_value(r),
                    None => Value::new(),
                }
            }
            _ => {
                if self.t_shape(&x, st).is_some() && TENSOR_METHODS.contains(&name) {
                    return self.t_method(name, &x, &rest, c, st);
                }
                tensor_value(Shape::any())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// registry entry points
// ---------------------------------------------------------------------------

/// `*args` / `**kwargs` with known contents become ordinary arguments; Err: some cannot be
/// seen through (the result is then not known).
fn expand_call(an: &Analyzer, c: &Call, st: &State) -> Result<Option<Call>, ()> {
    if c.star_args.is_empty() && c.star_kwargs.is_empty() {
        return Ok(None);
    }
    let mut args = c.args.clone();
    let mut star_left = Vec::new();
    for sa in &c.star_args {
        match an.t_seq(sa, st) {
            // only when the starred part comes last (its order is then clear)
            Some(items) if c.call_ast.map(|ca| matches!(ca.arguments.args.last(), Some(ruff_python_ast::Expr::Starred(_)))).unwrap_or(true) && c.star_args.len() == 1 => args.extend(items),
            _ => star_left.push(sa.clone()),
        }
    }
    let keep_ast = if star_left.len() == c.star_args.len() { c.call_ast } else { None };
    if star_left.len() == c.star_args.len() && c.star_kwargs.is_empty() {
        // nothing to expand (sizes like `*x.shape` are read in source order later)
        return Ok(None);
    }
    let mut kwargs = c.kwargs.clone();
    for sk in &c.star_kwargs {
        let Some(d) = sk.single().filter(|d| an.kind(*d) == Kind::Dict) else { return Err(()) };
        let row = st.row(d, Ns::Item);
        if !row.closed() || row.present != row.definite {
            return Err(());
        }
        for k in row.definite.iter() {
            let Some(name) = intern::key_as_str(k) else { return Err(()) };
            kwargs.insert(s(&name), st.slot(d, Ns::Item, k));
        }
    }
    Ok(Some(Call { qual: c.qual.clone(), recv: c.recv.clone(), args, kwargs, star_args: star_left, star_kwargs: vec![], node: c.node, call_ast: keep_ast, site: c.site.clone(), line: c.line }))
}

/// `torch.*` (functions, `nn` constructors, `nn.functional`, `nn.init`).
pub fn m_torch(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let expanded = match expand_call(an, c, st) {
        Ok(e) => e,
        Err(()) => {
            if c.qual.starts_with("torch.nn.") && !c.qual.starts_with("torch.nn.functional.") {
                return Value::data(&format!("L{}", c.line));
            }
            return tensor_value(Shape::any());
        }
    };
    let c = expanded.as_ref().unwrap_or(c);
    let q = c.qual.as_str();
    if let Some(name) = q.strip_prefix("torch.nn.functional.") {
        return an.t_functional(name, c, st);
    }
    if q.starts_with("torch.nn.init.") {
        return c.args.first().cloned().unwrap_or_default();
    }
    if q.starts_with("torch.nn.utils.") || q.starts_with("torch.optim.") || q.starts_with("torch.utils.") || q.starts_with("torch.cuda.") || q.starts_with("torch.backends.") || q.starts_with("torch.distributed.") {
        return Value::data(&format!("L{}", c.line));
    }
    if let Some(ty) = q.strip_prefix("torch.nn.") {
        let ty = ty.rsplit('.').next().unwrap_or(ty);
        return an.t_layer_new(ty, c, st);
    }
    if let Some(name) = q.strip_prefix("torch.Tensor.") {
        let Some(recv) = c.args.first().cloned() else { return tensor_value(Shape::any()) };
        let rest: Vec<Value> = c.args[1..].to_vec();
        return an.t_method(name, &recv, &rest, c, st);
    }
    if let Some(name) = q.strip_prefix("torch.linalg.") {
        let _ = name;
        return tensor_value(Shape::any());
    }
    if let Some(name) = q.strip_prefix("torch.") {
        if name.contains('.') {
            return Value::data(&format!("L{}", c.line));
        }
        return an.t_torch_fn(name, c, st);
    }
    Value::data(&format!("L{}", c.line))
}

/// `tensor.<method>(…)`
pub fn m_tensor_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let expanded = match expand_call(an, c, st) {
        Ok(e) => e,
        Err(()) => return tensor_value(Shape::any()),
    };
    let c = expanded.as_ref().unwrap_or(c);
    let name = c.qual.strip_prefix("tensor.").unwrap_or("");
    let recv = c.recv.clone().unwrap_or_default();
    let args = c.args.clone();
    an.t_method(name, &recv, &args, c, st)
}

/// `self.register_buffer(…)` & co. on a user `nn.Module`.
pub fn m_nnmodule(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.strip_prefix("nnmodule.").unwrap_or("");
    let recv = c.recv.clone().unwrap_or_default();
    match name {
        "register_buffer" | "register_parameter" | "add_module" | "register_module" => {
            let (Some(n), Some(v)) = (c.arg(0, Some("name")).and_then(|v| an.t_str(&v)), c.arg(1, None).or_else(|| kw(c, "tensor").cloned()).or_else(|| kw(c, "param").cloned()).or_else(|| kw(c, "module").cloned())) else {
                return an.none_v.clone();
            };
            let node = c.node;
            an.store_attr(&recv, &n, &v, &node, st);
            an.none_v.clone()
        }
        "to" | "cuda" | "cpu" | "eval" | "train" | "requires_grad_" | "half" | "float" | "double" | "bfloat16" | "type" | "share_memory" | "apply" => recv,
        _ => Value::data(&format!("L{}", c.line)),
    }
}

impl Analyzer {
    /// Unpacking a size / tensor into `n` targets (`star`: index of a starred target, which
    /// takes a size / list of the middle). None: not ours.
    pub fn t_unpack(&mut self, v: &Value, n: usize, star: Option<usize>, node: &Node, st: &mut State) -> Option<Vec<Value>> {
        let fixed = n - star.map(|_| 1).unwrap_or(0);
        let (shape, is_size) = if let Some(s) = size_shape(v) {
            (s, true)
        } else if let Some(o) = v.single() {
            if matches!(&self.obj(o).payload, Payload::Str(q) if q == "torch.Size") {
                let len = self.obj(o).literal_len?;
                if star.is_none() && len != n || star.is_some() && len < fixed {
                    let m = format!("cannot unpack a torch.Size of {} dims into {} names", len, n);
                    self.t_error(node, m, st);
                    return Some(vec![Value::new(); n]);
                }
                return None;
            }
            return None;
        } else if is_tensor(v) {
            // iterating a tensor: its rows (dim 0)
            let s = self.t_shape(v, st).unwrap_or_else(Shape::any);
            let Some((e, p)) = self.t_locate(&s, 0, node, st, "unpack") else { return Some(vec![Value::new(); n]) };
            let d = get_at(&e, p);
            if star.is_none() {
                let w = format!("unpacking into {} names", n);
                if !self.t_require_eq(d, sh::lit(n as i64), node, st, &w) {
                    return Some(vec![Value::new(); n]);
                }
            }
            let row = tensor_value(replace_at(&e, p, &[]));
            return Some(vec![row; n]);
        } else {
            return None;
        };
        let _ = is_size;
        let s = sh::norm_shape(&shape, &st.tf);
        match star {
            None => {
                if !self.t_require_rank(&s, RankRel::Eq(n as u32), node, st, "unpacking a shape") {
                    return Some(vec![Value::new(); n]);
                }
                let s = sh::norm_shape(&s, &st.tf);
                let d = s.dims().unwrap_or_else(|| vec![sh::unknown(); n]);
                Some(d.iter().map(|x| dim_value(*x)).collect())
            }
            Some(si) => {
                let after = n - si - 1;
                if !self.t_require_rank(&s, RankRel::Ge(fixed as u32), node, st, "unpacking a shape") {
                    return Some(vec![Value::new(); n]);
                }
                let s = sh::norm_shape(&s, &st.tf);
                let e = sh::expose(&s, si, after);
                let (lead, mid, trail): (Vec<DimId>, Shape, Vec<DimId>) = match e.dims() {
                    Some(d) => {
                        let len = d.len();
                        (d[..si].to_vec(), Shape::known(d[si..len - after].to_vec()), d[len - after..].to_vec())
                    }
                    None => {
                        let l = e.suffix.len();
                        (e.prefix[..si].to_vec(), sh::tidy(Shape { prefix: e.prefix[si..].to_vec(), rest: e.rest.clone(), suffix: e.suffix[..l - after].to_vec(), assume: vec![] }), e.suffix[l - after..].to_vec())
                    }
                };
                let mut out: Vec<Value> = lead.iter().map(|x| dim_value(*x)).collect();
                out.push(self.t_size_value(&mid, node, st));
                out.extend(trail.iter().map(|x| dim_value(*x)));
                Some(out)
            }
        }
    }
}

pub const QUOT_PREFIX: &str = "\u{1}Q";

/// `d / k` (true division of a dim by an int literal): a float, but `int()` / `math.ceil()`
/// of it are dims again.
pub fn quotient_value(d: DimId, k: i64) -> Value {
    Value::data(&format!("{}{}:{}", QUOT_PREFIX, d.0, k))
}

pub fn quotient_of(v: &Value) -> Option<(DimId, i64)> {
    if !v.pts.is_empty() {
        return None;
    }
    let l = label(v)?;
    let (d, k) = l.strip_prefix(QUOT_PREFIX)?.split_once(':')?;
    Some((DimId(d.parse().ok()?), k.parse().ok()?))
}

pub fn dim_of_label(v: &Value) -> Option<DimId> {
    if !v.pts.is_empty() {
        return None;
    }
    sh::parse_dim_label(&label(v)?)
}

/// A tensor for sure (not a tensor-or-int value)
pub fn is_definite_tensor(v: &Value) -> bool {
    is_tensor(v) && label(v).map(|l| sh::dual_dim(&l).is_none()).unwrap_or(false)
}

impl Analyzer {
    pub fn t_compare(&mut self, a: &Value, b: &Value, node: &Node, st: &mut State) -> Value {
        let sa = self.t_shape(a, st).unwrap_or_else(Shape::any);
        match self.t_shape(b, st) {
            Some(sb) => match sh::broadcast(&sa, &sb, &st.tf) {
                Ok(r) => bool_tensor_value(r),
                Err(_) => self.t_broadcast(&sa, &sb, node, st, "comparison"),
            },
            None => bool_tensor_value(sa),
        }
    }
}

impl Analyzer {
    pub fn t_note_alias(&mut self, _obj: &Value, _name: &str, _v: &Value) {}
}

impl Analyzer {
    /// What the golden harness checks for `#@shape` / `#@dim` on a name's value.
    pub fn t_check_json(&self, v: &Value, st: &State) -> serde_json::Value {
        let (chain, entry) = self.t_note_ctx();
        let shape = if let Some(s) = size_shape(v) {
            Some(sh::norm_shape(&s, &st.tf))
        } else if let Some(o) = v.single().filter(|o| matches!(&self.obj(*o).payload, Payload::Str(q) if q == "torch.Size") || (self.kind(*o) == Kind::Tuple && self.obj(*o).literal_len.is_some())) {
            self.t_shape_arg(&Value::obj(o), st)
        } else if tensor_shape(v).is_some() {
            self.t_shape(v, st)
        } else if !v.pts.is_empty() && v.pts.iter().all(|o| matches!(self.kind(o), Kind::Ph | Kind::Unknown)) {
            // an unknown: a shape only once facts say something about it
            self.t_shape(v, st).filter(|s| !(s.prefix.is_empty() && s.suffix.is_empty() && matches!(s.rest, Some(Rest::Slice(_, 0, 0)) | Some(Rest::Any))))
        } else {
            None
        };
        let dual = label(v).map(|l| sh::dual_dim(&l).is_some()).unwrap_or(false);
        let dim = if dim_of_label(v).is_some() || dual || self.t_int(v).is_some() || (!v.pts.is_empty() && v.pts.iter().all(|o| matches!(self.kind(o), Kind::Ph | Kind::Unknown))) {
            self.t_dim(v, st).map(|d| self.t_print(st, || sh::dim_text(d)))
        } else {
            None
        };
        let shape_json = shape.map(|s| self.t_print(st, || sh::shape_json(&s)));
        serde_json::json!({"shape": shape_json, "dim": dim, "chain": chain, "entry": entry})
    }
}

impl Analyzer {
    /// `(*x.shape[:-1], n)`: Ok = one dim, Err = a starred sequence. None: not a size.
    pub fn t_size_from_parts(&mut self, parts: &[Result<Value, Value>], node: &Node, st: &mut State) -> Option<Value> {
        let mut s = Shape::known(vec![]);
        let mut any_size = false;
        for p in parts {
            match p {
                Ok(v) => {
                    let d = self.t_dim(v, st)?;
                    s = append_dims(&s, &[d]);
                }
                Err(v) => {
                    if size_shape(v).is_some() || v.single().map(|o| matches!(&self.obj(o).payload, Payload::Str(q) if q == "torch.Size")).unwrap_or(false) {
                        any_size = true;
                    }
                    let part = self.t_shape_arg(v, st)?;
                    s = concat(&s, &part);
                }
            }
        }
        if !any_size {
            return None;
        }
        Some(self.t_size_value(&s, node, st))
    }

    /// `len(t)` (dim 0) / `len(x.shape)` (the rank).
    pub fn t_len(&mut self, v: &Value, node: &Node, st: &mut State) -> Option<Value> {
        if let Some(d) = self.t_tuple_len(v) {
            return Some(dim_value(d));
        }
        if let Some(s) = size_shape(v) {
            return Some(self.t_rank_value(&sh::norm_shape(&s, &st.tf)));
        }
        if is_tensor(v) || (self.torch_on && !v.pts.is_empty() && v.pts.iter().all(|o| matches!(self.kind(o), Kind::Ph | Kind::Unknown) && self.t_sym_of_obj(o).is_some())) {
            let s = self.t_shape(v, st)?;
            let (e, p) = self.t_locate(&s, 0, node, st, "len")?;
            return Some(dim_value(get_at(&e, p)));
        }
        None
    }

    /// Positional args in source order with starred ones expanded as sizes: for sizes
    /// given as `zeros(*x.shape[:-1], 5)`.
    pub fn t_sizes_call(&self, c: &Call, args: &[Value], st: &State) -> Option<Shape> {
        if c.star_args.is_empty() || args.len() != c.args.len() {
            return self.t_sizes(args, &c.star_args, st);
        }
        let ca = c.call_ast?;
        let mut pi = 0;
        let mut si = 0;
        let mut s = Shape::known(vec![]);
        for a in ca.arguments.args.iter() {
            if matches!(a, ruff_python_ast::Expr::Starred(_)) {
                let v = c.star_args.get(si)?;
                si += 1;
                let part = self.t_shape_arg(v, st).unwrap_or_else(Shape::any);
                s = concat(&s, &part);
            } else {
                let v = args.get(pi)?;
                pi += 1;
                let d = self.t_dim(v, st)?;
                s = append_dims(&s, &[d]);
            }
        }
        Some(s)
    }
}

impl Analyzer {
    /// The rank-of-x expression `x.dim()`, `x.ndim`, `len(x.shape)`, `x.ndimension()`: x.
    fn t_rank_operand(e: &'static ruff_python_ast::Expr) -> Option<&'static ruff_python_ast::Expr> {
        use ruff_python_ast::Expr as E;
        match e {
            E::Call(c) if c.arguments.args.is_empty() => match &*c.func {
                E::Attribute(a) if matches!(a.attr.id.as_str(), "dim" | "ndimension") => Some(&a.value),
                E::Name(n) if n.id.as_str() == "len" => None,
                _ => None,
            },
            E::Call(c) if c.arguments.args.len() == 1 => match (&*c.func, &c.arguments.args[0]) {
                (E::Name(n), E::Attribute(a)) if n.id.as_str() == "len" && a.attr.id.as_str() == "shape" => Some(&a.value),
                (E::Name(n), E::Call(inner)) if n.id.as_str() == "len" && inner.arguments.args.is_empty() => match &*inner.func {
                    E::Attribute(a) if a.attr.id.as_str() == "size" => Some(&a.value),
                    _ => None,
                },
                _ => None,
            },
            E::Attribute(a) if a.attr.id.as_str() == "ndim" => Some(&a.value),
            _ => None,
        }
    }

    /// A test that held (`assert`, the branch of an `if`): dim / rank equalities become facts;
    /// one that is decided false makes the path dead.
    pub fn t_narrow(&mut self, test: &'static ruff_python_ast::Expr, truth: bool, st: &mut State) {
        use ruff_python_ast::{CmpOp, Expr as E};
        let E::Compare(c) = test else { return };
        if c.ops.len() != 1 {
            return;
        }
        let eq = match c.ops[0] {
            CmpOp::Eq => truth,
            CmpOp::NotEq => !truth,
            _ => return,
        };
        if !eq {
            return;
        }
        let (l, r) = (&*c.left, &c.comparators[0]);
        // rank: x.dim() == n
        for (re, ne) in [(l, r), (r, l)] {
            if let Some(x) = Self::t_rank_operand(re) {
                let nv = self.eval(ne, st);
                let Some(n) = self.t_int(&nv) else { return };
                let xv = self.eval(x, st);
                let Some(s) = self.t_shape(&xv, st) else { return };
                self.t_assume_rank(&s, n as u32, st);
                return;
            }
        }
        let a = self.eval(l, st);
        let b = self.eval(r, st);
        let (Some(da), Some(db)) = (self.t_dim(&a, st), self.t_dim(&b, st)) else { return };
        // only facts about dims (a plain int comparison is the interpreter's business)
        let symbolic = |v: &Value| dim_of_label(v).is_some() || label(v).map(|l| sh::dual_dim(&l).is_some()).unwrap_or(false);
        if !symbolic(&a) && !symbolic(&b) {
            return;
        }
        match sh::dim_eq(da, db, &st.tf) {
            Some(true) => {}
            Some(false) => st.reachable = false,
            None => {
                let mut f = (*st.tf).clone();
                sh::learn_eq(&mut f, da, db);
                st.tf = std::sync::Arc::new(f);
            }
        }
    }

    fn t_assume_rank(&mut self, s: &Shape, n: u32, st: &mut State) {
        if let Some(r) = s.rank() {
            if r as u32 != n {
                st.reachable = false;
            }
            return;
        }
        let fixed = (s.prefix.len() + s.suffix.len()) as i64;
        if let Some(Rest::Slice(sym, from, back)) = &s.rest {
            let k = n as i64 - fixed + *from as i64 + *back as i64;
            if k < (*from + *back) as i64 {
                st.reachable = false;
                return;
            }
            let c = Cons::Rank(*sym, RankRel::Eq(k as u32));
            match sh::decide(&c, &st.tf) {
                Some(false) => st.reachable = false,
                Some(true) => {}
                None => {
                    let mut f = (*st.tf).clone();
                    sh::learn(&mut f, &c);
                    st.tf = std::sync::Arc::new(f);
                }
            }
        }
    }
}

impl Analyzer {
    /// `max(a, b)` / `min(a, b)` on dims (None: not ours).
    pub fn t_minmax(&mut self, is_max: bool, args: &[Value], st: &State) -> Option<Value> {
        if args.len() < 2 || !args.iter().any(|a| dim_of_label(a).is_some() || label(a).map(|l| sh::dual_dim(&l).is_some()).unwrap_or(false)) {
            return None;
        }
        let mut acc = self.t_dim(&args[0], st)?;
        for a in &args[1..] {
            let d = self.t_dim(a, st)?;
            acc = if is_max { sh::max(acc, d) } else { sh::min(acc, d) };
        }
        Some(dim_value(acc))
    }

    /// Unary minus on a named unknown (tensor or int).
    pub fn t_neg_named(&self, v: &Value, st: &State) -> Option<Value> {
        if !self.torch_on || !(v.kv.is_bot() && v.pts.len() == 1) {
            return None;
        }
        let o = v.pts.first()?;
        let sy = self.t_sym_of_obj(o)?;
        let s = sh::shape_id(Shape::of_sym(sy));
        let _ = st;
        Some(Value::data(&sh::dual_label(s, sh::scale(sh::named(sy), -1))))
    }

    /// `a < b` & co. where `a` is a named unknown in a torch module: a bool tensor (or bool).
    pub fn t_compare_named(&mut self, a: &Value, b: &Value, node: &Node, st: &mut State) -> Option<Value> {
        if !self.torch_on || !(a.kv.is_bot() && a.pts.len() == 1 && self.t_sym_of_obj(a.pts.first()?).is_some()) {
            return None;
        }
        let numeric = |v: &Value| v.pts.is_empty() && (matches!(&v.kv, KeyVal::Lit(k) if intern::key_is_number(*k)) || tensor_shape(v).is_some() || dim_of_label(v).is_some());
        let named = |an: &Analyzer, v: &Value| v.kv.is_bot() && v.pts.len() == 1 && an.t_sym_of_obj(v.pts.first().unwrap()).is_some();
        if !numeric(b) && !named(self, b) {
            return None;
        }
        Some(self.t_compare(a, b, node, st))
    }
}

impl Analyzer {
    /// `w = load(...)`: an unknown call's result is named by the variable it is bound to
    /// (its dims print as `w.shape[0]`).
    pub fn t_name_result(&mut self, name: StrId, v: &Value) {
        if v.pts.len() != 1 || !v.kv.is_bot() {
            return;
        }
        let o = v.pts.first().unwrap();
        if self.kind(o) != Kind::Unknown || !self.origins.contains_key(&o) || self.t_var_names.contains_key(&o) {
            return;
        }
        let scope = self.frames.iter().rev().find_map(|fr| fr.func).unwrap_or(0);
        self.t_var_names.insert(o, sh::sym(scope, &intern::str_get(name)));
    }
}

impl Analyzer {
    /// `t op= u` in place: the broadcast result must have t's shape (false: an error).
    pub fn t_inplace_fits(&mut self, cur: &Value, res: &Value, node: &Node, st: &mut State) -> bool {
        let (Some(a), Some(r)) = (self.t_shape(cur, st), self.t_shape(res, st)) else { return true };
        let grows = match (r.rank(), a.rank()) {
            (Some(x), Some(y)) => x > y,
            _ => false,
        };
        let differs = match (r.dims(), a.dims()) {
            (Some(x), Some(y)) if x.len() == y.len() => x.iter().zip(y.iter()).any(|(p, q)| sh::dim_eq(*p, *q, &st.tf) == Some(false)),
            _ => false,
        };
        if grows || differs {
            let m = self.t_print(st, || format!("in-place op: the result {} does not fit {}", sh::shape_text(&r), sh::shape_text(&a)));
            self.t_error(node, m, st);
            return false;
        }
        true
    }
}

impl Analyzer {
    /// Any element of a list / tuple without known positions: its objects and the label of
    /// its values (kept apart in the state).
    pub fn t_list_elem(&self, o: u32, st: &State) -> Value {
        let e = st.slot(o, Ns::Elem, STAR);
        match st.elems.get(&o) {
            Some(kv) if !kv.is_bot() => Value::with_kv(e.pts.clone(), kv),
            _ => e,
        }
    }
}

// ---------------------------------------------------------------------------
// hover rendering
// ---------------------------------------------------------------------------

impl Analyzer {
    /// The torch part of a value for the hover (None: not ours): `Tensor[B, T, 4*C]`, a dim
    /// expression, a size.
    /// The shape of a named unknown (a parameter) when facts say something about it.
    pub fn t_known_param_shape(&self, v: &Value, st: &State) -> Option<Shape> {
        if !self.torch_on || !v.kv.is_bot() || v.pts.len() != 1 {
            return None;
        }
        let o = v.pts.first()?;
        let sy = self.t_sym_of_obj(o)?;
        let s = sh::canon(sh::norm_shape(&Shape::of_sym(sy), &st.tf));
        if s == Shape::of_sym(sy) {
            return None;
        }
        Some(s)
    }

    pub fn t_render(&self, v: &Value, st: &State) -> Option<String> {
        if let Some(s) = self.t_known_param_shape(v, st) {
            return Some(tensor_text(&s));
        }
        let l = label(v)?;
        if let Some(sid) = sh::parse_tensor_label(&l) {
            let s = sh::canon(sh::norm_shape(&sh::shape_get(sid), &st.tf));
            if let Some(d) = sh::dual_dim(&l) {
                let d = sh::norm(d, &st.tf);
                let mut atoms = Vec::new();
                sh::dim_atoms(d, &mut atoms);
                // arithmetic on dims: an int; on unknowns: either
                if !atoms.is_empty() || s.is_any() {
                    return Some(sh::dim_text(d));
                }
                return Some(format!("{} | {}", sh::dim_text(d), tensor_text(&s)));
            }
            return Some(tensor_text(&s));
        }
        if let Some(d) = sh::parse_dim_label(&l) {
            return Some(sh::dim_text(sh::norm(d, &st.tf)));
        }
        if &*l == sh::NUM_LABEL {
            return Some("?".into());
        }
        if let Some(s) = size_shape(v) {
            return Some(format!("torch.Size({})", sh::shape_text(&sh::norm_shape(&s, &st.tf))));
        }
        None
    }

    /// Names for the shape dims a value mentions: locals that hold them (`B, T, C = x.shape`),
    /// else fresh capital letters; with the legend lines.
    pub fn t_hover_names(&self, v: &Value, st: &State) -> (FxHashMap<sh::AtomId, String>, Vec<(String, String)>) {
        let mut names: FxHashMap<sh::AtomId, String> = FxHashMap::default();
        let mut legend: Vec<(String, String)> = Vec::new();
        let mut atoms: Vec<sh::AtomId> = Vec::new();
        let mut collect = |val: &Value| {
            if let Some(l) = label(val) {
                if let Some(sid) = sh::parse_tensor_label(&l) {
                    sh::shape_atoms(&sh::norm_shape(&sh::shape_get(sid), &st.tf), &mut atoms);
                }
                if let Some(d) = sh::parse_dim_label(&l).or_else(|| sh::dual_dim(&l)) {
                    sh::dim_atoms(sh::norm(d, &st.tf), &mut atoms);
                }
                if let Some(sz) = size_shape(val) {
                    sh::shape_atoms(&sh::norm_shape(&sz, &st.tf), &mut atoms);
                }
            }
        };
        collect(v);
        for o in v.pts.iter() {
            if matches!(self.kind(o), Kind::Tuple | Kind::List) {
                if let Some(n) = self.obj(o).literal_len {
                    for i in 0..n.min(8) {
                        collect(&st.slot(o, Ns::Pos, key_int(i as i64)));
                    }
                }
            }
        }
        // a named unknown with shape facts (a parameter after `B, T, C = x.shape`)
        if let Some(s) = self.t_known_param_shape(v, st) {
            sh::shape_atoms(&s, &mut atoms);
        }
        // locals holding exactly one shape dim (also used for requirement texts)
        let mut used: Vec<String> = Vec::new();
        for (n, val) in st.env.iter() {
            let name = intern::str_get(*n).to_string();
            used.push(name.clone());
            if let Some(d0) = dim_of_label(val) {
                // the dim as bound and as it is now (a fact may have rewritten it)
                for d in [d0, sh::norm(d0, &st.tf)] {
                    let p = sh::poly(d);
                    if let [(m, 1)] = &p[..] {
                        if m.len() == 1 && m[0].1 == 1 && matches!(sh::atom_get(m[0].0), sh::Atom::Dim(..)) && !names.contains_key(&m[0].0) {
                            names.insert(m[0].0, name.clone());
                        }
                    }
                }
            }
        }
        if atoms.is_empty() {
            return (names, legend);
        }
        let mut letters = (b'A'..=b'Z').map(|c| (c as char).to_string()).filter(|c| !used.contains(c));
        for a in &atoms {
            if !names.contains_key(a) {
                if let Some(l) = letters.next() {
                    names.insert(*a, l);
                }
            }
        }
        // legend: per tensor whose dims are named, `B, T, C: x.shape` when all its dims are
        let mut by_sym: Vec<(sh::Sym, Vec<(i32, sh::AtomId)>)> = Vec::new();
        for a in &atoms {
            if let sh::Atom::Dim(sy, i) = sh::atom_get(*a) {
                match by_sym.iter_mut().find(|(s0, _)| *s0 == sy) {
                    Some((_, v)) => v.push((i, *a)),
                    None => by_sym.push((sy, vec![(i, *a)])),
                }
            }
        }
        for (sy, mut ds) in by_sym {
            let Some(base) = sh::sym_print(sy) else { continue };
            ds.sort();
            let rank = st.tf.rank_of(sy);
            let full = rank.map(|r| ds.len() == r as usize && ds.iter().enumerate().all(|(k, (i, _))| *i == k as i32)).unwrap_or(false);
            if full {
                let ns: Vec<String> = ds.iter().map(|(_, a)| names[a].clone()).collect();
                legend.push((ns.join(", "), format!("{}.shape", base)));
            } else {
                for (i, a) in ds {
                    legend.push((names[&a].clone(), format!("{}.shape[{}]", base, i)));
                }
            }
        }
        (names, legend)
    }
}

pub fn tensor_text(s: &Shape) -> String {
    let mut parts: Vec<String> = s.prefix.iter().map(|d| sh::dim_text(*d)).collect();
    if let Some(r) = &s.rest {
        let t = sh::rest_text(r);
        // an unknown number of unknown dims: `*` (as PyTorch's docs write `(*, H_in)`)
        parts.push(if t == "*" { "*".into() } else { format!("*{}", t) });
    }
    parts.extend(s.suffix.iter().map(|d| sh::dim_text(*d)));
    let mut out = format!("Tensor[{}]", parts.join(", "));
    if !s.assume.is_empty() {
        out.push_str(&format!(" (if {})", s.assume.iter().map(sh::cons_text).collect::<Vec<_>>().join(", ")));
    }
    out
}

impl Analyzer {
    /// (hover legend) a symbol shown as a tensor needs no `x: ?` line
    pub fn facts_sym_is_tensor(&self, _sym: &str) -> bool {
        true
    }
}

/// `x.split(…)` / `x.sort(…)` on an unknown: a string separator / list sort keywords make it
/// the str / list method, sizes or `dim=` the tensor one.
pub fn m_tensor_or_builtin(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.strip_prefix("tensorq.").unwrap_or("");
    let recv = c.recv.clone().unwrap_or_default();
    let has_dim = kw(c, "dim").is_some() || kw(c, "descending").is_some();
    let builtin = match name {
        "split" => !has_dim && (c.args.is_empty() || c.args.first().map(|a| an.t_str(a).is_some()).unwrap_or(false)) || kw(c, "sep").is_some() || kw(c, "maxsplit").is_some(),
        _ => !has_dim && (kw(c, "key").is_some() || kw(c, "reverse").is_some() || c.args.is_empty() && c.kwargs.is_empty()),
    };
    if builtin {
        let q = if name == "split" { "str.split" } else { "list.sort" };
        let call = Call { qual: q.to_string(), recv: c.recv.clone(), args: c.args.clone(), kwargs: c.kwargs.clone(), star_args: c.star_args.clone(), star_kwargs: c.star_kwargs.clone(), node: c.node, call_ast: c.call_ast, site: c.site.clone(), line: c.line };
        if let Some(m) = an.models.lookup(q) {
            return m(an, st, &call);
        }
        return Value::data(&format!("L{}", c.line));
    }
    let args = c.args.clone();
    an.t_method(name, &recv, &args, c, st)
}
