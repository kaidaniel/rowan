//! pandas models (DataFrame hooks used by the interpreter and the `DataFrame.*` methods).

use indexmap::IndexMap;

use crate::domain::{classify, join_rows, join_values, kv_join, kv_literals, Classification, FnKind, KeyVal, KeysOf, Ns, ObjSet, RenameFn, Row, RowData, Source, SourceKind, Step, Tier, Value, DATA};
use crate::frontend::Node;
use crate::interp::funcs::Elem;
use crate::interp::{Analyzer, Call, Payload, ViewKind};
use crate::intern::{self, key_str_lit, KeyId, KeySet, EMPTY_LENS};
use crate::state::{Kind, State};

use super::{noeffect, Model};

pub const DF_METHODS: &[&str] = &[
    "abs", "add", "add_prefix", "add_suffix", "agg", "aggregate", "align", "all", "any", "apply", "applymap", "asfreq", "asof", "assign", "astype",
    "at_time", "backfill", "between_time", "bfill", "bool", "boxplot", "clip", "combine", "combine_first", "compare", "convert_dtypes", "copy",
    "corr", "corrwith", "count", "cov", "cummax", "cummin", "cumprod", "cumsum", "describe", "diff", "div", "divide", "dot", "drop",
    "drop_duplicates", "droplevel", "dropna", "duplicated", "eq", "equals", "eval", "ewm", "expanding", "explode", "ffill", "fillna", "filter",
    "first", "first_valid_index", "floordiv", "from_dict", "from_records", "ge", "get", "groupby", "gt", "head", "hist", "idxmax", "idxmin",
    "infer_objects", "info", "insert", "interpolate", "isetitem", "isin", "isna", "isnull", "items", "iteritems", "iterrows", "itertuples", "join",
    "keys", "kurt", "kurtosis", "last", "last_valid_index", "le", "lookup", "lt", "map", "mask", "max", "mean", "median", "melt", "memory_usage",
    "merge", "min", "mod", "mode", "mul", "multiply", "ne", "nlargest", "notna", "notnull", "nsmallest", "nth", "nunique", "pad", "pct_change",
    "pipe", "pivot", "pivot_table", "plot", "pop", "pow", "prod", "product", "quantile", "query", "radd", "rank", "rdiv", "reindex", "reindex_like",
    "rename", "rename_axis", "reorder_levels", "replace", "resample", "reset_index", "rfloordiv", "rmod", "rmul", "rolling", "round", "rpow", "rsub",
    "rtruediv", "sample", "select_dtypes", "sem", "set_axis", "set_flags", "set_index", "shift", "skew", "sort", "sort_index", "sort_values",
    "squeeze", "stack", "std", "sub", "subtract", "sum", "swapaxes", "swaplevel", "tail", "take", "to_clipboard", "to_csv", "to_dict", "to_excel",
    "to_feather", "to_gbq", "to_hdf", "to_html", "to_json", "to_latex", "to_markdown", "to_numpy", "to_orc", "to_parquet", "to_period", "to_pickle",
    "to_records", "to_sql", "to_stata", "to_string", "to_timestamp", "to_xarray", "to_xml", "transform", "transpose", "truediv", "truncate",
    "tz_convert", "tz_localize", "unstack", "update", "value_counts", "var", "where", "xs", "cumcount", "ngroup",
];
pub const DF_ATTRS: &[&str] = &["columns", "index", "shape", "values", "dtypes", "T", "empty", "size", "ndim", "axes", "style", "plot", "iloc", "loc", "at", "iat", "str", "dt", "cat", "attrs", "flags"];

/// Attributes that make a placeholder a frame for the summary (tabulated where it is one).
pub const FRAME_ONLY_ATTRS: &[&str] = &["loc", "iloc", "at", "iat", "columns", "shape", "dtypes", "T", "empty", "ndim", "axes"];

pub fn frame_producer(q: &str) -> Option<&'static str> {
    Some(match q {
        "pandas.read_csv" => "read_csv",
        "pandas.read_excel" => "read_excel",
        "pandas.read_parquet" => "read_parquet",
        "pandas.read_sql" | "pandas.read_sql_query" | "pandas.read_sql_table" => "read_sql",
        "pandas.read_json" => "read_json",
        "pandas.read_table" => "read_table",
        "pandas.read_feather" => "read_feather",
        "pandas.read_pickle" => "read_pickle",
        "pandas.read_html" => "read_html",
        "pandas.read_fwf" => "read_fwf",
        "pandas.read_stata" => "read_stata",
        "pandas.read_hdf" => "read_hdf",
        "pandas.read_orc" => "read_orc",
        "pandas.read_xml" => "read_xml",
        "pandas.read_clipboard" => "read_clipboard",
        "pandas.read_gbq" => "read_gbq",
        _ => return None,
    })
}
const FRAME_PRODUCERS: &[&str] = &[
    "pandas.read_csv", "pandas.read_excel", "pandas.read_parquet", "pandas.read_sql", "pandas.read_sql_query", "pandas.read_sql_table", "pandas.read_json",
    "pandas.read_table", "pandas.read_feather", "pandas.read_pickle", "pandas.read_html", "pandas.read_fwf", "pandas.read_stata", "pandas.read_hdf",
    "pandas.read_orc", "pandas.read_xml", "pandas.read_clipboard", "pandas.read_gbq",
];
const OPEN_FRAME_OPS: &[&str] = &["pivot", "pivot_table", "stack", "unstack", "transpose", "T", "describe", "corr", "cov", "agg", "aggregate", "explode_unknown", "get_dummies", "crosstab", "json_normalize", "melt_unknown"];
const COPY_FRAME_OPS: &[&str] = &[
    "copy", "head", "tail", "sample", "sort_values", "sort_index", "drop_duplicates", "fillna", "dropna", "astype", "query", "where", "mask", "round", "abs",
    "clip", "replace", "shift", "diff", "cumsum", "cumprod", "cummax", "cummin", "rank", "pct_change", "isna", "isnull", "notna", "notnull",
    "convert_dtypes", "infer_objects", "interpolate", "bfill", "ffill", "pad", "backfill", "eq", "ne", "lt", "le", "gt", "ge", "add", "sub", "mul",
    "div", "truediv", "floordiv", "mod", "pow", "applymap", "map", "take", "truncate", "between_time", "at_time", "asfreq", "tz_localize", "tz_convert",
    "first", "last", "isin", "duplicated_frame", "combine_first", "align", "reindex_like", "sort", "squeeze_frame", "compare_frame", "nlargest",
    "nsmallest", "to_period", "to_timestamp", "swaplevel", "droplevel", "reorder_levels",
];
/// Reductions over the rows: one value per column.
const FRAME_REDUCTIONS: &[&str] = &[
    "mean", "sum", "min", "max", "count", "median", "std", "var", "nunique", "idxmax", "idxmin", "any", "all", "sem", "skew", "kurt", "kurtosis", "prod", "product", "isna_sum_",
];
const SERIES_RESULT_OPS: &[&str] = &[
    "mean", "sum", "min", "max", "count", "median", "std", "var", "nunique", "idxmax", "idxmin", "any", "all", "value_counts", "memory_usage", "sem",
    "skew", "kurt", "prod", "quantile", "mode", "duplicated", "dot", "corrwith", "size", "cumcount", "ngroup",
];
const NOEFFECT_OPS: &[&str] = &[
    "to_csv", "to_parquet", "to_excel", "to_json", "to_sql", "to_numpy", "to_records", "to_string", "to_html", "to_markdown", "info", "equals", "plot",
    "hist", "boxplot", "to_pickle", "to_feather", "to_clipboard", "to_latex", "to_stata", "to_hdf", "to_xml", "to_orc",
];

pub fn install(r: &mut dyn FnMut(&[&'static str], Model)) {
    r(FRAME_PRODUCERS, read_frame);
    r(&["pandas.DataFrame", "pandas.DataFrame.from_records", "pandas.DataFrame.from_dict"], pd_dataframe);
    r(&["pandas.merge", "pandas.merge_asof", "pandas.merge_ordered"], pd_merge);
    r(&["pandas.concat"], concat);
    r(&["pandas.read_html_table"], open_frame_call);
    r(&["pandas.json_normalize"], json_normalize);
    r(&["pandas.get_dummies"], get_dummies);
    r(&["pandas.crosstab"], crosstab);
    r(&["pandas.pivot_table", "pandas.pivot"], pivot);
    r(&["pandas.wide_to_long"], wide_to_long);
    r(&["pandas.melt"], melt);
    r(
        &[
            "pandas.to_datetime", "pandas.to_numeric", "pandas.isna", "pandas.notna", "pandas.isnull", "pandas.notnull", "pandas.date_range", "pandas.Timestamp", "pandas.Timedelta", "pandas.to_timedelta", "pandas.factorize", "pandas.unique",
            "pandas.array", "pandas.Categorical", "pandas.Index", "pandas.MultiIndex", "pandas.Grouper", "pandas.set_option",
            "pandas.option_context", "pandas.testing.assert_frame_equal", "pandas.ExcelWriter", "pandas.eval",
        ],
        series_call,
    );
    r(&["Series.*"], series_method);
    r(&["io.StringIO", "io.BytesIO"], string_io);
    r(&["pandas.Series"], series_ctor);
    r(&["pandas.NamedAgg"], named_agg);
    r(&["pandas.cut", "pandas.qcut"], cut);
    r(&["Series.str.*", "Series.dt.*", "Series.cat.*", "Series.sparse.*", "Series.list.*", "Series.struct.*"], accessor_method);
    r(&["DataFrame.*"], df_dispatch);
    r(&["groupby.*"], groupby_method);
}

/// Inside a summary a frame made before the call has a symbolic row: the models build
/// concrete rows, so such a call is decided where the frame is known (a "concrete" event).
pub fn symbolic_frames_event(an: &mut Analyzer, st: &State, vals: &[&Value]) {
    if an.sum_stack.is_empty() {
        return;
    }
    for v in vals {
        for o in v.pts.iter() {
            let k = an.kind(o);
            if (k == Kind::Frame || is_series_obj(an, o)) && st.row(o, Ns::Item).symbolic() {
                an.record_event("concrete", o);
            }
            if k == Kind::Groupby {
                if let Payload::Groupby(fs, ..) = an.obj(o).payload.clone() {
                    for f in fs {
                        if st.row(f, Ns::Item).symbolic() {
                            an.record_event("concrete", f);
                        }
                    }
                }
            }
        }
    }
}

fn call_values(c: &Call) -> Vec<&Value> {
    c.recv.iter().chain(c.args.iter()).chain(c.kwargs.values()).chain(c.star_args.iter()).collect()
}

/// Every `DataFrame.<m>(…)`: the op's model; with `inplace=True` its result replaces the
/// receiver's columns (and what is known of its rows) and the call returns None.
fn df_dispatch(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    symbolic_frames_event(an, st, &call_values(c));
    let name = last(&c.qual).to_string();
    let m: Model = match name.as_str() {
        "merge" => df_merge,
        "join" => df_join,
        "rename" => df_rename,
        "add_prefix" | "add_suffix" => add_prefix,
        "drop" => df_drop,
        "assign" => df_assign,
        "reset_index" => reset_index,
        "set_index" => set_index,
        "groupby" => groupby,
        "apply" => df_apply,
        "pipe" => df_pipe,
        "iterrows" => iterrows,
        "itertuples" => itertuples,
        "to_dict" => to_dict,
        "to_json" => to_json,
        "to_records" => to_records,
        "insert" => df_insert,
        "pop" => df_pop,
        "melt" => melt,
        "explode" => explode,
        "filter" => df_filter,
        "reindex" => reindex,
        "reindex_like" => reindex_like,
        "select_dtypes" => select_dtypes,
        "sort_values" | "nlargest" | "nsmallest" => sort_values,
        "transform" => transform,
        "update" => df_update,
        "describe" => describe,
        "droplevel" | "swaplevel" => levels_op,
        "agg" | "aggregate" => df_agg,
        "corr" | "cov" => df_corr,
        "combine_first" | "combine" => df_combine,
        "set_axis" => set_axis,
        "rename_axis" => rename_axis,
        "transpose" | "T" => transpose,
        "query" => df_query,
        "pivot" | "pivot_table" => pivot,
        "stack" => |an: &mut Analyzer, st: &mut State, c: &Call| match frames(an, c.recv.as_ref()).as_slice() {
            [f] => stack_frame(an, st, c, *f),
            _ => open_frame_call(an, st, c),
        },
        "unstack" => |an: &mut Analyzer, st: &mut State, c: &Call| match frames(an, c.recv.as_ref()).as_slice() {
            [f] => unstack(an, st, c, *f, false),
            _ => open_frame_call(an, st, c),
        },
        "eval" => df_eval,
        _ => df_generic,
    };
    let res = m(an, st, c);
    if !key_is_bool(&c.kw("inplace"), true) || matches!(name.as_str(), "insert" | "pop" | "update") {
        return res;
    }
    let fs = frames(an, c.recv.as_ref());
    let strong = fs.len() == 1 && an.obj(fs[0]).single;
    for r in frames(an, Some(&res)) {
        let row = st.row(r, Ns::Item);
        let meta: Vec<(crate::state::SlotKey, Value)> = st.slots_of(r).into_iter().filter(|((ns, _), _)| *ns == Ns::Meta).collect();
        for f in &fs {
            let nr = if strong { row.clone() } else { st.row(*f, Ns::Item).join(&row) };
            st.set_row(*f, Ns::Item, nr);
            for ((ns, k), v) in &meta {
                st.set_slot(*f, *ns, *k, v.clone(), strong);
            }
        }
    }
    an.none_v.clone()
}

fn last(q: &str) -> &str {
    q.rsplit('.').next().unwrap_or(q)
}

fn series(an: &Analyzer) -> Value {
    Value::with_kv(ObjSet::one(an.series), DATA)
}

fn series_call(an: &mut Analyzer, _st: &mut State, _c: &Call) -> Value {
    series(an)
}

// ------------------------------------------------------------- Series with a tracked index
// A Series whose index matters (a column, a frame's row, a reduction over the columns, a
// grouped aggregate) is a dict-like object: its keys are the index labels (closed when
// known, an open data row otherwise). Meta: `name` (the Series name), `index` (index level
// names, as for frames), ("vals", "") / ("dtype", "") its values.

pub fn is_series_obj(an: &Analyzer, o: u32) -> bool {
    an.kind(o) == Kind::Dict && {
        let l = &an.obj(o).label;
        l.starts_with("series@") || l.starts_with("row@") || l.starts_with("row of ")
    }
}

/// Any Series: the generic one or a tracked one.
pub fn is_seriesish(an: &Analyzer, v: &Value) -> bool {
    v.pts.iter().any(|o| o == an.series || is_series_obj(an, o))
}

fn vkey() -> KeyId {
    mk("")
}

/// The index row of a Series made from a frame's rows: its labels when known.
fn labels_row(st: &State, f: u32) -> Row {
    match labels(st, f) {
        Some(k) => k.row(),
        None => Row::with_source(Source::new("index", SourceKind::Data)),
    }
}

/// A new tracked Series at `node` (`tag` keeps several made at one node apart).
pub fn new_series(an: &mut Analyzer, st: &mut State, node: &Node, idx: Row, name: Option<Value>, tag: &str) -> u32 {
    let site = format!("{}:s{}", crate::interp::site_string(node), tag);
    let sid = an.alloc(Kind::Dict, &site, st, &format!("series@L{}", node.line), Payload::None, Some(idx.clone()), Ns::Item, None, None);
    for k in idx.present.iter() {
        st.set_slot(sid, Ns::Item, k, Value::data(&format!("cell {}", intern::key_py_str(k))), false);
    }
    st.set_slot(sid, Ns::Meta, mk("name"), name.unwrap_or_else(|| an.none_v.clone()), true);
    sid
}

fn series_name(an: &Analyzer, st: &State, sid: u32) -> Value {
    if sid == an.series {
        return Value::data("name");
    }
    st.slot_opt(sid, Ns::Meta, mk("name")).unwrap_or_else(|| Value::data("name"))
}

/// The column a Series becomes in a frame: its name (unnamed: 0); None when unknown.
fn name_column(an: &Analyzer, v: &Value) -> Option<KeyId> {
    if !v.pts.is_empty() {
        return None;
    }
    let k = v.kv.as_lit()?;
    if v.kv == an.none_v.kv {
        return Some(intern::key_int(0));
    }
    Some(k)
}

fn copy_series_meta(st: &mut State, src: u32, dst: u32) {
    for i in 0..4 {
        let (a, b) = lvals_keys(i);
        for k in [a, b] {
            if let Some(v) = st.slot_opt(src, Ns::Meta, k) {
                st.set_slot(dst, Ns::Meta, k, v, true);
            }
        }
    }
    for k in [mk("index"), mk("name")] {
        if let Some(v) = st.slot_opt(src, Ns::Meta, k) {
            st.set_slot(dst, Ns::Meta, k, v, true);
        }
    }
    if let Some(v) = vals(st, src, vkey()) {
        set_vals(st, dst, vkey(), Some(&v));
    }
    if let Some(v) = st.slot_opt(src, Ns::Meta, dtype_key(vkey())) {
        st.set_slot(dst, Ns::Meta, dtype_key(vkey()), v, true);
    }
}

/// `df[col]`: the column as a Series (index = the frame's row labels, name = col).
fn column_series(an: &mut Analyzer, st: &mut State, f: u32, col: KeyId, node: &Node) -> Value {
    let idx = labels_row(st, f);
    let sid = new_series(an, st, node, idx, Some(Value::lit(col)), &format!("c{}", intern::key_py_str(col)));
    if let Some(m) = st.slot_opt(f, Ns::Meta, mk("index")) {
        st.set_slot(sid, Ns::Meta, mk("index"), m, true);
    }
    for i in 0..4 {
        let (a, b) = lvals_keys(i);
        for k in [a, b] {
            if let Some(v) = st.slot_opt(f, Ns::Meta, k) {
                st.set_slot(sid, Ns::Meta, k, v, true);
            }
        }
    }
    if let Some(v) = vals(st, f, col) {
        set_vals(st, sid, vkey(), Some(&v));
    }
    if let Some(v) = st.slot_opt(f, Ns::Meta, dtype_key(col)) {
        st.set_slot(sid, Ns::Meta, dtype_key(vkey()), v, true);
    }
    Value::obj(sid)
}

/// `.str` / `.dt` / `.cat` and other attributes of a Series.
pub fn series_getattr(an: &mut Analyzer, oid: u32, name: &str, node: &Node, st: &mut State) -> Value {
    match name {
        "str" | "dt" | "cat" | "sparse" | "list" | "struct" => {
            let label = format!("{}.{}", an.obj(oid).label, name);
            let lid = an.alloc_raw(Kind::Locidx, &format!("acc:{}:{}", name, oid), "", true, &label, Payload::Locidx(name.to_string(), oid));
            an.obj_mut(lid).payload = Payload::Locidx(name.to_string(), oid);
            Value::obj(lid)
        }
        "index" if oid != an.series => {
            let vid = an.alloc_raw(Kind::View, &format!("view:keys:{}", oid), "", true, "index", Payload::View(ViewKind::Keys, Value::obj(oid)));
            an.obj_mut(vid).payload = Payload::View(ViewKind::Keys, Value::obj(oid));
            Value::obj(vid)
        }
        "name" => series_name(an, st, oid),
        "values" | "array" | "dtype" | "shape" | "size" | "empty" | "nbytes" | "ndim" | "hasnans" | "is_unique" | "is_monotonic_increasing" | "is_monotonic_decreasing" | "index" | "axes" | "dtypes" | "flags" | "attrs" => Value::data(&format!("{}@L{}", name, node.line)),
        "iloc" | "loc" | "at" | "iat" if oid != an.series => {
            let label = format!("{} of {}", name, an.obj(oid).label);
            let lid = an.alloc_raw(Kind::Locidx, &format!("sloc:{}:{}", name, oid), "", true, &label, Payload::Locidx(format!("s{}", name), oid));
            an.obj_mut(lid).payload = Payload::Locidx(format!("s{}", name), oid);
            Value::obj(lid)
        }
        "iloc" | "loc" | "at" | "iat" => Value::data(&format!("{}@L{}", name, node.line)),
        "T" => Value::obj(oid),
        _ => an.method_value(&format!("Series.{}", name), &Value::obj(oid)),
    }
}

/// The tracked Series objects a call's receiver holds (empty: only the generic Series).
fn recv_series(an: &Analyzer, c: &Call) -> Vec<u32> {
    c.recv.as_ref().map(|r| r.pts.iter().filter(|o| is_series_obj(an, *o)).collect()).unwrap_or_default()
}

fn series_like(an: &mut Analyzer, st: &mut State, c: &Call, ss: &[u32], idx: Row, tag: &str) -> Value {
    Value::obj(series_like_id(an, st, c, ss, idx, tag))
}

fn series_like_id(an: &mut Analyzer, st: &mut State, c: &Call, ss: &[u32], idx: Row, tag: &str) -> u32 {
    let name = join_values(ss.iter().map(|x| series_name(an, st, *x)));
    let sid = new_series(an, st, &c.node, idx, Some(name), tag);
    if let [one] = ss {
        let name = st.slot_opt(sid, Ns::Meta, mk("name"));
        copy_series_meta(st, *one, sid);
        if let Some(n) = name {
            st.set_slot(sid, Ns::Meta, mk("name"), n, true);
        }
    }
    sid
}

/// `pd.Series(data, index=, name=)`: a dict's keys (or the positions, or index=) are the index.
/// `pd.cut(x, bins, labels=[…])` / `pd.qcut`: x's index; the values are among the labels.
fn cut(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let x = c.arg(0, Some("x")).unwrap_or_default();
    let ss: Vec<u32> = x.pts.iter().filter(|o| is_series_obj(an, *o)).collect();
    if ss.is_empty() {
        return series(an);
    }
    let idx = join_rows(ss.iter().map(|o| st.row(*o, Ns::Item)));
    let sid = series_like_id(an, st, c, &ss, idx, "cut");
    let labels = c.kw("labels").and_then(|v| lit_key_seq(an, st, &v));
    // (a value outside the bins is NaN)
    let k = labels.clone().map(|l| Known { may: KeySet::from_iter(l).insert(none_key()), sure: KeySet::new() });
    set_vals(st, sid, vkey(), k.as_ref());
    // a categorical: its categories are the labels
    if let Some(l) = labels {
        st.set_slot(sid, Ns::Meta, mk("cats"), Value::kv_only(KeyVal::Lits(KeySet::from_iter(l))), true);
    }
    st.set_slot(sid, Ns::Meta, dtype_key(vkey()), Value::lit(mk("object")), true);
    Value::obj(sid)
}

/// `pd.NamedAgg(column=, aggfunc=)`: the (column, function) pair.
fn named_agg(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let col = c.arg(0, Some("column")).unwrap_or_default();
    let f = c.arg(1, Some("aggfunc")).unwrap_or_default();
    let tid = an.alloc_at(Kind::Tuple, &c.node, st, "tuple", Payload::None, None, Ns::Item, None, None);
    an.set_llen(tid, st, Some(2));
    st.set_slot(tid, Ns::Pos, intern::key_int(0), col, true);
    st.set_slot(tid, Ns::Pos, intern::key_int(1), f, true);
    Value::obj(tid)
}

/// Named aggregations `name=(column, f)`, from keywords and `**dict`: (name, surely, column).
fn named_aggs(an: &Analyzer, st: &State, c: &Call) -> Vec<(KeyId, bool, Option<KeyId>)> {
    let col_of = |v: &Value| -> Option<KeyId> {
        v.pts.iter().find_map(|x| (an.kind(x) == Kind::Tuple && an.llen(x, st) == Some(2)).then(|| st.slot(x, Ns::Pos, intern::key_int(0)).kv.as_lit()).flatten())
    };
    let mut out = Vec::new();
    for (k, v) in c.kwargs.iter() {
        if c.qual.ends_with(".agg") || c.qual.ends_with(".aggregate") {
            out.push((intern::key(intern::Key::Str(intern::str_get(*k))), true, col_of(v)));
        }
    }
    for sk in &c.star_kwargs {
        for d in sk.pts.iter().filter(|o| an.kind(*o) == Kind::Dict) {
            let r = st.row(d, Ns::Item);
            for k in r.present.iter() {
                out.push((k, r.definite.contains(k), col_of(&st.slot(d, Ns::Item, k))));
            }
        }
    }
    out
}

fn series_ctor(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let data = c.arg(0, Some("data"));
    let index = c.kw("index").or_else(|| c.arg(1, None));
    let name = c.kw("name").or_else(|| c.arg(3, None)).unwrap_or_else(|| an.none_v.clone());
    let idx_kw = index.as_ref().and_then(|v| an.lit_keys(v, st));
    let (idx, vk): (Option<Row>, Option<Known>) = match data.as_ref().and_then(|d| d.single()) {
        Some(d) if an.kind(d) == Kind::Dict && !is_series_obj(an, d) => {
            let r = st.row(d, Ns::Item);
            let mut vals_ = KeySet::new();
            let mut ok = r.closed();
            for k in r.present.iter() {
                match cell_lit(&st.slot(d, Ns::Item, k)) {
                    Some(l) => vals_ = vals_.insert(l),
                    None => ok = false,
                }
            }
            (Some(Row::new().copy_from(&r, true, &KeySet::new(), EMPTY_LENS)), ok.then(|| Known::exact(vals_)))
        }
        Some(d) if is_series_obj(an, d) => (Some(st.row(d, Ns::Item)), vals(st, d, vkey())),
        Some(d) if matches!(an.kind(d), Kind::List | Kind::Tuple) => {
            let items = lit_key_seq(an, st, &Value::obj(d));
            let n = an.llen(d, st);
            let idx = n.map(|n| range_labels(n as i64).row());
            (idx, items.map(|it| Known::exact(KeySet::from_iter(it))))
        }
        None => (Some(Row::new()), Some(Known::exact(KeySet::new()))),
        _ => (None, None),
    };
    let idx = match idx_kw {
        Some(ks) => Some(Row::closed_with(ks)),
        None => idx,
    };
    let idx = idx.unwrap_or_else(|| Row::with_source(Source::new("index", SourceKind::Data)));
    let sid = new_series(an, st, &c.node, idx, Some(name), "c");
    set_vals(st, sid, vkey(), vk.as_ref());
    if let Some(v) = &vk {
        let dt = dtype_of_lits(&v.may);
        st.set_slot(sid, Ns::Meta, dtype_key(vkey()), match dt {
            Some(d) => Value::lit(mk(d)),
            None => Value::data("dtype"),
        }, true);
    }
    Value::obj(sid)
}

fn series_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    symbolic_frames_event(an, st, &call_values(c));
    let name = last(&c.qual).to_string();
    let ss = recv_series(an, c);
    if ss.is_empty() {
        // the generic Series: what can be said without its index
        return match name.as_str() {
            "to_frame" => {
                let col = c.arg(0, Some("name")).and_then(|v| name_column(an, &v));
                match col {
                    Some(k) => new_frame(an, st, c, Row::closed_with(KeySet::one(k)), None, None),
                    None => new_frame(an, st, c, Row::with_source(Source::new(&format!("to_frame@L{}", c.line), SourceKind::Call)), None, None),
                }
            }
            "reset_index" if !key_is_bool(&c.kw("drop"), true) => {
                let mut row = Row::with_source(Source::new(&format!("reset_index@L{}", c.line), SourceKind::Call));
                if let Some(k) = c.kw("name").and_then(|v| name_column(an, &v)) {
                    row = row.store(k, true);
                }
                new_frame(an, st, c, row, None, None)
            }
            "to_dict" => {
                let oid = an.alloc_at(Kind::Dict, &c.node, st, &format!("to_dict@L{}", c.line), Payload::None, Some(Row::with_source(Source::new("index", SourceKind::Data))), Ns::Item, None, None);
                Value::obj(oid)
            }
            _ if SERIES_SCALAR_OPS.contains(&name.as_str()) => Value::data(&format!("{}@L{}", name, c.line)),
            _ => series(an),
        };
    }
    let idx = join_rows(ss.iter().map(|x| st.row(*x, Ns::Item)));
    match name.as_str() {
        "get" | "keys" | "items" | "iteritems" | "pop" | "__contains__" => {
            let q = format!("dict.{}", if name == "iteritems" { "items" } else { name.as_str() });
            match an.models.lookup(&q) {
                Some(m) => m(an, st, c),
                None => series(an),
            }
        }
        "to_dict" => {
            let oid = an.alloc_at(Kind::Dict, &c.node, st, &format!("to_dict@L{}", c.line), Payload::None, Some(idx.clone()), Ns::Item, None, None);
            for k in idx.present.iter() {
                st.set_slot(oid, Ns::Item, k, Value::data("cell"), false);
            }
            Value::obj(oid)
        }
        "to_frame" => {
            let col = match c.arg(0, Some("name")) {
                Some(v) => name_column(an, &v),
                None => name_column(an, &join_values(ss.iter().map(|x| series_name(an, st, *x)))),
            };
            let row = match col {
                Some(k) => Row::closed_with(KeySet::one(k)),
                None => Row::with_source(Source::new(&format!("to_frame@L{}", c.line), SourceKind::Call)),
            };
            let v = new_frame(an, st, c, row, None, None);
            if let (Some(nid), [one]) = (v.single(), ss.as_slice()) {
                let lab = if idx.closed() { Some(Known { may: idx.present.clone(), sure: idx.definite.clone() }) } else { None };
                set_labels(st, nid, lab.as_ref());
                copy_meta(*one, nid, st);
                if let Some(k) = col {
                    if let Some(vv) = vals(st, *one, vkey()) {
                        set_vals(st, nid, k, Some(&vv));
                    }
                }
            }
            v
        }
        "reset_index" => {
            if key_is_bool(&c.kw("drop"), true) {
                let n = series_like(an, st, c, &ss, Row::with_source(Source::new("index", SourceKind::Data)), "r");
                return n;
            }
            let mut row = Row::new();
            match levels_of(st, &ss) {
                Some(levels) => {
                    for i in 0..levels.len() {
                        row = row.store(level_column(&levels, i), true);
                    }
                }
                None => {
                    let mut d = row.data();
                    d.sources = d.sources.insert((Source::new(&format!("reset_index@L{}", c.line), SourceKind::Call), EMPTY_LENS));
                    row = Row::from_data(d);
                }
            }
            let col = match c.kw("name") {
                Some(v) => name_column(an, &v),
                None => name_column(an, &join_values(ss.iter().map(|x| series_name(an, st, *x)))),
            };
            match col {
                Some(k) => row = row.store(k, true),
                None => {
                    let mut d = row.data();
                    d.sources = d.sources.insert((Source::new(&format!("reset_index@L{}", c.line), SourceKind::Call), EMPTY_LENS));
                    row = Row::from_data(d);
                }
            }
            let v = new_frame(an, st, c, row, None, None);
            fresh_index(st, &v);
            v
        }
        "rename" => {
            let a = c.arg(0, Some("index"));
            match &a {
                Some(v) if v.pts.is_empty() && v.kv.is_literal() => {
                    let sid = series_like_id(an, st, c, &ss, idx, "n");
                    st.set_slot(sid, Ns::Meta, mk("name"), v.clone(), true);
                    Value::obj(sid)
                }
                Some(v) => {
                    let fs: Vec<u32> = ss.clone();
                    match rename_step(an, st, v, c, &fs) {
                        Some(steps) => {
                            let mut r = idx;
                            for s0 in &steps {
                                r = r.rename(s0);
                            }
                            series_like(an, st, c, &ss, r, "n")
                        }
                        None => series_like(an, st, c, &ss, Row::from_data(RowData { present: idx.present.clone(), sources: idx.sources.insert((Source::new("rename", SourceKind::Data), EMPTY_LENS)), ..Default::default() }), "n"),
                    }
                }
                None => series_like(an, st, c, &ss, idx, "n"),
            }
        }
        "rename_axis" => {
            let sid = series_like_id(an, st, c, &ss, idx, "a");
            if let Some(v) = c.arg(0, Some("mapper")).or_else(|| c.kw("index")) {
                let names: Option<Vec<Option<KeyId>>> = match lit_key_seq(an, st, &v) {
                    Some(seq) => Some(seq.into_iter().map(|k| if k == none_key() { None } else { Some(k) }).collect()),
                    None => v.kv.as_lit().filter(|_| v.pts.is_empty()).map(|k| vec![if k == none_key() { None } else { Some(k) }]),
                };
                set_index_levels(st, sid, names.as_deref());
            }
            Value::obj(sid)
        }
        "value_counts" => {
            let vs = ss.iter().map(|x| vals(st, *x, vkey())).collect::<Option<Vec<Known>>>();
            let cats: Option<KeySet> = (ss.len() == 1).then(|| st.slot_opt(ss[0], Ns::Meta, mk("cats"))).flatten().filter(|v| v.pts.is_empty() && v.kv.is_literal()).map(|v| kv_literals(&v.kv));
            let row = match vs {
                // a categorical counts every category (zero counts too)
                _ if cats.is_some() => Known::exact(cats.clone().unwrap()).row(),
                Some(v) if !v.is_empty() => {
                    let k = v[1..].iter().fold(v[0].clone(), |a, b| a.join(b));
                    if key_is_bool(&c.kw("dropna"), false) { k.row() } else { k.row() }
                }
                _ => Row::with_source(Source::new("values", SourceKind::Data)),
            };
            let nm = if key_is_bool(&c.kw("normalize"), true) { "proportion" } else { "count" };
            let sid = new_series(an, st, &c.node, row, Some(Value::lit(mk(nm))), "v");
            let names = join_values(ss.iter().map(|x| series_name(an, st, *x)));
            st.set_slot(sid, Ns::Meta, mk("index"), names, true);
            Value::obj(sid)
        }
        "drop" => {
            let ks = c.arg(0, Some("labels")).or_else(|| c.kw("index")).and_then(|v| an.lit_keys(&v, st));
            let mut r = idx;
            if let Some(ks) = ks {
                for k in ks.iter() {
                    r = r.delete(k, true);
                }
            }
            series_like(an, st, c, &ss, r, "d")
        }
        "reindex" | "set_axis" => match c.arg(0, None).or_else(|| c.kw("index")).or_else(|| c.kw("labels")).and_then(|v| an.lit_keys(&v, st)) {
            Some(ks) => series_like(an, st, c, &ss, Row::closed_with(ks), "x"),
            None => series(an),
        },
        "add_prefix" | "add_suffix" => {
            let fnk = if name == "add_prefix" { FnKind::Prefix } else { FnKind::Suffix };
            match lit_key(&c.arg(0, None)) {
                Some(k) => series_like(an, st, c, &ss, idx.rename(&Step::RenameFn(RenameFn::with_arg(fnk, k))), "p"),
                None => series(an),
            }
        }
        "idxmax" | "idxmin" => {
            if idx.closed() && !idx.present.is_empty() {
                Value::kv_only(KeyVal::lits(idx.present.clone()))
            } else {
                Value::data(&format!("{}@L{}", name, c.line))
            }
        }
        "head" | "tail" | "dropna" | "drop_duplicates" | "nlargest" | "nsmallest" | "sample" | "loc" | "iloc" | "truncate" => {
            let mut d = idx.data();
            d.definite = KeySet::new();
            d.likely = KeySet::new();
            series_like(an, st, c, &ss, Row::from_data(d), "h")
        }
        "copy" | "sort_values" | "sort_index" | "astype" | "fillna" | "round" | "abs" | "clip" | "cumsum" | "cumprod" | "cummax" | "cummin" | "shift" | "diff" | "rank" | "pct_change" | "isna" | "isnull" | "notna" | "notnull" | "replace" | "where" | "mask" | "map" | "apply" | "transform" | "isin" | "between" | "eq" | "ne" | "lt" | "le" | "gt" | "ge" | "add" | "sub" | "mul" | "div" | "truediv" | "floordiv" | "mod" | "pow" | "interpolate" | "ffill" | "bfill" | "convert_dtypes" | "infer_objects" | "explode" | "mean_" => {
            let sid = series_like_id(an, st, c, &ss, idx, "m");
            if matches!(name.as_str(), "fillna" | "ffill" | "bfill" | "where" | "mask" | "shift" | "interpolate") {
                // the same kind of values (a shift / fill of numbers stays numeric)
                set_vals(st, sid, vkey(), None);
                let d = st.slot_opt(sid, Ns::Meta, dtype_key(vkey())).and_then(|v| v.kv.as_lit()).and_then(intern::key_as_str).map(|x| x.to_string());
                let nd = match d.as_deref() {
                    Some("int") | Some("float") | Some("num") => Value::lit(mk("num")),
                    Some("object") if matches!(name.as_str(), "fillna" | "ffill" | "bfill") => Value::lit(mk("object")),
                    _ => Value::data("dtype"),
                };
                st.set_slot(sid, Ns::Meta, dtype_key(vkey()), nd, true);
            } else if !matches!(name.as_str(), "copy" | "sort_values" | "sort_index") {
                set_vals(st, sid, vkey(), None);
                let dt = if matches!(name.as_str(), "isna" | "isnull" | "notna" | "notnull" | "isin" | "between" | "eq" | "ne" | "lt" | "le" | "gt" | "ge") {
                    Value::lit(mk("bool"))
                } else if matches!(name.as_str(), "clip" | "round" | "abs" | "cumsum" | "cumprod" | "cummax" | "cummin" | "diff" | "pct_change" | "add" | "sub" | "mul" | "div" | "truediv" | "floordiv" | "mod" | "pow" | "rank" | "interpolate") {
                    Value::lit(mk("num"))
                } else if name == "astype" {
                    // (strings stay the same strings)
                    if astype_dtype(an, &c.arg(0, Some("dtype"))) == Some("object") {
                        if let [one] = ss.as_slice() {
                            if let Some(v) = vals(st, *one, vkey()).filter(|v| v.may.iter().all(|k| intern::key_is_str(k) || k == none_key())) {
                                // (NaN becomes "nan")
                                let fix = |ks: &KeySet| KeySet::from_iter(ks.iter().map(|k| if k == none_key() { mk("nan") } else { k }));
                                set_vals(st, sid, vkey(), Some(&Known { may: fix(&v.may), sure: fix(&v.sure) }));
                            }
                        }
                    }
                    match astype_dtype(an, &c.arg(0, Some("dtype"))) {
                        Some(d) => Value::lit(mk(d)),
                        None => Value::data("dtype"),
                    }
                } else {
                    Value::data("dtype")
                };
                st.set_slot(sid, Ns::Meta, dtype_key(vkey()), dt, true);
            }
            Value::obj(sid)
        }
        "rolling" | "expanding" | "ewm" | "resample" => {
            let v = window_of(an, st, c, Vec::new(), None, None, &name);
            if let Some(g) = v.single() {
                st.set_slot(g, Ns::Meta, mk("srcseries"), Value::with_kv(ObjSet::from_vec(ss.clone()), DATA), true);
            }
            v
        }
        "unstack" => match ss.as_slice() {
            [one] => unstack(an, st, c, *one, true),
            _ => open_frame_call(an, st, c),
        },
        "describe" => {
            let dt: Vec<Option<String>> = ss.iter().map(|x| st.slot_opt(*x, Ns::Meta, dtype_key(vkey())).and_then(|v| v.kv.as_lit()).and_then(intern::key_as_str).map(|s0| s0.to_string())).collect();
            let row = if dt.iter().all(|d| matches!(d.as_deref(), Some("int") | Some("float") | Some("num"))) {
                describe_labels(true, false).row()
            } else if dt.iter().all(|d| d.is_some()) && dt.iter().all(|d| !matches!(d.as_deref(), Some("int") | Some("float") | Some("num"))) {
                describe_labels(false, true).row()
            } else {
                Row::with_source(Source::new("describe", SourceKind::Data))
            };
            series_like(an, st, c, &ss, row, "d")
        }
        "unique" | "tolist" | "to_list" | "to_numpy" => Value::data(&format!("{}@L{}", name, c.line)),
        _ if SERIES_SCALAR_OPS.contains(&name.as_str()) => Value::data(&format!("{}@L{}", name, c.line)),
        _ => series(an),
    }
}

/// The dtype `astype(x)` gives: a builtin type or a dtype name.
fn astype_dtype(an: &Analyzer, v: &Option<Value>) -> Option<&'static str> {
    let v = v.as_ref()?;
    let name: String = if let Some(k) = v.kv.as_lit().filter(|_| v.pts.is_empty()) {
        intern::key_as_str(k)?.to_string()
    } else {
        let o = v.single()?;
        if an.kind(o) != Kind::External {
            return None;
        }
        last(&an.external_qual(o)).to_string()
    };
    Some(match name.as_str() {
        "int" | "int64" | "int32" | "Int64" | "int8" | "int16" | "uint8" => "int",
        "float" | "float64" | "float32" | "Float64" => "float",
        "bool" | "boolean" => "bool",
        "str" | "object" | "string" | "category" => "object",
        _ => return None,
    })
}

const SERIES_SCALAR_OPS: &[&str] = &[
    "sum", "mean", "min", "max", "count", "median", "std", "var", "nunique", "any", "all", "prod", "sem", "skew", "kurt", "quantile", "item", "squeeze", "len", "first_valid_index", "last_valid_index", "memory_usage", "equals",
];

/// `s.str.<m>(…)` / `s.dt.<m>(…)`.
fn accessor_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual).to_string();
    let acc = c.qual.trim_start_matches("Series.").split('.').next().unwrap_or("").to_string();
    let base = c.recv.clone().unwrap_or_default();
    let idx_of = |an: &Analyzer, _st: &State| -> Option<u32> { base.pts.iter().find(|o| is_series_obj(an, *o)) };
    let expand = key_is_bool(&c.kw("expand"), true);
    if acc == "str" && name == "extract" || name == "extractall" {
        let pat = lit_key(&c.arg(0, Some("pat"))).and_then(intern::key_as_str);
        let re = pat.as_ref().and_then(|p| super::minire::Regex::new(p));
        let one_series = !key_is_bool(&c.kw("expand"), true) && c.kw("expand").is_some();
        return match re {
            Some(r) => {
                let (names, n) = r.groups();
                if one_series && n == 1 {
                    return series(an);
                }
                let mut row = Row::new();
                for (i, g) in names.iter().enumerate() {
                    let k = match g {
                        Some(nm) => key_str_lit(nm),
                        None => intern::key_int(i as i64),
                    };
                    row = row.store(k, true);
                }
                new_frame(an, st, c, row, None, None)
            }
            None => open_frame_call(an, st, c),
        };
    }
    if acc == "str" && (name == "split" || name == "rsplit") && expand {
        let n = c.kw("n").or_else(|| c.arg(1, None)).and_then(|v| v.kv.as_lit()).and_then(intern::key_as_int);
        return match n {
            Some(n) if n >= 0 => {
                let mut d = Row::closed_with(KeySet::from_iter((0..=n).map(intern::key_int))).data();
                d.definite = KeySet::one(intern::key_int(0));
                d.likely = d.definite.clone();
                new_frame(an, st, c, Row::from_data(d), None, None)
            }
            _ => {
                let mut d = Row::with_source(Source::new(&format!("split@L{}", c.line), SourceKind::Call)).data();
                d.present = KeySet::one(intern::key_int(0));
                d.definite = d.present.clone();
                d.likely = d.present.clone();
                new_frame(an, st, c, Row::from_data(d), None, None)
            }
        };
    }
    if acc == "str" && (name == "partition" || name == "rpartition") && (expand || c.kw("expand").is_none()) {
        return new_frame(an, st, c, Row::closed_with(KeySet::from_iter((0..3).map(intern::key_int))), None, None);
    }
    if acc == "str" && name == "get_dummies" {
        return open_frame_call(an, st, c);
    }
    if acc == "dt" && name == "isocalendar" {
        return new_frame(an, st, c, closed_ordered(&[mk("year"), mk("week"), mk("day")]), None, None);
    }
    if acc == "dt" && name == "components" {
        let ks: Vec<KeyId> = ["days", "hours", "minutes", "seconds", "milliseconds", "microseconds", "nanoseconds"].iter().map(|x| mk(x)).collect();
        return new_frame(an, st, c, closed_ordered(&ks), None, None);
    }
    // element-wise: the same index and name
    let ss: Vec<u32> = idx_of(an, st).into_iter().collect();
    if ss.is_empty() {
        return series(an);
    }
    let idx = st.row(ss[0], Ns::Item);
    let sid = series_like_id(an, st, c, &ss, idx, "s");
    set_vals(st, sid, vkey(), None);
    st.set_slot(sid, Ns::Meta, dtype_key(vkey()), Value::data("dtype"), true);
    Value::obj(sid)
}

fn frames(an: &Analyzer, v: Option<&Value>) -> Vec<u32> {
    match v {
        Some(v) => v.pts.iter().filter(|o| an.kind(*o) == Kind::Frame).collect(),
        None => Vec::new(),
    }
}

/// The source's index levels (no slot: one unnamed level).
fn copy_meta(src: u32, dst: u32, state: &mut State) {
    let lv = index_levels(state, src);
    set_index_levels(state, dst, lv.as_deref());
}

// ------------------------------------------------------------- what literal data tells
// Frames built from literal data carry, in Meta slots: the row count (`nrows`), the row
// labels (`labels` = every label that may be there, `labels!` = surely there; no slot =
// unknown) and per column its literal values (`("vals", c)` / `("vals!", c)`) and dtype.
// Ops copy them (`derive`), weaken them (row subsets) or drop them.

/// A set of literals: those that may occur and those that surely do.
#[derive(Clone, Debug, PartialEq)]
pub struct Known {
    pub may: KeySet,
    pub sure: KeySet,
}

impl Known {
    pub fn exact(ks: KeySet) -> Known {
        Known { may: ks.clone(), sure: ks }
    }
    /// A subset of these (rows filtered).
    pub fn subset(&self) -> Known {
        Known { may: self.may.clone(), sure: KeySet::new() }
    }
    pub fn row(&self) -> Row {
        let mut d = Row::closed_with(self.may.clone()).data();
        d.definite = self.sure.clone();
        d.likely = self.sure.clone();
        Row::from_data(d)
    }
    pub fn join(&self, o: &Known) -> Known {
        Known { may: self.may.union(&o.may), sure: self.sure.intersection(&o.sure) }
    }
}

fn mk(a: &str) -> KeyId {
    key_str_lit(a)
}

fn get_known(st: &State, f: u32, may: KeyId, sure: KeyId) -> Option<Known> {
    let m = st.slot_opt(f, Ns::Meta, may)?;
    if !m.pts.is_empty() || !m.kv.is_literal() {
        return None;
    }
    let s = st.slot_opt(f, Ns::Meta, sure).filter(|v| v.pts.is_empty() && v.kv.is_literal()).map(|v| kv_literals(&v.kv)).unwrap_or_default();
    Some(Known { may: kv_literals(&m.kv), sure: s })
}

fn set_known(st: &mut State, f: u32, may: KeyId, sure: KeyId, k: Option<&Known>) {
    match k {
        Some(k) => {
            st.set_slot(f, Ns::Meta, may, Value::kv_only(KeyVal::Lits(k.may.clone())), true);
            st.set_slot(f, Ns::Meta, sure, Value::kv_only(KeyVal::Lits(k.sure.clone())), true);
        }
        None => {
            st.set_slot(f, Ns::Meta, may, Value::data("unknown"), true);
            st.set_slot(f, Ns::Meta, sure, Value::data("unknown"), true);
        }
    }
}

pub fn labels(st: &State, f: u32) -> Option<Known> {
    get_known(st, f, mk("labels"), mk("labels!"))
}

pub fn set_labels(st: &mut State, f: u32, k: Option<&Known>) {
    set_known(st, f, mk("labels"), mk("labels!"), k)
}

fn vals_keys(col: KeyId) -> (KeyId, KeyId) {
    (intern::key_tuple(vec![mk("vals"), col]), intern::key_tuple(vec![mk("vals!"), col]))
}

pub fn vals(st: &State, f: u32, col: KeyId) -> Option<Known> {
    let (a, b) = vals_keys(col);
    get_known(st, f, a, b)
}

pub fn set_vals(st: &mut State, f: u32, col: KeyId, k: Option<&Known>) {
    let (a, b) = vals_keys(col);
    set_known(st, f, a, b, k)
}

pub fn nrows(st: &State, f: u32) -> Option<i64> {
    st.slot_opt(f, Ns::Meta, mk("nrows")).filter(|v| v.pts.is_empty()).and_then(|v| v.kv.as_lit()).and_then(intern::key_as_int)
}

pub fn set_nrows(st: &mut State, f: u32, n: Option<i64>) {
    let v = match n {
        Some(n) => Value::lit(intern::key_int(n)),
        None => Value::data("nrows"),
    };
    st.set_slot(f, Ns::Meta, mk("nrows"), v, true);
}

/// How a derived frame relates to its source's rows.
#[derive(Clone, Copy, PartialEq)]
pub enum Rows {
    /// the same rows (copy, sort, column ops)
    Same,
    /// some of the rows (filters, head, dropna)
    Subset,
    /// unrelated rows: nothing carried
    Other,
}

/// Carry what is known of `src`'s rows and of the columns `cols` (renamed by `rename`) to
/// `dst`.
pub fn derive(st: &mut State, src: u32, dst: u32, rows: Rows, cols: &KeySet) {
    if rows == Rows::Other {
        return;
    }
    let lab = labels(st, src).map(|k| if rows == Rows::Same { k } else { k.subset() });
    if lab.is_some() {
        set_labels(st, dst, lab.as_ref());
    }
    if rows == Rows::Same {
        if let Some(n) = nrows(st, src) {
            set_nrows(st, dst, Some(n));
        }
    }
    for i in 0..4 {
        let (a, _) = lvals_keys(i);
        if st.slot_opt(src, Ns::Meta, a).is_none() {
            break;
        }
        let v = lvals(st, src, i).map(|k| if rows == Rows::Same { k } else { k.subset() });
        set_lvals(st, dst, i, v.as_ref());
    }
    if let Some(v) = st.slot_opt(src, Ns::Meta, mk("colname")) {
        st.set_slot(dst, Ns::Meta, mk("colname"), v, true);
    }
    for c in cols.iter() {
        if let Some(v) = vals(st, src, c) {
            let v = if rows == Rows::Same { v } else { v.subset() };
            set_vals(st, dst, c, Some(&v));
        }
        if let Some(v) = st.slot_opt(src, Ns::Meta, dtype_key(c)) {
            st.set_slot(dst, Ns::Meta, dtype_key(c), v, true);
        }
    }
    copy_meta(src, dst, st);
}

// ------------------------------------------------------------- index levels
// Meta "index": the names of the index levels in order, as one literal tuple (None = an
// unnamed level); no slot = one unnamed level (a fresh frame); data = unknown.

fn none_key() -> KeyId {
    intern::key(intern::Key::None)
}

pub fn index_levels(st: &State, f: u32) -> Option<Vec<Option<KeyId>>> {
    let Some(v) = st.slot_opt(f, Ns::Meta, mk("index")) else { return Some(vec![None]) };
    if !v.pts.is_empty() {
        return None;
    }
    let k = v.kv.as_lit()?;
    let items = intern::with_key(k, |kk| if let intern::Key::Tuple(t) = kk { Some(t.clone()) } else { None });
    let dec = |x: KeyId| if x == none_key() { None } else { Some(x) };
    Some(match items {
        Some(t) => t.into_iter().map(dec).collect(),
        None => vec![dec(k)],
    })
}

pub fn set_index_levels(st: &mut State, f: u32, levels: Option<&[Option<KeyId>]>) {
    let v = match levels {
        Some(ls) => Value::lit(intern::key_tuple(ls.iter().map(|x| x.unwrap_or_else(none_key)).collect())),
        None => Value::data("index levels"),
    };
    st.set_slot(f, Ns::Meta, mk("index"), v, true);
}

/// The levels of several frames: equal, or unknown.
fn levels_of(st: &State, fs: &[u32]) -> Option<Vec<Option<KeyId>>> {
    let mut out: Option<Vec<Option<KeyId>>> = None;
    for f in fs {
        let l = index_levels(st, *f)?;
        match &out {
            None => out = Some(l),
            Some(o) if *o == l => {}
            Some(_) => return None,
        }
    }
    out
}

/// The column a level becomes in `reset_index`: its name, else "index" (one level) or
/// "level_i".
fn level_column(levels: &[Option<KeyId>], i: usize) -> KeyId {
    match levels[i] {
        Some(k) => k,
        None if levels.len() == 1 => mk("index"),
        None => mk(&format!("level_{}", i)),
    }
}

fn lvals_keys(i: usize) -> (KeyId, KeyId) {
    let t = |a: &str| intern::key_tuple(vec![mk(a), intern::key_int(i as i64)]);
    (t("lvals"), t("lvals!"))
}

/// The values of index level i (a MultiIndex level or the single level), when known.
pub fn lvals(st: &State, f: u32, i: usize) -> Option<Known> {
    let (a, b) = lvals_keys(i);
    get_known(st, f, a, b).or_else(|| if i == 0 && index_levels(st, f).map(|l| l.len() == 1).unwrap_or(false) { labels(st, f) } else { None })
}

pub fn set_lvals(st: &mut State, f: u32, i: usize, k: Option<&Known>) {
    let (a, b) = lvals_keys(i);
    set_known(st, f, a, b, k)
}

/// The columns' axis name (`df.columns.name`, set by unstack / pivot / crosstab colnames=).
fn colname(st: &State, f: u32) -> Option<KeyId> {
    st.slot_opt(f, Ns::Meta, mk("colname")).filter(|v| v.pts.is_empty()).and_then(|v| v.kv.as_lit()).filter(|k| *k != none_key())
}

/// A row of the known keys, or open from `srcs` (unknown data-derived names).
fn known_or_open(k: Option<Known>, srcs: &Row, line: u32, tag: &str) -> Row {
    match k {
        Some(k) => k.row(),
        None => {
            // names from the data: no column of an input is asked for
            let _ = srcs;
            let mut d = Row::new().data();
            d.sources = intern::IdSet::one((Source::new(&format!("{}@L{}", tag, line), SourceKind::Call), EMPTY_LENS));
            Row::from_data(d)
        }
    }
}

/// The values of `col` over several frames / Series (all known, joined), else None.
fn vals_join(st: &State, objs: &[u32], col: KeyId) -> Option<Known> {
    let mut out: Option<Known> = None;
    for o in objs {
        let k = vals(st, *o, col)?;
        out = Some(match out {
            None => k,
            Some(p) => p.join(&k),
        });
    }
    out
}

/// RangeIndex labels 0..n-1.
fn range_labels(n: i64) -> Known {
    Known::exact(KeySet::from_iter((0..n.max(0)).map(intern::key_int)))
}

// ------------------------------------------------------------- column dtypes (literal data)
fn dtype_key(col: KeyId) -> KeyId {
    intern::key_tuple(vec![key_str_lit("dtype"), col])
}

/// The dtype ("int", "float", "bool", "object") of a column made of these literals.
fn dtype_of_lits(ks: &KeySet) -> Option<&'static str> {
    let (mut i, mut f, mut b, mut st, mut none) = (false, false, false, false, false);
    for k in ks.iter() {
        intern::with_key(k, |kk| match kk {
            intern::Key::Int(_) => i = true,
            intern::Key::Float(_) => f = true,
            intern::Key::Bool(_) => b = true,
            intern::Key::Str(_) => st = true,
            intern::Key::None => none = true,
            _ => st = true,
        });
    }
    Some(if st || (b && (i || f || none)) {
        "object"
    } else if b {
        "bool"
    } else if f || (i && none) {
        "float"
    } else if i {
        "int"
    } else {
        // empty or all None
        "object"
    })
}

/// The dtype of a column holding `v` (a list / tuple of literals or one literal), if known.
fn dtype_of_value(an: &Analyzer, st: &State, v: &Value) -> Option<&'static str> {
    if let Some(o) = v.single().filter(|o| is_series_obj(an, *o)) {
        let d = st.slot_opt(o, Ns::Meta, dtype_key(vkey())).and_then(|x| x.kv.as_lit()).and_then(intern::key_as_str);
        return match d.as_deref() {
            Some("int") => Some("int"),
            Some("float") => Some("float"),
            Some("bool") => Some("bool"),
            Some("object") => Some("object"),
            Some("num") => Some("num"),
            _ => None,
        };
    }
    if v.pts.is_empty() {
        return if v.kv.is_literal() { dtype_of_lits(&kv_literals(&v.kv)) } else { None };
    }
    let o = v.single()?;
    if !matches!(an.kind(o), Kind::List | Kind::Tuple) {
        return None;
    }
    if an.llen(o, st) == Some(0) {
        return Some("object");
    }
    match st.elems.get(&o) {
        Some(kv) if kv.is_literal() => dtype_of_lits(&kv_literals(&kv)),
        _ => None,
    }
}

fn set_dtype(st: &mut State, f: u32, col: KeyId, d: Option<&'static str>, strong: bool) {
    let v = match d {
        Some(d) => Value::lit(key_str_lit(d)),
        None => Value::data("dtype"),
    };
    st.set_slot(f, Ns::Meta, dtype_key(col), v, strong);
}

fn dtype(st: &State, f: u32, col: KeyId) -> Option<String> {
    st.slot_opt(f, Ns::Meta, dtype_key(col)).and_then(|v| v.kv.as_lit()).and_then(intern::key_as_str).map(|s| s.to_string())
}

/// Row subsets and projections keep each column's dtype.
fn copy_dtypes(st: &mut State, src: u32, dst: u32, cols: &KeySet) {
    for c in cols.iter() {
        if let Some(v) = st.slot_opt(src, Ns::Meta, dtype_key(c)) {
            st.set_slot(dst, Ns::Meta, dtype_key(c), v, true);
        }
    }
}

/// A frame derived from the receiver's frames: what is known of its rows (and, with
/// `with_vals`, of the kept columns' values) carries over from a single source.
fn derived(an: &mut Analyzer, st: &mut State, c: &Call, row: Row, fs: &[u32], rows: Rows, with_vals: bool) -> Value {
    let v = new_frame(an, st, c, row.clone(), None, None);
    if let (Some(nid), [f]) = (v.single(), fs) {
        let cols = if with_vals { row.present.clone() } else { KeySet::new() };
        derive(st, *f, nid, rows, &cols);
    }
    v
}

/// A new frame. What its index is stays unknown unless the op says (`fresh_index` for a
/// new RangeIndex, `derive` / `src` for the source's).
fn new_frame(an: &mut Analyzer, st: &mut State, c: &Call, row: Row, label: Option<&str>, src: Option<u32>) -> Value {
    let label = label.map(|l| l.to_string()).unwrap_or_else(|| format!("frame@L{}", c.line));
    let nid = an.alloc_at(Kind::Frame, &c.node, st, &label, Payload::None, Some(row), Ns::Item, None, None);
    set_index_levels(st, nid, None);
    set_labels(st, nid, None);
    set_nrows(st, nid, None);
    if let Some(s) = src {
        copy_meta(s, nid, st);
    }
    Value::obj(nid)
}

/// The frame has a new RangeIndex (one unnamed level).
fn fresh_index(st: &mut State, v: &Value) {
    if let Some(nid) = v.single() {
        set_index_levels(st, nid, Some(&[None]));
    }
}

/// A frame's columns for a model: a symbolic row (a frame from before the call, inside a
/// summary) is open beyond its concrete part.
fn frame_row(st: &State, f: u32) -> Row {
    let r = st.row(f, Ns::Item);
    if !r.symbolic() {
        return r;
    }
    Row::from_data(RowData {
        present: r.present.clone(),
        definite: r.definite.clone(),
        likely: r.likely.clone(),
        sources: r.sources.insert((Source::new("frame before the call", SourceKind::Call), EMPTY_LENS)),
        mutators: r.mutators.clone(),
        ..Default::default()
    })
}

fn recv_row(an: &Analyzer, st: &State, c: &Call) -> Row {
    let fs = frames(an, c.recv.as_ref());
    if fs.is_empty() {
        Row::top()
    } else {
        join_rows(fs.iter().map(|f| frame_row(st, *f)))
    }
}

fn open_frame_from(an: &mut Analyzer, oid: u32, node: &Node, state: &mut State, op: &str) -> Value {
    let row = state.row(oid, Ns::Item);
    let r = Row::from_data(RowData { sources: intern::IdSet::one((Source::new(&format!("{}@L{}", op, node.line), SourceKind::Call), EMPTY_LENS)), mutators: row.mutators.clone(), ..Default::default() });
    let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(r), Ns::Item, None, None);
    set_index_levels(state, nid, None);
    Value::obj(nid)
}

fn lit_key(v: &Option<Value>) -> Option<KeyId> {
    v.as_ref().and_then(|x| x.kv.as_lit())
}

/// The literal keys of a single list / tuple in order (`["b", "a"]`), or None.
pub fn lit_key_seq(an: &Analyzer, st: &State, v: &Value) -> Option<Vec<KeyId>> {
    let o = v.single()?;
    if !matches!(an.kind(o), Kind::List | Kind::Tuple) {
        return None;
    }
    let n = an.llen(o, st)?;
    (0..n).map(|i| st.slot(o, Ns::Pos, intern::key_int(i as i64)).kv.as_lit().filter(|_| true)).collect()
}

/// The keys a list of column names surely holds and those it may hold (one appended on
/// some paths only, items past a known prefix); None when not literal.
fn list_keys_split(an: &mut Analyzer, st: &State, v: &Value) -> Option<(KeySet, KeySet)> {
    let all = an.lit_keys(v, st)?;
    if v.pts.is_empty() {
        let sure = if matches!(v.kv, KeyVal::Lit(_)) { all.clone() } else { KeySet::new() };
        return Some((all, sure));
    }
    let mut sure: Option<KeySet> = None;
    for o in v.pts.iter() {
        let s1 = match an.kind(o) {
            Kind::List | Kind::Tuple => {
                let n = an.llen(o, st).or(an.prefix(o, st)).unwrap_or(0);
                KeySet::from_iter((0..n).filter_map(|i| {
                    let x = st.slot(o, Ns::Pos, intern::key_int(i as i64));
                    if x.pts.is_empty() { if let KeyVal::Lit(k) = x.kv { return Some(k) } }
                    None
                }))
            }
            Kind::Set => KeySet::new(),
            _ => st.row(o, Ns::Item).definite.clone(),
        };
        sure = Some(match sure {
            None => s1,
            Some(p0) => p0.intersection(&s1),
        });
    }
    Some((all, sure.unwrap_or_default()))
}

/// A closed row with exactly `ks`, in this column order.
fn closed_ordered(ks: &[KeyId]) -> Row {
    Row::closed_with(KeySet::from_iter(ks.iter().copied())).with_order(ks.to_vec())
}

/// Known bounds of a slice value (`slice[lo:hi:step]@L…`); None = unknown.
fn slice_bounds(v: &Value) -> Option<(Option<i64>, Option<i64>, Option<i64>)> {
    let KeyVal::Data(s) = &v.kv else { return None };
    if !v.pts.is_empty() {
        return None;
    }
    let s = intern::str_get(*s);
    let inner = s.strip_prefix("slice[")?.split(']').next()?.to_string();
    let parts: Vec<&str> = inner.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let p = |x: &str| -> Option<Option<i64>> { if x.is_empty() { Some(None) } else { x.parse().ok().map(Some) } };
    Some((p(parts[0])?, p(parts[1])?, p(parts[2])?))
}

fn is_slice(v: &Value) -> bool {
    matches!(&v.kv, KeyVal::Data(s) if v.pts.is_empty() && intern::str_get(*s).starts_with("slice"))
}

/// Python's slice of n positions.
fn slice_positions(n: usize, (lo, hi, step): (Option<i64>, Option<i64>, Option<i64>)) -> Option<Vec<usize>> {
    let n = n as i64;
    let step = step.unwrap_or(1);
    if step == 0 {
        return None;
    }
    let norm = |x: i64| if x < 0 { x + n } else { x };
    let mut out = Vec::new();
    if step > 0 {
        let lo = lo.map(norm).unwrap_or(0).clamp(0, n);
        let hi = hi.map(norm).unwrap_or(n).clamp(0, n);
        let mut i = lo;
        while i < hi {
            out.push(i as usize);
            i += step;
        }
    } else {
        let lo = lo.map(norm).unwrap_or(n - 1).clamp(-1, n - 1);
        let hi = hi.map(norm).unwrap_or(-1).clamp(-1, n - 1);
        let mut i = lo;
        while i > hi {
            out.push(i as usize);
            i += step;
        }
    }
    Some(out)
}

/// The columns at known positions of a row: exact when the column order is known; else the
/// row's columns, none definite.
fn select_positions(row: &Row, pick: &dyn Fn(usize) -> Option<Vec<usize>>) -> Row {
    if let Some(order) = row.ordered_keys().filter(|_| row.closed()) {
        if let Some(pos) = pick(order.len()) {
            let ks: Vec<KeyId> = pos.iter().map(|i| order[*i]).collect();
            return closed_ordered(&ks);
        }
    }
    let mut d = Row::new().copy_from(row, false, &KeySet::new(), EMPTY_LENS).data();
    d.sources = row.sources.clone();
    d.top = row.top;
    Row::from_data(d)
}

fn key_is(v: &Option<Value>, s: &str) -> bool {
    lit_key(v).and_then(intern::key_as_str).map(|k| &*k == s).unwrap_or(false)
}

fn key_is_bool(v: &Option<Value>, b: bool) -> bool {
    lit_key(v).map(|k| intern::with_key(k, |kk| *kk == intern::Key::Bool(b))).unwrap_or(false)
}

/// axis == 1 or "columns"
fn axis_is_columns(v: &Option<Value>) -> bool {
    lit_key(v).map(|k| intern::with_key(k, |kk| matches!(kk, intern::Key::Int(1)) || matches!(kk, intern::Key::Str(s) if &**s == "columns"))).unwrap_or(false)
}

fn axis_is_index(v: &Option<Value>) -> bool {
    lit_key(v).map(|k| intern::with_key(k, |kk| matches!(kk, intern::Key::Int(0)) || matches!(kk, intern::Key::Str(s) if &**s == "index"))).unwrap_or(false)
}

/// `io.StringIO(text)`: the text itself (a `to_json` text keeps its frame).
fn string_io(an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    match c.args.first() {
        Some(v) => v.clone(),
        None => { let _ = an; Value::data(&format!("StringIO@L{}", c.line)) }
    }
}

/// `df.to_json(orient=…)`: a text that remembers (a snapshot of) the frame and the orient.
fn to_json(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let path = c.arg(0, Some("path_or_buf"));
    if path.map(|p| p.kv != an.none_v.kv || !p.pts.is_empty()).unwrap_or(false) {
        return an.none_v.clone();
    }
    let orient = lit_key(&c.kw("orient")).and_then(intern::key_as_str).map(|s0| s0.to_string()).unwrap_or_else(|| "columns".into());
    let mut out = Value::new();
    for (j, f) in fs.iter().enumerate() {
        let row = st.row(*f, Ns::Item);
        let snap = an.alloc(Kind::Frame, &format!("{}:json{}", crate::interp::site_string(&c.node), j), st, &format!("frame@L{}", c.line), Payload::None, Some(row.clone()), Ns::Item, None, None);
        derive(st, *f, snap, Rows::Same, &row.present);
        out = out.join(&Value::data(&format!("jsontext:{}:{}", snap, orient)));
    }
    if out.kv == KeyVal::Bot {
        return Value::data("json");
    }
    out
}

/// The frames (and orients) a `to_json` text holds.
pub fn json_texts(v: &Value) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let ks: Vec<crate::intern::StrId> = match &v.kv {
        KeyVal::Data(s0) => vec![*s0],
        _ => Vec::new(),
    };
    for s0 in ks {
        let t = intern::str_get(s0);
        if let Some(rest) = t.strip_prefix("jsontext:") {
            let mut it = rest.splitn(2, ':');
            if let (Some(o), Some(orient)) = (it.next().and_then(|x| x.parse().ok()), it.next()) {
                out.push((o, orient.to_string()));
            }
        }
    }
    out
}

/// `json.loads` of a `to_json` text: the structure its orient gives.
pub fn json_of_frame(an: &mut Analyzer, st: &mut State, c: &Call, f: u32, orient: &str) -> Value {
    let base = st.row(f, Ns::Item);
    let labs = labels_row(st, f);
    let rec_row = Row::new().copy_from(&base, true, &KeySet::new(), EMPTY_LENS);
    match orient {
        "records" => {
            let vrow = dict_with(an, st, &c.node, "jrec", rec_row, &|_| Value::data("cell"));
            let site = format!("{}:jrecs", crate::interp::site_string(&c.node));
            let lid = an.alloc(Kind::List, &site, st, &format!("records@L{}", c.line), Payload::None, None, Ns::Item, None, None);
            let n = nrows(st, f);
            an.set_llen(lid, st, n.map(|n| n as usize));
            if let Some(n) = n {
                for i in 0..n {
                    st.set_slot(lid, Ns::Pos, intern::key_int(i), Value::obj(vrow), true);
                }
            }
            st.set_slot(lid, Ns::Elem, intern::STAR, Value::obj(vrow), true);
            st.elems.insert(lid, DATA);
            Value::obj(lid)
        }
        "index" => {
            let rec = dict_with(an, st, &c.node, "jr", rec_row, &|_| Value::data("cell"));
            Value::obj(dict_with(an, st, &c.node, "ji", labs, &|_| Value::obj(rec)))
        }
        "split" => Value::obj(dict_with(an, st, &c.node, "js", closed_ordered(&[mk("columns"), mk("index"), mk("data")]), &|_| Value::data("split"))),
        "table" => Value::obj(dict_with(an, st, &c.node, "jt", closed_ordered(&[mk("schema"), mk("data")]), &|_| Value::data("table"))),
        "values" => Value::data("values"),
        _ => {
            let inner = dict_with(an, st, &c.node, "jc", labs, &|_| Value::data("cell"));
            Value::obj(dict_with(an, st, &c.node, "jo", rec_row, &|_| Value::obj(inner)))
        }
    }
}

fn read_frame(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // (a StringIO: read as its text)
    if c.args.first().map(|a| !a.pts.is_empty()).unwrap_or(false) {
        let t = crate::models::csvio::unwrap_text(an, st, &c.args[0]);
        if t != c.args[0] {
            let mut c2 = c.clone();
            c2.args[0] = t;
            return read_frame(an, st, &c2);
        }
    }
    // a `to_json` text read back: the frame's columns
    if c.qual.ends_with("read_json") {
        let texts = c.args.first().map(json_texts).unwrap_or_default();
        if !texts.is_empty() {
            let mut rows = Vec::new();
            for (f, orient) in &texts {
                let r = st.row(*f, Ns::Item);
                rows.push(if orient == "values" { Row::with_source(Source::new(&format!("read_json@L{}", c.line), SourceKind::Call)) } else { Row::new().copy_from(&r, true, &KeySet::new(), EMPTY_LENS) });
            }
            let v = new_frame(an, st, c, join_rows(rows.into_iter()), None, None);
            return v;
        }
    }
    let tag = frame_producer(&c.qual).unwrap_or_else(|| last(&c.qual)).to_string();
    let src = Source::new(&format!("{}@L{}", tag, c.line), SourceKind::External);
    let site = intern::s(&an.access_site(&c.node));
    let mut row = Row::with_source(src);
    if let Some(ks) = c.kw("usecols").and_then(|u| an.lit_keys(&u, st)) {
        // the file must have every listed column (ValueError otherwise)
        for k in intern::sorted_by_str(ks.iter()) {
            st.require(src, k, site, intern::IdSet::new(), intern::IdSet::new(), intern::IdSet::new());
        }
        row = Row::closed_with(ks);
    }
    if let Some(n) = c.kw("names") {
        if let Some(seq) = lit_key_seq(an, st, &n) {
            row = closed_ordered(&seq);
        } else if let Some(ks) = an.lit_keys(&n, st) {
            row = Row::closed_with(ks);
        }
    }
    let mut index: Option<KeySet> = None;
    if let Some(ic) = c.kw("index_col") {
        match an.lit_keys(&ic, st) {
            Some(ks) if ks.iter().all(intern::key_is_str) => {
                for k in intern::sorted_by_str(ks.iter()) {
                    if row.open() {
                        st.require(src, k, site, intern::IdSet::new(), intern::IdSet::new(), intern::IdSet::new());
                    }
                    row = row.delete(k, true);
                }
                index = Some(ks);
            }
            _ => {
                if !key_is_bool(&Some(ic.clone()), false) && ic.kv != an.none_v.kv {
                    // a column by position leaves: which one is not known
                    let mut d = row.data();
                    d.definite = KeySet::new();
                    d.likely = d.likely.union(&d.present);
                    row = Row::from_data(d);
                }
            }
        }
    }
    let v = new_frame(an, st, c, row, Some(&format!("{}@L{}", tag, c.line)), None);
    if c.kw("index_col").map(|ic| ic.kv == an.none_v.kv || key_is_bool(&Some(ic), false)).unwrap_or(true) {
        fresh_index(st, &v);
    }
    if let (Some(ks), Some(nid)) = (index, v.pts.first()) {
        let seq: Vec<Option<KeyId>> = match c.kw("index_col").and_then(|ic| lit_key_seq(an, st, &ic)) {
            Some(seq) => seq.into_iter().map(Some).collect(),
            None => intern::sorted_by_str(ks.iter()).into_iter().map(Some).collect(),
        };
        set_index_levels(st, nid, Some(&seq));
    }
    v
}

/// The columns a frame built from one row-like item gets: a dict's keys, a frame's columns
/// (row = Some), a sequence of known length n: 0..n-1 (Err(n)); None = unknown.
fn item_columns(an: &Analyzer, st: &State, v: &Value) -> Option<Vec<Result<Row, usize>>> {
    if v.pts.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    // a record that is one of several dicts: a key all of them have is surely a column, one
    // some of them have maybe
    let mut rec: Option<Row> = None;
    for x in v.pts.iter() {
        match an.kind(x) {
            Kind::Dict | Kind::Frame => {
                let mut d = Row::new().copy_from(&st.row(x, Ns::Item), true, &KeySet::new(), EMPTY_LENS).data();
                // a key the record has is a column of the frame
                d.definite = d.present.clone();
                d.likely = d.likely.union(&d.present);
                let r = Row::from_data(d);
                rec = Some(match rec {
                    None => r,
                    Some(cur) if !crate::interp::disabled("recalts") => cur.join(&r),
                    Some(cur) => cur.copy_from(&r, true, &KeySet::new(), EMPTY_LENS),
                });
            }
            Kind::List | Kind::Tuple => out.push(Err(an.llen(x, st)?)),
            _ => return None,
        }
    }
    if let Some(r) = rec {
        out.push(Ok(r));
    }
    Some(out)
}

/// The columns of a frame made from a sequence of rows: dicts give the union of their keys
/// (missing cells are NaN), sequences 0..max-1.
fn rows_frame(rows: &[Result<Row, usize>]) -> Row {
    let mut row = Row::new();
    let mut lens: Vec<usize> = Vec::new();
    for r in rows {
        match r {
            // (a record's keys: definite where it surely has them, `item_columns`)
            Ok(r) => row = row.copy_from(r, true, &KeySet::new(), EMPTY_LENS),
            Err(n) => lens.push(*n),
        }
    }
    if let Some(m) = lens.iter().max() {
        for i in 0..*m {
            row = row.store(intern::key_int(i as i64), true);
        }
    }
    row
}

fn pd_dataframe(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let data = c.arg(0, Some("data"));
    let from_dict = c.qual.ends_with("from_dict");
    let index_orient = from_dict && key_is(&c.kw("orient").or_else(|| c.arg(1, None)), "index");
    let cols = c.kw("columns").and_then(|v| an.lit_keys(&v, st));
    let col_seq = c.kw("columns").and_then(|v| lit_key_seq(an, st, &v));
    let exclude = c.kw("exclude").and_then(|v| an.lit_keys(&v, st)).unwrap_or_default();
    let open = |an: &mut Analyzer, st: &mut State| new_frame(an, st, c, Row::with_source(Source::new(&format!("DataFrame@L{}", c.line), SourceKind::Call)), None, None);
    let finish = |an: &mut Analyzer, st: &mut State, mut row: Row| {
        for k in exclude.iter() {
            row = row.delete(k, true);
        }
        let v = new_frame(an, st, c, row, None, None);
        fresh_index(st, &v);
        v
    };
    // `columns=` names (sequences) or selects (dicts, frames): the frame has exactly those
    if let Some(ks) = &cols {
        if c.kw("columns").map(|v| v.pts.iter().all(|x| an.kind(x) != Kind::Ph)).unwrap_or(true) {
            return finish(an, st, match &col_seq {
                Some(seq) => closed_ordered(seq),
                None => Row::closed_with(ks.clone()),
            });
        }
    }
    // `DataFrame(**df.to_dict("split"))`: the columns entry
    for sk in &c.star_kwargs {
        for d in sk.pts.iter().filter(|o| an.kind(*o) == Kind::Dict) {
            let cv = st.slot(d, Ns::Item, mk("columns"));
            if let Some(seq) = lit_key_seq(an, st, &cv) {
                return finish(an, st, closed_ordered(&seq));
            }
        }
    }
    // orient="tight": the columns entry
    if from_dict && key_is(&c.kw("orient").or_else(|| c.arg(1, None)), "tight") {
        if let Some(d) = data.as_ref().and_then(|d| d.single()).filter(|d| an.kind(*d) == Kind::Dict) {
            let cv = st.slot(d, Ns::Item, mk("columns"));
            if let Some(seq) = lit_key_seq(an, st, &cv) {
                return finish(an, st, closed_ordered(&seq));
            }
        }
        return open(an, st);
    }
    // a record array (to_records): its fields
    if let Some(rec) = data.as_ref().and_then(|d| d.single()).filter(|d| an.kind(*d) == Kind::Namespace) {
        if let Some(fd) = st.slot_opt(rec, Ns::Meta, mk("recfields")).and_then(|v| v.single()) {
            let r = st.row(fd, Ns::Item);
            return finish(an, st, Row::new().copy_from(&r, true, &KeySet::new(), EMPTY_LENS));
        }
    }
    let Some(data) = data else { return finish(an, st, Row::new()) };
    if data.pts.is_empty() {
        return if data.kv == an.none_v.kv { finish(an, st, Row::new()) } else { open(an, st) };
    }
    let mut row: Option<Row> = None;
    let add = |row: &mut Option<Row>, r: Row| {
        *row = Some(match row.take() {
            None => r,
            Some(p) => p.join(&r),
        });
    };
    for x in data.pts.iter() {
        match an.kind(x) {
            Kind::Dict if index_orient => {
                // {label: {col: v}} / {label: [v, …]}: the inner rows are the frame's rows
                let r = st.row(x, Ns::Item);
                if r.open() {
                    return open(an, st);
                }
                let mut items = Vec::new();
                for k in r.present.iter() {
                    match item_columns(an, st, &st.slot(x, Ns::Item, k)) {
                        Some(cs) => items.extend(cs),
                        None => return open(an, st),
                    }
                }
                add(&mut row, rows_frame(&items));
            }
            Kind::Dict | Kind::Frame => add(&mut row, Row::new().copy_from(&st.row(x, Ns::Item), true, &KeySet::new(), EMPTY_LENS)),
            Kind::List | Kind::Tuple => {
                let it = an.iterate(&Value::obj(x), &c.node, st);
                let items: Vec<Value> = match (&it.items, &it.elem) {
                    (Some(items), _) => items.clone(),
                    (None, Elem::One(e)) => vec![e.clone()],
                    _ => return open(an, st),
                };
                let mut rows = Vec::new();
                for item in &items {
                    match item_columns(an, st, item) {
                        Some(cs) => rows.extend(cs),
                        None => return open(an, st),
                    }
                }
                let mut r = rows_frame(&rows);
                if it.items.is_none() {
                    // rows of unknown count: a key of one row may be no column (no rows)
                    let mut d = r.data();
                    d.definite = KeySet::new();
                    d.likely = d.present.clone();
                    r = Row::from_data(d);
                }
                add(&mut row, r);
            }
            _ => return open(an, st),
        }
    }
    let v = match row {
        Some(r) => finish(an, st, r),
        None => return open(an, st),
    };
    // a dict of columns: each column's dtype from its literal values
    if let (Some(nid), Some(d)) = (v.single(), data.single().filter(|d| an.kind(*d) == Kind::Dict && !index_orient)) {
        for k in st.row(d, Ns::Item).present.iter() {
            let dt = dtype_of_value(an, st, &st.slot(d, Ns::Item, k));
            set_dtype(st, nid, k, dt, true);
        }
    }
    if let (Some(nid), Some(d)) = (v.single(), data.single()) {
        frame_meta_from_data(an, st, c, nid, d, index_orient);
    }
    v
}

/// One cell of literal data: its literal value (a missing cell is NaN: None).
fn cell_lit(v: &Value) -> Option<KeyId> {
    if v.pts.is_empty() {
        v.kv.as_lit()
    } else {
        None
    }
}

/// Row count, labels and column values of a frame built from literal data.
fn frame_meta_from_data(an: &mut Analyzer, st: &mut State, c: &Call, nid: u32, d: u32, index_orient: bool) {
    let cols = st.row(nid, Ns::Item);
    let index_kw = c.kw("index").and_then(|v| lit_key_seq(an, st, &v));
    let mut n: Option<i64> = index_kw.as_ref().map(|i| i.len() as i64);
    let mut labs: Option<Known> = index_kw.as_ref().map(|i| Known::exact(KeySet::from_iter(i.iter().copied())));
    let none = an.none_v.kv.as_lit();
    let mut colvals: Vec<(KeyId, Option<Known>)> = Vec::new();
    match an.kind(d) {
        Kind::Dict if index_orient => {
            let r = st.row(d, Ns::Item);
            if r.closed() && r.present == r.definite {
                labs = labs.or(Some(Known::exact(r.present.clone())));
                n = n.or(Some(r.present.len() as i64));
            }
        }
        Kind::Dict if !is_series_obj(an, d) => {
            let r = st.row(d, Ns::Item);
            let mut lens: Vec<Option<i64>> = Vec::new();
            let mut sidx: Option<Known> = None;
            for k in r.present.iter() {
                let v = st.slot(d, Ns::Item, k);
                if let Some(items) = lit_key_seq(an, st, &v) {
                    lens.push(Some(items.len() as i64));
                    colvals.push((k, Some(Known::exact(KeySet::from_iter(items.iter().copied())))));
                } else if let Some(o) = v.single().filter(|o| is_series_obj(an, *o)) {
                    let sr = st.row(o, Ns::Item);
                    let kn = if sr.closed() { Some(Known { may: sr.present.clone(), sure: sr.definite.clone() }) } else { None };
                    sidx = match (sidx, kn) {
                        (None, k) => k,
                        (Some(a), Some(b)) => Some(Known { may: a.may.union(&b.may), sure: a.sure.union(&b.sure) }),
                        (Some(_), None) => None,
                    };
                    lens.push(None);
                    colvals.push((k, None));
                } else if let Some(l) = cell_lit(&v) {
                    colvals.push((k, Some(Known::exact(KeySet::one(l)))));
                } else {
                    lens.push(None);
                    colvals.push((k, None));
                }
            }
            if n.is_none() && !lens.is_empty() && lens.iter().all(|l| l.is_some() && *l == lens[0]) {
                n = lens[0];
            }
            if labs.is_none() {
                labs = sidx;
            }
        }
        Kind::List | Kind::Tuple => {
            if let Some(len) = an.llen(d, st) {
                n = n.or(Some(len as i64));
                for k in cols.present.iter() {
                    let mut may = KeySet::new();
                    let mut ok = true;
                    for i in 0..len {
                        let item = st.slot(d, Ns::Pos, intern::key_int(i as i64));
                        let cell = match item.single() {
                            Some(o) if an.kind(o) == Kind::Dict => {
                                let rr = st.row(o, Ns::Item);
                                if rr.definite.contains(k) {
                                    cell_lit(&st.slot(o, Ns::Item, k))
                                } else if !rr.present.contains(k) && rr.closed() {
                                    none
                                } else {
                                    None
                                }
                            }
                            Some(o) if matches!(an.kind(o), Kind::List | Kind::Tuple) => intern::key_as_int(k).and_then(|j| cell_lit(&st.slot(o, Ns::Pos, intern::key_int(j)))),
                            _ => None,
                        };
                        match cell {
                            Some(x) => may = may.insert(x),
                            None => ok = false,
                        }
                    }
                    colvals.push((k, if ok { Some(Known::exact(may)) } else { None }));
                }
            }
        }
        _ => {}
    }
    if labs.is_none() {
        labs = n.map(range_labels);
    }
    if an.kind(d) == Kind::Frame {
        if let Some(m) = st.slot_opt(d, Ns::Meta, mk("index")) {
            st.set_slot(nid, Ns::Meta, mk("index"), m, true);
        }
    } else {
        set_index_levels(st, nid, Some(&[None]));
    }
    set_nrows(st, nid, n);
    set_labels(st, nid, labs.as_ref());
    for (k, v) in colvals {
        if cols.present.contains(k) {
            set_vals(st, nid, k, v.as_ref());
        }
    }
}

fn open_frame_call(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let tag = last(&c.qual).to_string();
    new_frame(an, st, c, Row::with_source(Source::new(&format!("{}@L{}", tag, c.line), SourceKind::Call)), None, None)
}

/// The columns one concatenated piece contributes: a frame's, or a Series' name (axis=1).
/// (`pos`: the piece's key / position, naming an unnamed Series on axis=1.)
fn piece_row_at(an: &Analyzer, st: &State, v: &Value, axis1: bool, pos: Option<KeyId>) -> Option<Row> {
    let mut rows = Vec::new();
    for o in v.pts.iter() {
        if an.kind(o) == Kind::Frame {
            rows.push(st.row(o, Ns::Item));
        } else if is_series_obj(an, o) {
            if axis1 {
                let nm = series_name(an, st, o);
                let named = nm.pts.is_empty() && nm.kv.as_lit().map(|k| k != none_key()).unwrap_or(false);
                rows.push(match (pos, named, name_column(an, &nm)) {
                    (Some(p0), _, _) => Row::closed_with(KeySet::one(p0)),
                    (None, true, Some(k)) => Row::closed_with(KeySet::one(k)),
                    _ => return None,
                });
            } else {
                // Series stacked as rows of a frame: one column named after the Series
                let nm = series_name(an, st, o);
                rows.push(match name_column(an, &nm) {
                    Some(k) => Row::closed_with(KeySet::one(k)),
                    None => return None,
                });
            }
        } else {
            return None;
        }
    }
    if rows.is_empty() {
        None
    } else {
        Some(join_rows(rows.into_iter()))
    }
}

fn concat(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(objs) = c.arg(0, Some("objs")) else { return open_frame_call(an, st, c) };
    let axis1 = axis_is_columns(&c.kw("axis"));
    let inner = key_is(&c.kw("join"), "inner");
    // the pieces: a list / tuple, or a dict {key: frame} (its keys are keys=)
    let mut pieces: Vec<Value> = Vec::new();
    let mut keys: Option<Vec<KeyId>> = c.kw("keys").and_then(|k| lit_key_seq(an, st, &k));
    let mut known_count = true;
    if let Some(d) = objs.single().filter(|d| an.kind(*d) == Kind::Dict && !is_series_obj(an, *d)) {
        let r = st.row(d, Ns::Item);
        if !(r.closed() && r.present == r.definite) {
            return open_frame_call(an, st, c);
        }
        let ks = r.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(r.present.iter()));
        for k in &ks {
            pieces.push(st.slot(d, Ns::Item, *k));
        }
        keys = keys.or(Some(ks));
    } else {
        let it = an.iterate(&objs, &c.node, st);
        match (&it.items, &it.elem) {
            (Some(items), _) => pieces = items.clone(),
            (None, Elem::One(e)) => {
                pieces = vec![e.clone()];
                known_count = false;
            }
            _ => return open_frame_call(an, st, c),
        }
    }
    let mut rows: Vec<Row> = Vec::new();
    let all_series = pieces.iter().all(|p0| !p0.pts.is_empty() && p0.pts.iter().all(|o| is_series_obj(an, o)));
    let mut unnamed = 0i64;
    for (i, p0) in pieces.iter().enumerate() {
        // mixed with frames: an unnamed Series is numbered among the unnamed ones
        let is_ser = !p0.pts.is_empty() && p0.pts.iter().all(|o| is_series_obj(an, o));
        if axis1 && !all_series && is_ser {
            let nm = join_values(p0.pts.iter().map(|o| series_name(an, st, o)));
            if nm.pts.is_empty() && nm.kv == an.none_v.kv {
                rows.push(Row::closed_with(KeySet::one(intern::key_int(unnamed))));
                unnamed += 1;
                continue;
            }
        }
        // Series side by side: keys= name them, else their names, unnamed ones their position
        let pos = if axis1 && all_series {
            match &keys {
                Some(ks) => ks.get(i).copied(),
                None => {
                    let nm = join_values(p0.pts.iter().map(|o| series_name(an, st, o)));
                    let unnamed = nm.pts.is_empty() && nm.kv == an.none_v.kv;
                    unnamed.then(|| intern::key_int(i as i64))
                }
            }
        } else {
            None
        };
        match piece_row_at(an, st, p0, axis1, pos) {
            Some(r) => rows.push(r),
            None => return open_frame_call(an, st, c),
        }
    }
    let keys = if axis1 && all_series { None } else { keys };
    if rows.is_empty() {
        return open_frame_call(an, st, c);
    }
    let row = if axis1 {
        match &keys {
            // keys= label each piece's columns: (key, column)
            Some(ks) if ks.len() == rows.len() => {
                let mut out = Row::new();
                let mut open = false;
                for (k, r) in ks.iter().zip(rows.iter()) {
                    for col in r.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(r.present.iter())) {
                        out = out.store(intern::key_tuple(vec![*k, col]), r.definite.contains(col));
                    }
                    open |= r.open();
                }
                if open {
                    let mut d = out.data();
                    d.sources = rows.iter().fold(intern::IdSet::new(), |a, r| a.union(&r.sources));
                    out = Row::from_data(d);
                }
                out
            }
            _ => {
                let mut out = Row::new();
                for r in &rows {
                    out = out.copy_from(r, true, &KeySet::new(), EMPTY_LENS);
                }
                out
            }
        }
    } else if inner {
        let mut row = rows[0].clone();
        for r in &rows[1..] {
            row = Row::from_data(RowData {
                present: row.present.intersection(&r.present),
                definite: row.definite.intersection(&r.definite),
                likely: row.likely.intersection(&r.likely),
                sources: row.sources.union(&r.sources),
                mutators: row.mutators.union(&r.mutators),
                top: row.top || r.top,
                ..Default::default()
            });
        }
        row
    } else {
        // rows stacked: the union of the columns, each NaN-filled where a piece lacks it
        let mut out = Row::new();
        for r in &rows {
            let mut d = Row::new().copy_from(r, true, &KeySet::new(), EMPTY_LENS).data();
            d.definite = d.present.clone();
            d.likely = d.likely.union(&d.present);
            out = out.copy_from(&Row::from_data(d), true, &KeySet::new(), EMPTY_LENS);
        }
        out
    };
    let mut row = row;
    if !known_count {
        // pieces of unknown count (maybe none: ValueError, maybe one)
        let mut d = row.data();
        d.definite = KeySet::new();
        row = Row::from_data(d);
    }
    let v = new_frame(an, st, c, row, None, None);
    if let Some(nid) = v.single() {
        set_labels(st, nid, None);
        let piece_frames: Vec<u32> = pieces.iter().flat_map(|p0| p0.pts.iter().filter(|o| an.kind(*o) == Kind::Frame).collect::<Vec<_>>()).collect();
        let ignore = key_is_bool(&c.kw("ignore_index"), true);
        if axis1 {
            let lv = levels_of(st, &piece_frames);
            set_index_levels(st, nid, if piece_frames.len() == pieces.len() { lv.as_deref() } else { None });
        } else if ignore {
            set_index_levels(st, nid, Some(&[None]));
        } else {
            match levels_of(st, &piece_frames).filter(|_| piece_frames.len() == pieces.len()) {
                Some(mut lv) => {
                    if keys.is_some() {
                        lv.insert(0, None);
                    }
                    set_index_levels(st, nid, Some(&lv));
                }
                None => set_index_levels(st, nid, None),
            }
        }
        if !axis1 {
            if let Some(names) = c.kw("names").and_then(|n| lit_key_seq(an, st, &n)) {
                let mut lv: Vec<Option<KeyId>> = names.into_iter().map(Some).collect();
                if lv.len() < 2 && keys.is_some() {
                    lv.push(None);
                }
                set_index_levels(st, nid, Some(&lv));
            }
        }
    }
    v
}

/// The op reads these columns of each frame (raising for a missing one): verdicts, and the
/// frames the program can go on with (those not surely lacking one; all when none is left).
fn require_cols(an: &mut Analyzer, st: &mut State, c: &Call, fs: &[u32], keys: &KeySet) -> Vec<u32> {
    let guarded = an.is_guarded(Ns::Item);
    let mut alive = Vec::new();
    let unsure = fs.iter().any(|f| keys.iter().any(|k| !matches!(column_tier(&st.row(*f, Ns::Item), k), Tier::Proven | Tier::Likely)));
    for f in fs {
        let dead = keys.iter().any(|k| column_tier(&st.row(*f, Ns::Item), k) == Tier::Missing);
        for k in intern::sorted_by_str(keys.iter()) {
            frame_getitem(an, *f, &Value::lit(k), &c.node, st, guarded);
        }
        if !dead {
            alive.push(*f);
        }
    }
    if !guarded {
        // (outside a try the analysis goes on past a failing access, as for dicts)
        return fs.to_vec();
    }
    if alive.is_empty() {
        if !fs.is_empty() && !keys.is_empty() {
            an.raise_here("KeyError", st);
        }
        fs.to_vec()
    } else {
        if unsure {
            an.may_raise_here("KeyError", st);
        }
        alive
    }
}

fn rows_of(st: &State, fs: &[u32]) -> Row {
    if fs.is_empty() {
        Row::top()
    } else {
        join_rows(fs.iter().map(|f| frame_row(st, *f)))
    }
}

/// May the row have k, does it surely?
fn side_status(r: &Row, k: KeyId) -> (bool, bool) {
    // (an open row's sources may rule a key out: a prefixed input has only prefixed keys)
    let may = r.present.contains(k) || (r.open() && !r.absent.contains(k) && classify(r, k).tier != Tier::Missing);
    (may, r.definite.contains(k))
}

/// The columns of two frames side by side (merge / join): keys once; a column both sides
/// have gets the suffixes, a column only one side has keeps its name.
fn combine_overlap(lrow: &Row, rrow: &Row, keys: &KeySet, lsuf: &str, rsuf: &str) -> Row {
    let mut flags: IndexMap<KeyId, (bool, bool)> = IndexMap::new();
    let mut gone: Vec<KeyId> = Vec::new();
    let mut put = |k: KeyId, may: bool, def: bool| {
        let e = flags.entry(k).or_insert((false, false));
        e.0 |= may;
        e.1 |= def;
    };
    let mut cols: Vec<KeyId> = lrow.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(lrow.present.iter()));
    for k in rrow.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(rrow.present.iter())) {
        if !cols.contains(&k) {
            cols.push(k);
        }
    }
    for c in cols {
        if keys.contains(c) {
            let def = lrow.definite.contains(c) || rrow.definite.contains(c);
            put(c, true, def);
            continue;
        }
        let (lm, ld) = side_status(lrow, c);
        let (rm, rd) = side_status(rrow, c);
        let cs = intern::key_py_str(c);
        let name = |suf: &str| if suf.is_empty() { c } else { key_str_lit(&format!("{}{}", cs, suf)) };
        // every case of which side has the column: the names it gives
        let opts = |m: bool, d: bool| -> Vec<bool> { if d { vec![true] } else if m { vec![true, false] } else { vec![false] } };
        let mut cases: Vec<Vec<KeyId>> = Vec::new();
        for a in opts(lm, ld) {
            for b in opts(rm, rd) {
                cases.push(match (a, b) {
                    (true, true) => vec![name(lsuf), name(rsuf)],
                    (true, false) | (false, true) => vec![c],
                    (false, false) => vec![],
                });
            }
        }
        let mut names: Vec<KeyId> = Vec::new();
        for cs0 in &cases {
            for n0 in cs0 {
                if !names.contains(n0) {
                    names.push(*n0);
                }
            }
        }
        for n0 in &names {
            let def = cases.iter().all(|cs0| cs0.contains(n0));
            put(*n0, true, def);
        }
        if !names.contains(&c) && !lsuf.is_empty() && !rsuf.is_empty() {
            gone.push(c);
        }
    }
    let mut row = Row::new();
    for (k, (may, def)) in &flags {
        if *may {
            row = row.store(*k, *def);
        }
    }
    // a column neither side may have is not there
    for k in lrow.absent.union(&rrow.absent).iter() {
        if !flags.get(&k).map(|f| f.0).unwrap_or(false) && !side_status(lrow, k).0 && !side_status(rrow, k).0 {
            gone.push(k);
        }
    }
    // a column both sides surely have is there only under its suffixed names
    for c in gone {
        if !flags.get(&c).map(|f| f.0).unwrap_or(false) {
            row = row.delete(c, true);
        }
    }
    if lrow.open() || rrow.open() {
        // a column the other side surely has comes from an open side only suffixed: that
        // side's input does not provide the plain name
        let kill_for = |other: &Row| -> KeySet { other.definite.difference(keys) };
        let mark = |r: &Row, other: &Row| {
            let k = kill_for(other);
            if k.is_empty() || crate::interp::disabled("mergekill") {
                r.sources.clone()
            } else {
                r.sources.map(|(s0, l)| (s0, crate::domain::lens_push(l, Step::Kill(k.clone()))))
            }
        };
        let mut d = row.data();
        d.sources = mark(lrow, rrow).union(&mark(rrow, lrow));
        d.mutators = lrow.mutators.union(&rrow.mutators);
        d.top = lrow.top || rrow.top;
        row = Row::from_data(d);
    }
    row
}

fn suffix_pair(an: &Analyzer, st: &State, v: Option<Value>) -> (String, String) {
    let mut suffixes = ("_x".to_string(), "_y".to_string());
    if let Some(sv) = v {
        for x in sv.pts.iter() {
            if matches!(an.kind(x), Kind::Tuple | Kind::List) && an.llen(x, st) == Some(2) {
                let a = st.slot(x, Ns::Pos, intern::key_int(0));
                let b = st.slot(x, Ns::Pos, intern::key_int(1));
                if let (Some(ka), Some(kb)) = (a.kv.as_lit(), b.kv.as_lit()) {
                    let sa = intern::with_key(ka, |k| matches!(k, intern::Key::None));
                    let sb = intern::with_key(kb, |k| matches!(k, intern::Key::None));
                    suffixes = (if sa { String::new() } else { intern::key_py_str(ka) }, if sb { String::new() } else { intern::key_py_str(kb) });
                }
            }
        }
    }
    suffixes
}

/// mode 0: `left.merge(right, …)`, 1: `pd.merge(left, right, …)`, 2: `pd.merge_asof`.
fn merge_impl(an: &mut Analyzer, st: &mut State, c: &Call, left: Value, right: Value, mode: usize) -> Value {
    let lf = frames(an, Some(&left));
    let rf = frames(an, Some(&right));
    let mut on = c.kw("on");
    let pos_on = if mode == 0 { 2 } else { 3 };
    if on.is_none() && mode != 2 && c.args.len() > pos_on {
        on = Some(c.args[pos_on].clone());
    }
    let cross = key_is(&c.kw("how"), "cross");
    let left_on = c.kw("left_on");
    let right_on = c.kw("right_on");
    let mut on_keys = on.as_ref().and_then(|o| an.lit_keys(o, st));
    if mode == 2 {
        if let Some(by) = c.kw("by").and_then(|b| an.lit_keys(&b, st)) {
            on_keys = Some(on_keys.unwrap_or_default().union(&by));
        }
    }
    let lkeys = left_on.as_ref().and_then(|o| an.lit_keys(o, st));
    let rkeys = right_on.as_ref().and_then(|o| an.lit_keys(o, st));
    let (mut lf, mut rf) = (lf, rf);
    if let Some(ok) = &on_keys {
        lf = require_cols(an, st, c, &lf, ok);
        rf = require_cols(an, st, c, &rf, ok);
    }
    if let Some(lk) = &lkeys {
        lf = require_cols(an, st, c, &lf, lk);
    }
    if let Some(rk) = &rkeys {
        rf = require_cols(an, st, c, &rf, rk);
    }
    let lrow = rows_of(st, &lf);
    let rrow = rows_of(st, &rf);
    let (lsuf, rsuf) = suffix_pair(an, st, c.kw("suffixes"));
    let by_index = key_is_bool(&c.kw("left_index"), true) || key_is_bool(&c.kw("right_index"), true);
    let mut keys: KeySet = on_keys.clone().unwrap_or_default();
    if let (Some(lk), Some(rk)) = (&lkeys, &rkeys) {
        // a left_on / right_on pair with the same name is one column
        keys = keys.union(&lk.intersection(rk));
    }
    if on_keys.is_none() && lkeys.is_none() && rkeys.is_none() && !cross && !by_index {
        // the common columns are the keys
        keys = lrow.present.intersection(&rrow.present);
        if lrow.open() || rrow.open() {
            an.note(format!("merge@L{}: join keys unknown (no on=)", c.line));
        }
    }
    let mut row = combine_overlap(&lrow, &rrow, &keys, &lsuf, &rsuf);
    let fresh = !by_index;
    match c.kw("indicator") {
        Some(v) if key_is_bool(&Some(v.clone()), true) => row = row.store(mk("_merge"), true),
        Some(v) => {
            if let Some(k) = lit_key(&Some(v)).filter(|k| intern::key_is_str(*k)) {
                row = row.store(k, true);
            }
        }
        None => {}
    }
    let v = new_frame(an, st, c, row.clone(), None, None);
    if fresh {
        fresh_index(st, &v);
    }
    // a column that keeps its name keeps its dtype
    if let Some(nid) = v.single() {
        for (side, other) in [(&lf, &rrow), (&rf, &lrow)] {
            if let [f] = side.as_slice() {
                let src = st.row(*f, Ns::Item);
                // (a plain name when the other side may have it too exists only where the other
                // lacks it: this side's, if this side surely has it)
                let keep = KeySet::from_iter(row.present.iter().filter(|k| src.present.contains(*k) && (keys.contains(*k) || !side_status(other, *k).0 || src.definite.contains(*k))));
                copy_dtypes(st, *f, nid, &keep);
                // values: some of this side's (a row without a match gets NaN)
                for k in keep.iter() {
                    if let Some(v) = vals(st, *f, k) {
                        let v = Known { may: v.may.insert(none_key()), sure: KeySet::new() };
                        set_vals(st, nid, k, Some(&v));
                    }
                }
            }
        }
    }
    v
}

fn pd_merge(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let left = c.args.first().cloned().or_else(|| c.kw("left")).unwrap_or_default();
    let right = c.args.get(1).cloned().or_else(|| c.kw("right")).unwrap_or_default();
    merge_impl(an, st, c, left, right, if c.qual.ends_with("merge_asof") { 2 } else { 1 })
}

fn df_merge(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let left = c.recv.clone().unwrap_or_default();
    let right = c.args.first().cloned().or_else(|| c.kw("right")).unwrap_or_default();
    merge_impl(an, st, c, left, right, 0)
}

fn df_join(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let other = c.args.first().cloned().or_else(|| c.kw("other")).unwrap_or_default();
    let lf = frames(an, c.recv.as_ref());
    let on = c.kw("on");
    let on_keys = on.as_ref().and_then(|o| an.lit_keys(o, st));
    let lf = match &on_keys {
        Some(ok) => require_cols(an, st, c, &lf, ok),
        None => lf,
    };
    // the other side: frames, named Series, or a list of them
    let mut rrows: Vec<Row> = Vec::new();
    let mut items: Vec<Value> = vec![other.clone()];
    if other.pts.iter().any(|o| matches!(an.kind(o), Kind::List | Kind::Tuple)) {
        let it = an.iterate(&other, &c.node, st);
        items = match (&it.items, &it.elem) {
            (Some(i), _) => i.clone(),
            (None, Elem::One(e)) => vec![e.clone()],
            _ => vec![],
        };
    }
    for item in &items {
        for o in item.pts.iter() {
            if an.kind(o) == Kind::Frame {
                rrows.push(st.row(o, Ns::Item));
            } else if is_series_obj(an, o) {
                let nm = series_name(an, st, o);
                rrows.push(match name_column(an, &nm) {
                    Some(k) => Row::closed_with(KeySet::one(k)),
                    None => Row::with_source(Source::new(&format!("join@L{}", c.line), SourceKind::Call)),
                });
            } else {
                rrows.push(Row::with_source(Source::new(&format!("join@L{}", c.line), SourceKind::Call)));
            }
        }
    }
    let lrow = rows_of(st, &lf);
    let rrow = if rrows.is_empty() { Row::top() } else { join_rows(rrows.into_iter()) };
    let lsuf = c.kw("lsuffix").and_then(|v| v.kv.as_lit()).map(intern::key_py_str).unwrap_or_default();
    let rsuf = c.kw("rsuffix").and_then(|v| v.kv.as_lit()).map(intern::key_py_str).unwrap_or_default();
    if lsuf.is_empty() && rsuf.is_empty() {
        // overlapping columns without suffixes: ValueError
        for col in intern::sorted_by_str(lrow.present.intersection(&rrow.present).iter()) {
            if lrow.definite.contains(col) && rrow.definite.contains(col) {
                let target = lf.first().copied().unwrap_or(0);
                an.verdict(&c.node, col, &Classification::with_note(Tier::Missing, "join: overlapping column without suffix raises ValueError"), target, Ns::Item, None, None);
            }
        }
    }
    let row = combine_overlap(&lrow, &rrow, &KeySet::new(), &lsuf, &rsuf);
    let v = derived(an, st, c, row.clone(), &lf, Rows::Other, false);
    // the left frame's columns that keep their names keep their values and dtypes
    if let (Some(nid), [f]) = (v.single(), lf.as_slice()) {
        let keep = KeySet::from_iter(row.present.iter().filter(|k| lrow.present.contains(*k) && !side_status(&rrow, *k).0));
        copy_dtypes(st, *f, nid, &keep);
        for k in keep.iter() {
            if let Some(vv) = vals(st, *f, k) {
                set_vals(st, nid, k, Some(&vv.subset()));
            }
        }
    }
    // the result keeps the left frame's index (its rows, for how="left" / "inner")
    if let Some(nid) = v.single() {
        let lv = levels_of(st, &lf);
        set_index_levels(st, nid, lv.as_deref());
        let how = lit_key(&c.kw("how")).and_then(intern::key_as_str).map(|s0| s0.to_string()).unwrap_or_else(|| "left".into());
        let lab = match (lf.as_slice(), how.as_str()) {
            ([f], "left") => labels(st, *f),
            ([f], "inner") => labels(st, *f).map(|k| k.subset()),
            _ => None,
        };
        set_labels(st, nid, lab.as_ref());
    }
    v
}

fn rename_step(an: &mut Analyzer, st: &mut State, cols: &Value, c: &Call, fs: &[u32]) -> Option<Vec<Step>> {
    if let Some(m) = an.literal_mapping(cols, st) {
        let mut pairs = m;
        pairs.sort_by_key(|(k, _)| intern::key_py_str(*k));
        return Some(vec![Step::RenameMap(std::sync::Arc::new(pairs))]);
    }
    for x in cols.pts.iter() {
        match an.kind(x) {
            Kind::External => {
                let q = an.external_qual(x);
                if let Some(f) = FnKind::from_name(last(&q)) {
                    if matches!(f, FnKind::Upper | FnKind::Lower | FnKind::Strip | FnKind::Title) {
                        return Some(vec![Step::RenameFn(RenameFn::new(f))]);
                    }
                }
            }
            Kind::Function | Kind::Bound => {
                let probe = Value::kv_only(KeyVal::KeysOf(KeysOf::new(ObjSet::from_vec(fs.to_vec()), Ns::Item)));
                let res = an.call_value(&Value::obj(x), &c.node, None, st, Some(vec![probe]));
                if let KeyVal::KeysOf(ko) = &res.kv {
                    if ko.objs == ObjSet::from_vec(fs.to_vec()) && ko.exclude.is_empty() {
                        return Some(intern::lens_get(ko.chain).steps);
                    }
                }
                // the function on each known column: a literal mapping (columns of an open frame
                // beyond these stay unknown)
                let row = rows_of(st, fs);
                if !row.present.is_empty() && !fs.is_empty() {
                    let mut pairs: Vec<(KeyId, KeyId)> = Vec::new();
                    for k in intern::sorted_by_str(row.present.iter()) {
                        let r = an.call_value(&Value::obj(x), &c.node, None, st, Some(vec![Value::lit(k)]));
                        match r.kv.as_lit().filter(|_| r.pts.is_empty()) {
                            Some(nk) => {
                                if nk != k {
                                    pairs.push((k, nk));
                                }
                            }
                            None => return None,
                        }
                    }
                    return Some(vec![Step::RenameMap(std::sync::Arc::new(pairs))]);
                }
            }
            _ => {}
        }
    }
    None
}

fn df_rename(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let mut cols = c.kw("columns");
    let mapper = c.kw("mapper").or_else(|| c.args.first().cloned());
    let axis = c.kw("axis");
    let fs = frames(an, c.recv.as_ref());
    if cols.is_none() {
        if let Some(m) = mapper {
            if axis_is_columns(&axis) {
                cols = Some(m);
            } else {
                // the index labels are renamed
                let row = recv_row(an, st, c);
                let v = derived(an, st, c, row, &fs, Rows::Same, true);
                if let Some(nid) = v.single() {
                    set_labels(st, nid, None);
                }
                return v;
            }
        }
    }
    let row = recv_row(an, st, c);
    let Some(cols) = cols else { return derived(an, st, c, row, &fs, Rows::Same, true) };
    let pairs = an.literal_mapping(&cols, st);
    // errors="raise": every mapped column must be there
    let fs = match (&pairs, key_is(&c.kw("errors"), "raise")) {
        (Some(p), true) => require_cols(an, st, c, &fs, &KeySet::from_iter(p.iter().map(|(k, _)| *k))),
        _ => fs,
    };
    let row = rows_of(st, &fs);
    let steps = rename_step(an, st, &cols, c, &fs);
    let Some(steps) = steps else {
        an.note(format!("rename@L{}: unknown column mapping", c.line));
        let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.insert((Source::new(&format!("rename@L{}", c.line), SourceKind::Call), EMPTY_LENS)), mutators: row.mutators.clone(), ..Default::default() });
        return new_frame(an, st, c, r, None, None);
    };
    let mut out = row.clone();
    for s0 in &steps {
        out = out.rename(s0);
    }
    if let Some(p) = &pairs {
        // a renamed-away name is gone (unless another column was renamed to it)
        let targets = KeySet::from_iter(p.iter().map(|(_, v)| *v));
        for (k, _) in p {
            if !targets.contains(*k) {
                let mut d = out.data();
                d.present = d.present.remove(*k);
                d.definite = d.definite.remove(*k);
                d.likely = d.likely.remove(*k);
                d.absent = d.absent.insert(*k);
                out = Row::from_data(d);
            }
        }
    }
    let v = derived(an, st, c, out, &fs, Rows::Same, true);
    if let (Some(nid), [f], Some(p)) = (v.single(), fs.as_slice(), &pairs) {
        // values and dtypes follow the renamed columns
        for (from, to) in p {
            if let Some(vv) = vals(st, *f, *from) {
                set_vals(st, nid, *to, Some(&vv));
            }
            if let Some(dt) = st.slot_opt(*f, Ns::Meta, dtype_key(*from)) {
                st.set_slot(nid, Ns::Meta, dtype_key(*to), dt, true);
            }
        }
    }
    v
}

fn add_prefix(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fnk = if c.qual.ends_with("prefix") { FnKind::Prefix } else { FnKind::Suffix };
    let arg = c.arg(0, Some(fnk.name()));
    let row = recv_row(an, st, c);
    if axis_is_index(&c.kw("axis")) {
        // the index labels are renamed, the columns stay
        let fs = frames(an, c.recv.as_ref());
        let v = derived(an, st, c, row, &fs, Rows::Same, true);
        if let Some(nid) = v.single() {
            set_labels(st, nid, None);
        }
        return v;
    }
    if let Some(k) = lit_key(&arg) {
        let r = row.rename(&Step::RenameFn(RenameFn::with_arg(fnk, k)));
        let fs = frames(an, c.recv.as_ref());
        return derived(an, st, c, r, &fs, Rows::Same, false);
    }
    let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.insert((Source::new(&format!("add_{}@L{}", fnk.name(), c.line), SourceKind::Call), EMPTY_LENS)), mutators: row.mutators.clone(), ..Default::default() });
    new_frame(an, st, c, r, None, None)
}

fn df_drop(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let cols = c.kw("columns");
    let axis = c.kw("axis");
    let labels = c.arg(0, Some("labels"));
    let is_cols = cols.is_some() || axis_is_columns(&axis);
    let ignore = key_is(&c.kw("errors"), "ignore");
    let row = recv_row(an, st, c);
    if !is_cols {
        // rows dropped: the same columns
        return derived(an, st, c, row, &fs, Rows::Subset, true);
    }
    let target = cols.or(labels);
    let ks = target.as_ref().and_then(|t| an.lit_keys(t, st));
    let Some(ks) = ks else {
        let kv = target.as_ref().map(|t| t.kv.clone()).unwrap_or(DATA);
        if let KeyVal::KeysOf(_) = kv {
            let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.clone(), mutators: row.mutators.clone(), ..Default::default() });
            return new_frame(an, st, c, r, None, None);
        }
        an.note(format!("drop@L{}: unknown columns", c.line));
        let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.clone(), mutators: row.mutators.clone(), ..Default::default() });
        return new_frame(an, st, c, r, None, None);
    };
    let fs = if ignore { fs } else { require_cols(an, st, c, &fs, &ks) };
    let mut row = rows_of(st, &fs);
    for k in ks.iter() {
        row = row.delete(k, true);
    }
    derived(an, st, c, row, &fs, Rows::Same, true)
}

fn df_assign(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let mut row = recv_row(an, st, c);
    let mut colvals: Vec<(KeyId, Option<Known>)> = Vec::new();
    for (k, v) in c.kwargs.iter() {
        let col = intern::key(intern::Key::Str(intern::str_get(*k)));
        // a callable gets the frame as built so far (earlier keywords included)
        if v.pts.iter().any(|o| matches!(an.kind(o), Kind::Function | Kind::Bound | Kind::Partial | Kind::Class)) {
            let tmp = an.alloc(Kind::Frame, &format!("{}:assign:{}", crate::interp::site_string(&c.node), intern::key_py_str(col)), st, &format!("frame@L{}", c.line), Payload::None, Some(row.clone()), Ns::Item, None, None);
            an.call_value(v, &c.node, None, st, Some(vec![Value::obj(tmp)]));
            colvals.push((col, None));
        } else {
            let lit = lit_key_seq(an, st, v).map(|s0| Known::exact(KeySet::from_iter(s0))).or_else(|| cell_lit(v).map(|l| Known::exact(KeySet::one(l))));
            colvals.push((col, lit));
        }
        row = row.store(col, true);
    }
    // `assign(**{"a b": …})`: the dict's keys
    for sk in &c.star_kwargs {
        for d in sk.pts.iter().filter(|o| an.kind(*o) == Kind::Dict) {
            let r = st.row(d, Ns::Item);
            for k in r.present.iter() {
                row = row.store(k, r.definite.contains(k));
                colvals.push((k, None));
            }
            if r.open() {
                let mut dd = row.data();
                dd.sources = dd.sources.insert((Source::new(&format!("assign@L{}", c.line), SourceKind::Call), EMPTY_LENS));
                row = Row::from_data(dd);
            }
        }
    }
    let v = derived(an, st, c, row, &fs, Rows::Same, true);
    if let Some(nid) = v.single() {
        for (k, kn) in colvals {
            set_vals(st, nid, k, kn.as_ref());
        }
    }
    v
}

fn reset_index(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let row = recv_row(an, st, c);
    let drop = key_is_bool(&c.kw("drop"), true);
    let Some(levels) = levels_of(st, &fs) else {
        // levels unknown: the frame gets some columns back
        let mut d = row.data();
        if !drop {
            d.sources = d.sources.insert((Source::new(&format!("reset_index@L{}", c.line), SourceKind::Call), EMPTY_LENS));
        }
        let r = Row::from_data(d);
        return derived(an, st, c, r, &fs, Rows::Other, false);
    };
    // the levels moved: level= (positions / names), else all
    let mut chosen: Vec<usize> = (0..levels.len()).collect();
    if let Some(lv) = c.kw("level").or_else(|| c.arg(0, None)) {
        let sel: Option<Vec<KeyId>> = lit_key_seq(an, st, &lv).or_else(|| lv.kv.as_lit().filter(|_| lv.pts.is_empty()).map(|k| vec![k]));
        match sel {
            Some(sel) => {
                chosen = Vec::new();
                for k in sel {
                    if let Some(i) = intern::key_as_int(k).filter(|_| !intern::key_is_str(k)) {
                        let i = if i < 0 { i + levels.len() as i64 } else { i };
                        if (0..levels.len() as i64).contains(&i) {
                            chosen.push(i as usize);
                        }
                    } else if let Some(i) = levels.iter().position(|l| *l == Some(k)) {
                        chosen.push(i);
                    }
                }
                chosen.sort();
                chosen.dedup();
            }
            None => {
                let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.insert((Source::new(&format!("reset_index@L{}", c.line), SourceKind::Call), EMPTY_LENS)), mutators: row.mutators.clone(), ..Default::default() });
                return new_frame(an, st, c, r, None, None);
            }
        }
    }
    let names: Option<Vec<KeyId>> = c.kw("names").and_then(|v| lit_key_seq(an, st, &v).or_else(|| v.kv.as_lit().filter(|_| v.pts.is_empty()).map(|k| vec![k])));
    let mut new_cols: Vec<KeyId> = Vec::new();
    if !drop {
        for (j, i) in chosen.iter().enumerate() {
            let k = names.as_ref().and_then(|n| n.get(j).copied()).unwrap_or_else(|| level_column(&levels, *i));
            new_cols.push(k);
        }
    }
    // the new columns come first
    let mut out = row.clone();
    for k in &new_cols {
        out = out.store(*k, true);
    }
    let mut d = out.data();
    if let Some(o) = row.ordered_keys() {
        let mut ord = new_cols.clone();
        ord.extend(o.into_iter().filter(|k| !new_cols.contains(k)));
        d.order = ord.into();
    }
    let r = Row::from_data(d);
    let rest: Vec<Option<KeyId>> = levels.iter().enumerate().filter(|(i, _)| !chosen.contains(i)).map(|(_, l)| *l).collect();
    let v = derived(an, st, c, r.clone(), &fs, Rows::Same, true);
    if let Some(nid) = v.single() {
        if rest.is_empty() {
            set_index_levels(st, nid, Some(&[None]));
            let n = fs.first().filter(|_| fs.len() == 1).and_then(|f| nrows(st, *f));
            set_labels(st, nid, n.map(range_labels).as_ref());
            // a single level's labels become that column's values
            if let ([f], [k], 1) = (fs.as_slice(), new_cols.as_slice(), levels.len()) {
                let lab = labels(st, *f);
                set_vals(st, nid, *k, lab.as_ref());
            }
        } else {
            set_index_levels(st, nid, Some(&rest));
            set_labels(st, nid, None);
            if let [f] = fs.as_slice() {
                let kept: Vec<usize> = (0..levels.len()).filter(|i| !chosen.contains(i)).collect();
                for (j, i) in kept.iter().enumerate() {
                    let v = lvals(st, *f, *i);
                    set_lvals(st, nid, j, v.as_ref());
                }
                if kept.len() == 1 {
                    let v = lvals(st, *f, kept[0]);
                    set_labels(st, nid, v.as_ref());
                }
            }
        }
        if let [f] = fs.as_slice() {
            if levels.len() > 1 {
                for (j, i) in chosen.iter().enumerate() {
                    if let Some(k) = new_cols.get(j) {
                        let v = lvals(st, *f, *i);
                        set_vals(st, nid, *k, v.as_ref());
                    }
                }
            }
        }
    }
    v
}

/// The names a pandas expression (`query` / `eval`) assigns and reads: identifiers and
/// `backtick quoted` names, not `@locals`, string literals, keywords, attributes after `.`
/// or called functions. None when it cannot be read.
fn expr_names(e: &str) -> Option<(Vec<String>, Vec<String>)> {
    let mut assigned: Vec<String> = Vec::new();
    let mut read: Vec<String> = Vec::new();
    for stmt in e.split(|c| c == '\n' || c == ';') {
        let cs: Vec<char> = stmt.chars().collect();
        // `target = expr` (not ==, <=, >=, !=)
        let mut body_start = 0;
        let mut i = 0;
        while i < cs.len() {
            if cs[i] == '=' && (i + 1 >= cs.len() || cs[i + 1] != '=') && (i == 0 || !matches!(cs[i - 1], '=' | '<' | '>' | '!')) {
                let lhs: String = cs[..i].iter().collect();
                let t = lhs.trim().trim_matches('`').to_string();
                if t.is_empty() {
                    return None;
                }
                assigned.push(t);
                body_start = i + 1;
                break;
            }
            if cs[i] == '\'' || cs[i] == '"' {
                break;
            }
            i += 1;
        }
        let mut i = body_start;
        let mut prev_dot = false;
        let mut prev_at = false;
        while i < cs.len() {
            let c = cs[i];
            if c == '\'' || c == '"' {
                let q = c;
                i += 1;
                while i < cs.len() && cs[i] != q {
                    i += 1;
                }
                i += 1;
                prev_dot = false;
                continue;
            }
            if c == '`' {
                let st0 = i + 1;
                i += 1;
                while i < cs.len() && cs[i] != '`' {
                    i += 1;
                }
                let name: String = cs[st0..i.min(cs.len())].iter().collect();
                if !prev_dot && !assigned.contains(&name) {
                    read.push(name);
                }
                i += 1;
                prev_dot = false;
                continue;
            }
            if c.is_alphabetic() || c == '_' {
                let st0 = i;
                while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                    i += 1;
                }
                let name: String = cs[st0..i].iter().collect();
                let mut j = i;
                while j < cs.len() && cs[j] == ' ' {
                    j += 1;
                }
                let called = j < cs.len() && cs[j] == '(';
                let kw = matches!(name.as_str(), "and" | "or" | "not" | "in" | "is" | "True" | "False" | "None" | "if" | "else");
                if !prev_dot && !prev_at && !called && !kw && !assigned.contains(&name) && !read.contains(&name) {
                    read.push(name);
                }
                prev_dot = false;
                prev_at = false;
                continue;
            }
            if c.is_ascii_digit() {
                while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '.' || cs[i] == '_') {
                    i += 1;
                }
                continue;
            }
            prev_dot = c == '.';
            prev_at = c == '@';
            i += 1;
        }
    }
    Some((assigned, read))
}

fn expr_text(an: &Analyzer, c: &Call) -> Option<String> {
    let _ = an;
    lit_key(&c.arg(0, Some("expr"))).and_then(intern::key_as_str).map(|s| s.to_string())
}

/// The frames that remain after reading the expression's names (index level names aside).
fn read_expr_names(an: &mut Analyzer, st: &mut State, c: &Call, fs: &[u32], read: &[String]) -> Vec<u32> {
    let idx: Vec<KeyId> = levels_of(st, fs).map(|l| l.into_iter().flatten().collect()).unwrap_or_default();
    let ks = KeySet::from_iter(read.iter().map(|n| mk(n)).filter(|k| !idx.contains(k) && *k != mk("index")));
    require_cols(an, st, c, fs, &ks)
}

fn df_query(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let fs = match expr_text(an, c).as_deref().and_then(expr_names) {
        Some((_, read)) => read_expr_names(an, st, c, &fs, &read),
        None => fs,
    };
    let row = rows_of(st, &fs);
    derived(an, st, c, row, &fs, Rows::Subset, true)
}

fn df_eval(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let Some((assigned, read)) = expr_text(an, c).as_deref().and_then(expr_names) else {
        let row = recv_row(an, st, c);
        let r = Row::from_data(RowData { present: row.present.clone(), likely: row.definite.clone(), sources: row.sources.insert((Source::new(&format!("eval@L{}", c.line), SourceKind::Call), EMPTY_LENS)), mutators: row.mutators.clone(), ..Default::default() });
        return new_frame(an, st, c, r, None, None);
    };
    let fs = read_expr_names(an, st, c, &fs, &read);
    if assigned.is_empty() {
        return series(an);
    }
    let mut row = rows_of(st, &fs);
    for a in &assigned {
        row = row.store(mk(a), true);
    }
    derived(an, st, c, row, &fs, Rows::Same, true)
}

fn rename_axis(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let row = recv_row(an, st, c);
    let v = derived(an, st, c, row, &fs, Rows::Same, true);
    let Some(nid) = v.single() else { return v };
    let to_columns = axis_is_columns(&c.kw("axis"));
    let idx = if to_columns { None } else { c.arg(0, Some("mapper")).or_else(|| c.kw("index")) };
    if let Some(m) = idx {
        let names: Option<Vec<Option<KeyId>>> = match lit_key_seq(an, st, &m) {
            Some(seq) => Some(seq.into_iter().map(|k| if k == none_key() { None } else { Some(k) }).collect()),
            None => m.kv.as_lit().filter(|_| m.pts.is_empty()).map(|k| vec![if k == none_key() { None } else { Some(k) }]),
        };
        set_index_levels(st, nid, names.as_deref());
    }
    let cols = if to_columns { c.arg(0, Some("mapper")) } else { c.kw("columns") };
    if let Some(cn) = cols {
        st.set_slot(nid, Ns::Meta, mk("colname"), cn, true);
    }
    v
}

/// `df.T`: the row labels become the columns, the columns the row labels.
pub fn transpose_of(an: &mut Analyzer, st: &mut State, node: &Node, fs: &[u32]) -> Value {
    let row = rows_of(st, fs);
    let lab = if fs.len() == 1 { labels(st, fs[0]) } else { None };
    let newcols = match &lab {
        Some(k) => k.row(),
        None => Row::with_source(Source::new(&format!("T@L{}", node.line), SourceKind::Call)),
    };
    let nid = an.alloc_at(Kind::Frame, node, st, &format!("frame@L{}", node.line), Payload::None, Some(newcols), Ns::Item, None, None);
    let cn = if fs.len() == 1 { colname(st, fs[0]) } else { None };
    set_index_levels(st, nid, Some(&[cn]));
    set_nrows(st, nid, None);
    let newlab = if row.closed() { Some(Known { may: row.present.clone(), sure: row.definite.clone() }) } else { None };
    set_labels(st, nid, newlab.as_ref());
    if let Some(n) = row.closed().then(|| row.present.len() as i64).filter(|_| row.present == row.definite) {
        set_nrows(st, nid, Some(n));
    }
    Value::obj(nid)
}

fn transpose(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    transpose_of(an, st, &c.node, &fs)
}

fn set_index(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let keys = c.arg(0, Some("keys"));
    let seq: Option<Vec<KeyId>> = keys.as_ref().and_then(|k| lit_key_seq(an, st, k).or_else(|| k.kv.as_lit().filter(|_| k.pts.is_empty()).map(|x| vec![x])));
    let row = recv_row(an, st, c);
    let keep = key_is_bool(&c.kw("drop"), false);
    let append = key_is_bool(&c.kw("append"), true);
    let Some(seq) = seq else {
        // a Series / array as the index: the columns stay
        if keys.as_ref().map(|k| is_seriesish(an, k)).unwrap_or(false) {
            let v = derived(an, st, c, row, &fs, Rows::Same, true);
            if let Some(nid) = v.single() {
                let name = keys.as_ref().and_then(|k| k.pts.iter().find(|o| is_series_obj(an, *o))).map(|o| series_name(an, st, o)).and_then(|n| n.kv.as_lit().filter(|_| n.pts.is_empty())).filter(|k| *k != none_key());
                set_index_levels(st, nid, Some(&[name]));
                set_labels(st, nid, None);
            }
            return v;
        }
        let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.clone(), mutators: row.mutators.clone(), ..Default::default() });
        return new_frame(an, st, c, r, None, None);
    };
    let ks = KeySet::from_iter(seq.iter().copied());
    let fs = require_cols(an, st, c, &fs, &ks);
    let mut row = rows_of(st, &fs);
    if !keep {
        for k in ks.iter() {
            row = row.delete(k, true);
        }
    }
    let v = derived(an, st, c, row, &fs, Rows::Same, true);
    if let Some(nid) = v.single() {
        let mut levels: Vec<Option<KeyId>> = if append { levels_of(st, &fs).unwrap_or_default() } else { Vec::new() };
        levels.extend(seq.iter().map(|k| Some(*k)));
        set_index_levels(st, nid, Some(&levels));
        let lab = match (fs.as_slice(), seq.as_slice(), append) {
            ([f], [k], false) => vals(st, *f, *k),
            _ => None,
        };
        set_labels(st, nid, lab.as_ref());
        let base = levels.len() - seq.len();
        if let [f] = fs.as_slice() {
            for i in 0..base {
                let v = lvals(st, *f, i);
                set_lvals(st, nid, i, v.as_ref());
            }
            for (j, k) in seq.iter().enumerate() {
                let v = vals(st, *f, *k);
                set_lvals(st, nid, base + j, v.as_ref());
            }
        }
    }
    v
}

fn groupby(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let by = c.arg(0, Some("by"));
    let ks = by.as_ref().and_then(|b| an.lit_keys(b, st));
    let fs = match &ks {
        Some(k) => require_cols(an, st, c, &fs, k),
        None => fs,
    };
    let ai = !key_is_bool(&c.kw("as_index"), false);
    let seq = by.as_ref().and_then(|b| lit_key_seq(an, st, b).or_else(|| b.kv.as_lit().filter(|_| b.pts.is_empty()).map(|k| vec![k])));
    let payload = Payload::Groupby(fs.clone(), ks, ai, None);
    let gid = an.alloc_at(Kind::Groupby, &c.node, st, &format!("groupby@L{}", c.line), payload.clone(), None, Ns::Item, None, None);
    an.obj_mut(gid).payload = payload;
    if let Some(seq) = seq {
        st.set_slot(gid, Ns::Meta, mk("by"), Value::lit(intern::key_tuple(seq)), true);
    }
    Value::obj(gid)
}

/// Meta slots of a groupby / window object that carry over to the objects made from it.
const GB_META: &[&str] = &["by", "window", "on", "dropna", "selseq", "grouped"];

fn copy_gb_meta(st: &mut State, src: u32, dst: u32) {
    for k in GB_META {
        if let Some(v) = st.slot_opt(src, Ns::Meta, mk(k)) {
            st.set_slot(dst, Ns::Meta, mk(k), v, true);
        }
    }
}

fn gb_tuple(st: &State, g: u32, k: &str) -> Option<Vec<KeyId>> {
    st.slot_opt(g, Ns::Meta, mk(k)).and_then(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())).and_then(|k| intern::with_key(k, |kk| if let intern::Key::Tuple(t) = kk { Some(t.clone()) } else { None }))
}

fn gb_str(st: &State, g: u32, k: &str) -> Option<String> {
    st.slot_opt(g, Ns::Meta, mk(k)).and_then(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())).and_then(intern::key_as_str).map(|s| s.to_string())
}

pub fn groupby_select(an: &mut Analyzer, gid: u32, key: &Value, node: &Node, state: &mut State) -> Value {
    let Payload::Groupby(fs, ks, ai, _sel) = an.obj(gid).payload.clone() else { return Value::new() };
    let seq = lit_key_seq(an, state, key);
    let cols = if !key.pts.is_empty() { an.lit_keys(key, state) } else { Some(kv_literals(&key.kv)) };
    let cols = cols.filter(|c| !c.is_empty());
    let mut fs = fs;
    if let Some(cs) = &cols {
        let call = Call { qual: "groupby.select".into(), recv: None, args: vec![], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: *node, call_ast: None, site: format!("L{}", node.line), line: node.line };
        fs = require_cols(an, state, &call, &fs, cs);
    }
    let sel = cols.map(|cs| (cs, matches!(key.kv, KeyVal::Lit(_)) && key.pts.is_empty()));
    let payload = Payload::Groupby(fs, ks, ai, sel);
    let nid = an.alloc_at(Kind::Groupby, node, state, &format!("groupby@L{}", node.line), payload.clone(), None, Ns::Item, None, None);
    an.obj_mut(nid).payload = payload;
    copy_gb_meta(state, gid, nid);
    if let Some(seq) = seq {
        state.set_slot(nid, Ns::Meta, mk("selseq"), Value::lit(intern::key_tuple(seq)), true);
    }
    Value::obj(nid)
}

/// A rolling / expanding / ewm / resample window over a frame or over groups: a groupby
/// object marked `window` (its aggregations keep the rows' columns).
fn window_of(an: &mut Analyzer, st: &mut State, c: &Call, fs: Vec<u32>, keys: Option<KeySet>, src_gb: Option<u32>, kind: &str) -> Value {
    let (ai, sel) = match src_gb.map(|g| an.obj(g).payload.clone()) {
        Some(Payload::Groupby(_, _, ai, sel)) => (ai, sel),
        _ => (true, None),
    };
    let payload = Payload::Groupby(fs.clone(), keys, ai, sel);
    let gid = an.alloc_at(Kind::Groupby, &c.node, st, &format!("{}@L{}", kind, c.line), payload.clone(), None, Ns::Item, None, None);
    an.obj_mut(gid).payload = payload;
    if let Some(g) = src_gb {
        copy_gb_meta(st, g, gid);
        st.set_slot(gid, Ns::Meta, mk("grouped"), Value::lit(intern::key_bool(true)), true);
    }
    st.set_slot(gid, Ns::Meta, mk("window"), Value::lit(mk(kind)), true);
    if let Some(on) = lit_key(&c.kw("on")) {
        let _ = require_cols(an, st, c, &fs, &KeySet::one(on));
        st.set_slot(gid, Ns::Meta, mk("on"), Value::lit(on), true);
    }
    Value::obj(gid)
}

/// The (column, function name) pairs of an aggregation spec value: a string or function
/// gives its name, a list each of its items' names; None when not known.
fn agg_names(an: &mut Analyzer, st: &State, v: &Value) -> Option<(Vec<KeyId>, bool)> {
    if let (Some(k), true) = (v.kv.as_lit(), v.pts.is_empty()) {
        return Some((vec![k], false));
    }
    if let Some(seq) = lit_key_seq(an, st, v) {
        return Some((seq, true));
    }
    let o = v.single()?;
    // a function's __name__ (lambdas: "<lambda>"; numbered "<lambda_i>" in a list)
    let is_lambda = |an: &Analyzer, o: u32| an.kind(o) == Kind::Function && matches!(an.func_info(o).node, crate::interp::FuncNode::Lambda(_));
    let fname = |an: &Analyzer, o: u32| -> Option<KeyId> {
        match an.kind(o) {
            Kind::Function if is_lambda(an, o) => Some(mk("<lambda>")),
            Kind::Function => {
                let q = an.func_info(o).qualname.clone();
                Some(mk(q.rsplit('.').next().unwrap_or(&q)))
            }
            Kind::External => Some(mk(last(&an.external_qual(o)))),
            _ => None,
        }
    };
    if matches!(an.kind(o), Kind::List | Kind::Tuple) {
        let n = an.llen(o, st)?;
        let mut out = Vec::new();
        let mut lambdas = 0;
        for i in 0..n {
            let x = st.slot(o, Ns::Pos, intern::key_int(i as i64));
            match (x.kv.as_lit(), x.single()) {
                (Some(k), _) if x.pts.is_empty() => out.push(k),
                (_, Some(f)) if is_lambda(an, f) => {
                    out.push(mk(&format!("<lambda_{}>", lambdas)));
                    lambdas += 1;
                }
                (_, Some(f)) => out.push(fname(an, f)?),
                _ => return None,
            }
        }
        return Some((out, true));
    }
    fname(an, o).map(|k| (vec![k], false))
}

fn groupby_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    symbolic_frames_event(an, st, &call_values(c));
    let mut name = last(&c.qual).to_string();
    let recv = c.recv.clone().unwrap_or_default();
    let gids: Vec<u32> = recv.pts.iter().filter(|o| an.kind(*o) == Kind::Groupby).collect();
    if gids.is_empty() {
        return Value::data(&format!("L{}", c.line));
    }
    let g0 = gids[0];
    let Payload::Groupby(fs, ks, ai, sel) = an.obj(g0).payload.clone() else { return Value::new() };
    let window = gb_str(st, g0, "window");
    if let Some(src) = st.slot_opt(g0, Ns::Meta, mk("srcseries")) {
        // a window over a Series
        let ss: Vec<u32> = src.pts.iter().collect();
        let same = window.as_deref() != Some("resample");
        let idx = if same && !ss.is_empty() { join_rows(ss.iter().map(|x| st.row(*x, Ns::Item))) } else { Row::with_source(Source::new("index", SourceKind::Data)) };
        return match name.as_str() {
            "ohlc" => new_frame(an, st, c, closed_ordered(&[mk("open"), mk("high"), mk("low"), mk("close")]), None, None),
            "agg" | "aggregate" => match c.args.first().cloned().and_then(|a| agg_names(an, st, &a)) {
                Some((fnames, true)) => new_frame(an, st, c, closed_ordered(&fnames), None, None),
                _ => series_like(an, st, c, &ss, idx, "w"),
            },
            _ => {
                let sid = series_like_id(an, st, c, &ss, idx, "w");
                set_vals(st, sid, vkey(), None);
                Value::obj(sid)
            }
        };
    }
    let grouped = window.is_none() || st.slot_opt(g0, Ns::Meta, mk("grouped")).is_some();
    let base = rows_of(st, &fs);
    let keys = if grouped { ks.clone().unwrap_or_default() } else { KeySet::new() };
    let by_seq: Option<Vec<KeyId>> = if grouped { gb_tuple(st, g0, "by") } else { Some(Vec::new()) };
    let on = gb_str(st, g0, "on").map(|s| mk(&s));
    let levels: Option<Vec<Option<KeyId>>> = by_seq.as_ref().map(|s0| s0.iter().map(|k| Some(*k)).collect());
    // the group labels: the values of a single key column, when known
    let group_labels: Option<Known> = match (fs.as_slice(), by_seq.as_deref()) {
        ([f], Some([k])) if window.is_none() => vals(st, *f, *k).map(|v| {
            let nk = none_key();
            if key_is_bool(&c.kw("dropna"), false) { v } else { Known { may: v.may.remove(nk), sure: v.sure.remove(nk) } }
        }),
        _ => None,
    };
    let selseq = gb_tuple(st, g0, "selseq");
    // the value columns: the selection, else every column but the keys
    let value_row: Row = match &sel {
        Some((cols, _)) => match &selseq {
            Some(seq) => closed_ordered(seq),
            None => Row::closed_with(cols.clone()),
        },
        None => {
            let mut r = base.clone();
            for k in keys.iter() {
                r = r.delete(k, true);
            }
            r
        }
    };
    let single_sel: Option<KeyId> = match &sel {
        Some((cols, true)) => cols.first(),
        _ => None,
    };
    // a frame result: indexed by the keys (as_index) or with the keys as leading columns
    let frame_result = |an: &mut Analyzer, st: &mut State, row: Row, same_rows: bool| -> Value {
        let mut row = row;
        if !ai && !same_rows && window.is_none() {
            let mut lead = Row::new();
            for k in by_seq.clone().unwrap_or_else(|| intern::sorted_by_str(keys.iter())) {
                lead = lead.store(k, true);
            }
            let mut d = lead.copy_from(&row, true, &KeySet::new(), EMPTY_LENS).data();
            d.sources = row.sources.clone();
            d.top = row.top;
            row = Row::from_data(d);
        }
        let v = new_frame(an, st, c, row.clone(), None, None);
        let nid = v.single().unwrap();
        if same_rows || (window.is_some() && !grouped) {
            if let [f] = fs.as_slice() {
                derive(st, *f, nid, Rows::Same, &KeySet::new());
            }
        } else if ai && !keys.is_empty() {
            let mut lv: Vec<Option<KeyId>> = levels.clone().unwrap_or_default();
            if window.is_some() {
                // a grouped window keeps the original index under the keys
                lv.push(None);
                set_index_levels(st, nid, Some(&lv));
                set_labels(st, nid, None);
            } else {
                set_index_levels(st, nid, levels.as_deref());
                set_labels(st, nid, group_labels.as_ref());
            }
        } else if !ai {
            set_index_levels(st, nid, Some(&[None]));
            set_labels(st, nid, None);
        }
        v
    };
    // a Series result: one value per group (as_index) or a frame {keys…, name}
    let series_result = |an: &mut Analyzer, st: &mut State, nm: Option<KeyId>, same_rows: bool| -> Value {
        if !ai && !same_rows {
            let col = nm.unwrap_or_else(|| intern::key_int(0));
            return frame_result(an, st, Row::closed_with(KeySet::one(col)), false);
        }
        let idx = if same_rows {
            match fs.as_slice() {
                [f] => labels_row(st, *f),
                _ => Row::with_source(Source::new("index", SourceKind::Data)),
            }
        } else {
            match &group_labels {
                Some(k) => k.row(),
                None => Row::with_source(Source::new("groups", SourceKind::Data)),
            }
        };
        let name_v = match nm {
            Some(k) => Value::lit(k),
            None => an.none_v.clone(),
        };
        let sid = new_series(an, st, &c.node, idx, Some(name_v), "g");
        if same_rows {
            if let [f] = fs.as_slice() {
                if let Some(m) = st.slot_opt(*f, Ns::Meta, mk("index")) {
                    st.set_slot(sid, Ns::Meta, mk("index"), m, true);
                }
            }
        } else {
            set_index_levels(st, sid, levels.as_deref());
        }
        Value::obj(sid)
    };
    if name == "agg" || name == "aggregate" {
        let named = named_aggs(an, st, c);
        if !named.is_empty() || !c.star_kwargs.is_empty() {
            let sure_cols = KeySet::from_iter(named.iter().filter(|(_, d, _)| *d).filter_map(|(_, _, col)| *col));
            let _ = require_cols(an, st, c, &fs, &sure_cols);
            let mut row = Row::new();
            for (k, d, _) in &named {
                row = row.store(*k, *d);
            }
            if c.star_kwargs.iter().any(|sk| sk.pts.iter().any(|o| an.kind(o) != Kind::Dict || !st.row(o, Ns::Item).closed())) {
                let mut dd = row.data();
                dd.sources = dd.sources.insert((Source::new(&format!("agg@L{}", c.line), SourceKind::Call), EMPTY_LENS));
                row = Row::from_data(dd);
            }
            return frame_result(an, st, row, false);
        }
        if let Some(spec) = c.args.first().or_else(|| c.kwargs.get(&intern::s("func"))).cloned() {
            // {col: f | [f, …]}
            if spec.single().map(|o| an.kind(o) == Kind::Dict).unwrap_or(false) {
                let d = spec.single().unwrap();
                let r = st.row(d, Ns::Item);
                if r.closed() {
                    let cols: Vec<KeyId> = r.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(r.present.iter()));
                    let _ = require_cols(an, st, c, &fs, &r.definite);
                    let mut specs: Vec<(KeyId, Option<(Vec<KeyId>, bool)>)> = Vec::new();
                    for col in &cols {
                        let v = st.slot(d, Ns::Item, *col);
                        specs.push((*col, agg_names(an, st, &v)));
                    }
                    let multi = specs.iter().any(|(_, s0)| s0.as_ref().map(|x| x.1).unwrap_or(true));
                    let mut row = Row::new();
                    let mut open = false;
                    for (col, s0) in &specs {
                        let sure = r.definite.contains(*col);
                        if !multi {
                            row = row.store(*col, sure);
                            continue;
                        }
                        match s0 {
                            Some((fnames, _)) => {
                                for f in fnames {
                                    row = row.store(intern::key_tuple(vec![*col, *f]), sure);
                                }
                            }
                            None => open = true,
                        }
                    }
                    if open {
                        let mut dd = row.data();
                        dd.sources = dd.sources.insert((Source::new(&format!("agg@L{}", c.line), SourceKind::Call), EMPTY_LENS));
                        row = Row::from_data(dd);
                    }
                    return frame_result(an, st, row, false);
                }
                return frame_result(an, st, Row::with_source(Source::new(&format!("agg@L{}", c.line), SourceKind::Call)), false);
            }
            match agg_names(an, st, &spec) {
                Some((fnames, true)) => {
                    // a list: one column per function (a Series selection), else (col, f) pairs
                    let mut row = Row::new();
                    if single_sel.is_some() {
                        for f in &fnames {
                            row = row.store(*f, true);
                        }
                    } else {
                        let cols = value_row.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(value_row.present.iter()));
                        for col in &cols {
                            for f in &fnames {
                                let t = intern::key_tuple(vec![*col, *f]);
                                row = row.store(t, value_row.definite.contains(*col));
                            }
                        }
                        if value_row.open() {
                            let mut dd = row.data();
                            dd.sources = value_row.sources.clone();
                            row = Row::from_data(dd);
                        }
                    }
                    return frame_result(an, st, row, false);
                }
                Some((_, false)) | None => {
                    name = "sum".into();
                }
            }
        } else {
            name = "sum".into();
        }
    }
    match name.as_str() {
        "size" => {
            if !ai {
                return frame_result(an, st, Row::closed_with(KeySet::one(mk("size"))), false);
            }
            series_result(an, st, None, false)
        }
        "sum" | "mean" | "min" | "max" | "count" | "median" | "std" | "var" | "first" | "last" | "nunique" | "prod" | "sem" | "skew" | "any" | "all" | "quantile" | "idxmax" | "idxmin" | "ohlc" | "max_" => {
            if let Some(col) = single_sel {
                if window.is_some() && grouped && ai {
                    return series_result(an, st, Some(col), false);
                }
                if window.is_some() {
                    return series_result(an, st, Some(col), true);
                }
                return series_result(an, st, Some(col), false);
            }
            let mut row = value_row.clone();
            if let Some(o) = on {
                // rolling(on=c) keeps c; resample(on=c) moves it to the index
                if window.as_deref() == Some("resample") {
                    row = row.delete(o, true);
                } else {
                    row = row.store(o, true);
                }
            }
            if base.closed() {
                row = Row::from_data(RowData { present: row.present.clone(), definite: row.definite.clone(), likely: row.likely.clone(), order: row.data().order.clone(), ..Default::default() });
            }
            let same = window.is_some() && !grouped && window.as_deref() != Some("resample");
            let v = frame_result(an, st, row, same);
            if window.as_deref() == Some("resample") {
                if let Some(nid) = v.single() {
                    let lv = match on {
                        Some(o) => Some(vec![Some(o)]),
                        None => levels_of(st, &fs),
                    };
                    set_index_levels(st, nid, lv.as_deref());
                    set_labels(st, nid, None);
                }
            }
            v
        }
        "apply" | "pipe" => {
            if let Some(fnv) = c.args.first() {
                if !fnv.pts.is_empty() {
                    // each group without its key columns (pandas 3)
                    let mut grow = base.clone();
                    for k in keys.iter() {
                        grow = grow.delete(k, true);
                    }
                    if let Some((cols, _)) = &sel {
                        grow = Row::closed_with(cols.clone());
                    }
                    let vid = an.alloc_at(Kind::Frame, &c.node, st, &format!("group@L{}", c.line), Payload::None, Some(grow), Ns::Item, None, None);
                    let mut oargs = vec![Value::obj(vid)];
                    oargs.extend(c.args[1..].iter().cloned());
                    let res = an.call_value(fnv, &c.node, None, st, Some(oargs));
                    let rfs = frames(an, Some(&res));
                    if !rfs.is_empty() {
                        let row = rows_of(st, &rfs);
                        let v = new_frame(an, st, c, row, None, None);
                        if let Some(nid) = v.single() {
                            set_index_levels(st, nid, None);
                        }
                        return v;
                    }
                    let recs: Vec<u32> = res.pts.iter().filter(|o| an.kind(*o) == Kind::Dict).collect();
                    if !recs.is_empty() {
                        // a Series per group: its index becomes the columns
                        let row = join_rows(recs.iter().map(|x| st.row(*x, Ns::Item)));
                        return frame_result(an, st, row, false);
                    }
                    return series_result(an, st, None, false);
                }
            }
            series(an)
        }
        "transform" | "rank" | "cumsum" | "cumprod" | "cummax" | "cummin" | "shift" | "diff" | "pct_change" | "fillna" | "ffill" | "bfill" => {
            if let Some(col) = single_sel {
                return series_result(an, st, Some(col), true);
            }
            frame_result(an, st, value_row, true)
        }
        "cumcount" | "ngroup" => series_result(an, st, None, true),
        "head" | "tail" | "nth" | "sample" | "filter" | "take" => {
            let row = match &sel {
                Some((cols, _)) if name != "filter" => Row::closed_with(cols.clone()),
                _ => base,
            };
            derived(an, st, c, row, &fs, Rows::Subset, true)
        }
        "get_group" => derived(an, st, c, base, &fs, Rows::Subset, true),
        "value_counts" => series_result(an, st, Some(mk(if key_is_bool(&c.kw("normalize"), true) { "proportion" } else { "count" })), false),
        "describe" => {
            // one row per group; a selected column's statistics are the columns
            let kind = match (single_sel, fs.as_slice()) {
                (Some(col), [f]) => dtype(st, *f, col),
                _ => None,
            };
            let row = match kind.as_deref() {
                Some("int") | Some("float") | Some("num") => describe_labels(true, false).row(),
                Some(_) => describe_labels(false, true).row(),
                None => Row::with_source(Source::new(&format!("describe@L{}", c.line), SourceKind::Call)),
            };
            frame_result(an, st, row, false)
        }
        "rolling" | "expanding" | "ewm" | "resample" => window_of(an, st, c, fs.clone(), ks.clone(), Some(g0), &name),
        "__getitem__" => {
            let key = c.args.first().cloned().unwrap_or_default();
            groupby_select(an, g0, &key, &c.node, st)
        }
        _ => series(an),
    }
}

/// A row of a frame (iterrows / itertuples / apply / records): accesses on it narrow the
/// row, not the frame (a frame without rows never runs them).
fn mark_view(an: &mut Analyzer, vrow: u32, fs: &[u32]) {
    an.obj_mut(vrow).iter_elem = true;
    if fs.len() == 1 && crate::interp::disabled("framerowsound") {
        an.obj_mut(vrow).derived_from = Some(fs[0]);
    }
}

fn df_apply(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let fnv = c.arg(0, Some("func"));
    let rowwise = axis_is_columns(&c.kw("axis"));
    let Some(fnv) = fnv.filter(|f| !f.pts.is_empty()) else { return series(an) };
    let base = recv_row(an, st, c);
    let expand = key_is(&c.kw("result_type"), "expand");
    if rowwise {
        let label = format!("row of {}", fs.first().map(|f| an.obj(*f).label.clone()).unwrap_or_else(|| "frame".into()));
        let vrow = an.alloc_at(Kind::Dict, &c.node, st, &label, Payload::None, Some(base.clone()), Ns::Item, Some(false), None);
        mark_view(an, vrow, &fs);
        for k in base.present.iter() {
            st.set_slot(vrow, Ns::Item, k, Value::data(&format!("cell {}", intern::key_py_str(k))), false);
        }
        let mut oargs = vec![Value::obj(vrow)];
        oargs.extend(c.args[1..].iter().cloned());
        let res = an.call_value(&fnv, &c.node, None, st, Some(oargs));
        let sers: Vec<u32> = res.pts.iter().filter(|o| is_series_obj(an, *o)).collect();
        let dicts: Vec<u32> = res.pts.iter().filter(|o| an.kind(*o) == Kind::Dict && !is_series_obj(an, *o)).collect();
        // a Series per row (or dicts with result_type="expand"): its keys are the columns
        let recs: Vec<u32> = if expand { sers.iter().chain(dicts.iter()).copied().collect() } else { sers.clone() };
        if !recs.is_empty() && recs.len() + if expand { 0 } else { dicts.len() } == res.pts.len() {
            let row = join_rows(recs.iter().map(|x| st.row(*x, Ns::Item)));
            return derived(an, st, c, row, &fs, Rows::Same, false);
        }
        // anything else: one value per row (a dict per row stays a dict)
        let idx = match fs.as_slice() {
            [f] => labels_row(st, *f),
            _ => Row::with_source(Source::new("index", SourceKind::Data)),
        };
        let sid = new_series(an, st, &c.node, idx, None, "a");
        if !dicts.is_empty() {
            st.set_slot(sid, Ns::Meta, mk("elem"), res.clone(), true);
        }
        return Value::obj(sid);
    }
    // column by column: f gets each column (a Series over the rows)
    let col = match fs.as_slice() {
        [f] => labels_row(st, *f),
        _ => Row::with_source(Source::new("index", SourceKind::Data)),
    };
    let cid = new_series(an, st, &c.node, col, Some(Value::data("column name")), "col");
    // the columns' common kind (numeric / object) is the column Series' dtype
    if let [f] = fs.as_slice() {
        let dts: Vec<Option<String>> = base.present.iter().map(|k| dtype(st, *f, k)).collect();
        let num = |d: &Option<String>| matches!(d.as_deref(), Some("int") | Some("float"));
        if !dts.is_empty() && base.closed() && dts.iter().all(num) {
            st.set_slot(cid, Ns::Meta, dtype_key(vkey()), Value::lit(mk("float")), true);
        } else if !dts.is_empty() && base.closed() && dts.iter().all(|d| d.is_some() && !num(d)) {
            st.set_slot(cid, Ns::Meta, dtype_key(vkey()), Value::lit(mk("object")), true);
        }
    }
    let mut oargs = vec![Value::obj(cid)];
    oargs.extend(c.args[1..].iter().cloned());
    let res = an.call_value(&fnv, &c.node, None, st, Some(oargs));
    let sers: Vec<u32> = res.pts.iter().filter(|o| is_series_obj(an, *o) && *o != cid).collect();
    if !sers.is_empty() {
        // a Series per column: a frame with the frame's columns over the returned index
        let idx = join_rows(sers.iter().map(|x| st.row(*x, Ns::Item)));
        let v = new_frame(an, st, c, base.clone(), None, None);
        if let Some(nid) = v.single() {
            let lab = idx.closed().then(|| Known { may: idx.present.clone(), sure: idx.definite.clone() });
            set_labels(st, nid, lab.as_ref());
        }
        return v;
    }
    if is_seriesish(an, &res) {
        // an element-wise transform: the same frame shape
        return derived(an, st, c, base, &fs, Rows::Same, false);
    }
    // a scalar per column: a Series over the columns
    let sid = new_series(an, st, &c.node, Row::new().copy_from(&base, true, &KeySet::new(), EMPTY_LENS), None, "r");
    Value::obj(sid)
}

/// `df.agg(f)`: a string / function reduces each column (a Series over the columns); a list
/// gives a frame over the functions; a dict selects columns (a Series when every value is
/// one function, else a frame over the functions); named aggregations (k=(col, f)) give a
/// frame with the source columns, indexed by the names.
fn df_agg(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let row = recv_row(an, st, c);
    let set_lab = |an: &mut Analyzer, st: &mut State, v: &Value, lab: Option<Known>| {
        let _ = an;
        if let Some(nid) = v.single() {
            set_index_levels(st, nid, Some(&[None]));
            set_labels(st, nid, lab.as_ref());
        }
    };
    if !c.kwargs.is_empty() && c.args.is_empty() && !c.kwargs.contains_key(&intern::s("func")) {
        let mut cols = Row::new();
        let mut names = Vec::new();
        for (k, v) in c.kwargs.iter() {
            names.push(intern::key(intern::Key::Str(intern::str_get(*k))));
            for x in v.pts.iter() {
                if an.kind(x) == Kind::Tuple && an.llen(x, st) == Some(2) {
                    if let Some(col) = st.slot(x, Ns::Pos, intern::key_int(0)).kv.as_lit() {
                        cols = cols.store(col, true);
                    }
                }
            }
        }
        let _ = require_cols(an, st, c, &fs, &cols.present);
        let v = new_frame(an, st, c, cols, None, None);
        set_lab(an, st, &v, Some(Known::exact(KeySet::from_iter(names))));
        return v;
    }
    let Some(spec) = c.arg(0, Some("func")) else { return open_frame_call(an, st, c) };
    if let Some(d) = spec.single().filter(|o| an.kind(*o) == Kind::Dict) {
        let r = st.row(d, Ns::Item);
        if !(r.closed() && r.present == r.definite) {
            return open_frame_call(an, st, c);
        }
        let _ = require_cols(an, st, c, &fs, &r.present);
        let mut funcs: Option<KeySet> = Some(KeySet::new());
        let mut multi = false;
        for k in r.present.iter() {
            let v = st.slot(d, Ns::Item, k);
            match agg_names(an, st, &v) {
                Some((fnames, is_list)) => {
                    multi |= is_list;
                    funcs = funcs.map(|f0| f0.union(&KeySet::from_iter(fnames)));
                }
                None => funcs = None,
            }
        }
        let cols = Row::new().copy_from(&r, true, &KeySet::new(), EMPTY_LENS);
        if !multi {
            let sid = new_series(an, st, &c.node, cols, None, "agg");
            return Value::obj(sid);
        }
        // the index is every function named (NaN where a column did not use one)
        let v = new_frame(an, st, c, cols, None, None);
        set_lab(an, st, &v, funcs.map(Known::exact));
        return v;
    }
    match agg_names(an, st, &spec) {
        Some((fnames, true)) => {
            let v = derived(an, st, c, row, &fs, Rows::Other, false);
            set_lab(an, st, &v, Some(Known::exact(KeySet::from_iter(fnames))));
            v
        }
        _ => {
            let mut d = row.data();
            d.terms = Default::default();
            d.dterms = Default::default();
            d.lterms = Default::default();
            let sid = new_series(an, st, &c.node, Row::from_data(d), None, "agg");
            Value::obj(sid)
        }
    }
}

/// `corr` / `cov` (numeric_only=True): square over the numeric columns.
fn df_corr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let row = recv_row(an, st, c);
    let [f] = fs.as_slice() else { return open_frame_call(an, st, c) };
    if !row.closed() {
        return open_frame_call(an, st, c);
    }
    let mut d = row.data();
    for k in row.present.iter() {
        match dtype(st, *f, k).as_deref() {
            Some("int") | Some("float") | Some("bool") => {}
            Some(_) => {
                d.present = d.present.remove(k);
                d.definite = d.definite.remove(k);
                d.likely = d.likely.remove(k);
                d.absent = d.absent.insert(k);
            }
            None => return open_frame_call(an, st, c),
        }
    }
    d.terms = Default::default();
    d.dterms = Default::default();
    d.lterms = Default::default();
    let r = Row::from_data(d);
    let lab = Known { may: r.present.clone(), sure: r.definite.clone() };
    let v = new_frame(an, st, c, r, None, None);
    if let Some(nid) = v.single() {
        set_index_levels(st, nid, Some(&[None]));
        set_labels(st, nid, Some(&lab));
    }
    v
}

/// `droplevel(level, axis=)` / `swaplevel(i=-2, j=-1, axis=)`: on MultiIndex columns (tuple
/// keys) with axis=1, else on the index levels.
fn levels_op(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let row = recv_row(an, st, c);
    let swap = c.qual.ends_with("swaplevel");
    let on_cols = axis_is_columns(&c.kw("axis").or_else(|| if swap { c.arg(2, None) } else { c.arg(1, None) }));
    let int_of = |v: Option<Value>, d: i64| v.and_then(|x| x.kv.as_lit()).and_then(intern::key_as_int).unwrap_or(d);
    let tuple_of = |k: KeyId| intern::with_key(k, |kk| if let intern::Key::Tuple(t) = kk { Some(t.clone()) } else { None });
    if on_cols {
        let n = row.present.iter().filter_map(tuple_of).map(|t| t.len()).max().unwrap_or(0) as i64;
        if n < 2 || row.present.iter().any(|k| tuple_of(k).map(|t| t.len() as i64 != n).unwrap_or(true)) {
            return open_frame_call(an, st, c);
        }
        let norm = |i: i64| if i < 0 { i + n } else { i };
        let map: Box<dyn Fn(&[KeyId]) -> Vec<KeyId>> = if swap {
            let (i, j) = (norm(int_of(c.arg(0, Some("i")), -2)) as usize, norm(int_of(c.arg(1, Some("j")), -1)) as usize);
            Box::new(move |t: &[KeyId]| {
                let mut v = t.to_vec();
                v.swap(i, j);
                v
            })
        } else {
            let l = norm(int_of(c.arg(0, Some("level")), 0)) as usize;
            Box::new(move |t: &[KeyId]| t.iter().enumerate().filter(|(i, _)| *i != l).map(|(_, k)| *k).collect())
        };
        let conv = |k: KeyId| -> KeyId {
            let parts = map(&tuple_of(k).unwrap());
            if parts.len() == 1 { parts[0] } else { intern::key_tuple(parts) }
        };
        let mut out = Row::new();
        for k in row.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(row.present.iter())) {
            out = out.store(conv(k), false);
        }
        for k in row.definite.iter() {
            out = out.store(conv(k), true);
        }
        if row.open() {
            let mut d = out.data();
            d.sources = intern::IdSet::one((Source::new(&format!("{}@L{}", last(&c.qual), c.line), SourceKind::Call), EMPTY_LENS));
            out = Row::from_data(d);
        }
        return derived(an, st, c, out, &fs, Rows::Same, false);
    }
    let v = derived(an, st, c, row, &fs, Rows::Same, true);
    if let Some(nid) = v.single() {
        let lv = levels_of(st, &fs).map(|mut l| {
            let n = l.len() as i64;
            let norm = |i: i64| (if i < 0 { i + n } else { i }) as usize;
            if swap {
                let (i, j) = (norm(int_of(c.arg(0, Some("i")), -2)), norm(int_of(c.arg(1, Some("j")), -1)));
                if i < l.len() && j < l.len() {
                    l.swap(i, j);
                }
            } else if let Some(k) = c.arg(0, Some("level")).and_then(|x| x.kv.as_lit()) {
                let pos = if intern::key_is_str(k) { l.iter().position(|x| *x == Some(k)) } else { intern::key_as_int(k).map(norm) };
                if let Some(p0) = pos.filter(|p0| *p0 < l.len()) {
                    l.remove(p0);
                }
            }
            l
        });
        set_index_levels(st, nid, lv.as_deref());
        set_labels(st, nid, None);
    }
    v
}

/// The statistics `describe()` gives for numeric / other columns.
const NUM_STATS: &[&str] = &["count", "mean", "std", "min", "25%", "50%", "75%", "max"];
const OBJ_STATS: &[&str] = &["count", "unique", "top", "freq"];

fn describe_labels(num: bool, obj: bool) -> Known {
    let mut ks: Vec<KeyId> = Vec::new();
    if obj {
        ks.extend(OBJ_STATS.iter().map(|x| mk(x)));
    }
    if num {
        for k in NUM_STATS.iter().map(|x| mk(x)) {
            if !ks.contains(&k) {
                ks.push(k);
            }
        }
    }
    Known::exact(KeySet::from_iter(ks))
}

fn describe(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let row = recv_row(an, st, c);
    let all = key_is(&c.kw("include"), "all");
    let [f] = fs.as_slice() else { return open_frame_call(an, st, c) };
    if !row.closed() || c.kw("include").map(|_| !all).unwrap_or(false) || c.kw("exclude").is_some() {
        let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.insert((Source::new(&format!("describe@L{}", c.line), SourceKind::Call), EMPTY_LENS)), ..Default::default() });
        return new_frame(an, st, c, r, None, None);
    }
    let mut kinds: Vec<(KeyId, Option<bool>)> = Vec::new();
    for k in row.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(row.present.iter())) {
        let numeric = dtype(st, *f, k).map(|d| matches!(d.as_str(), "int" | "float" | "num"));
        kinds.push((k, numeric));
    }
    if kinds.iter().any(|(_, n)| n.is_none()) {
        if crate::interp::debug_level() == 2 {
            eprintln!("  describe@L{}: unknown dtypes {:?}", c.line, kinds.iter().filter(|(_, n)| n.is_none()).map(|(k, _)| intern::key_py_str(*k)).collect::<Vec<_>>());
        }
        let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.insert((Source::new(&format!("describe@L{}", c.line), SourceKind::Call), EMPTY_LENS)), ..Default::default() });
        return new_frame(an, st, c, r, None, None);
    }
    let any_num = kinds.iter().any(|(k, n)| *n == Some(true) && row.present.contains(*k));
    let any_num_def = kinds.iter().any(|(k, n)| *n == Some(true) && row.definite.contains(*k));
    let chosen: Vec<(KeyId, bool)> = kinds.iter().filter(|(_, n)| all || !any_num || *n == Some(true)).map(|(k, n)| (*k, n.unwrap_or(false))).collect();
    let mut out = Row::new();
    for (k, _) in &chosen {
        out = out.store(*k, row.definite.contains(*k) && (all || !any_num || any_num_def));
    }
    let has_num = chosen.iter().any(|(_, n)| *n);
    let has_obj = chosen.iter().any(|(_, n)| !*n);
    let v = new_frame(an, st, c, out, None, None);
    fresh_index(st, &v);
    if let Some(nid) = v.single() {
        let lab = describe_labels(has_num, has_obj);
        set_labels(st, nid, Some(&lab));
        set_nrows(st, nid, Some(lab.may.len() as i64));
    }
    v
}

fn df_pipe(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(fnv) = c.args.first() else { return c.recv.clone().unwrap_or_default() };
    let mut oargs = vec![c.recv.clone().unwrap_or_default()];
    oargs.extend(c.args[1..].iter().cloned());
    an.call_value(fnv, &c.node, None, st, Some(oargs))
}

fn iterrows(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let base = recv_row(an, st, c);
    let fs = frames(an, c.recv.as_ref());
    let vrow = an.alloc_at(Kind::Dict, &c.node, st, &format!("row@L{}", c.line), Payload::None, Some(base.clone()), Ns::Item, Some(false), None);
    mark_view(an, vrow, &fs);
    for k in base.present.iter() {
        st.set_slot(vrow, Ns::Item, k, Value::data(&format!("cell {}", intern::key_py_str(k))), false);
    }
    let ctx = an.frame().ctx.clone();
    let vid = an.alloc_raw(Kind::View, &format!("view:rows:{}", c.site), &ctx, true, "iterrows", Payload::View(ViewKind::Rows, Value::obj(vrow)));
    an.obj_mut(vid).payload = Payload::View(ViewKind::Rows, Value::obj(vrow));
    Value::obj(vid)
}

fn is_identifier(s0: &str) -> bool {
    const KW: &[&str] = &["False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield"];
    let mut cs = s0.chars();
    match cs.next() {
        Some(ch) if ch.is_alphabetic() || ch == '_' => {}
        _ => return false,
    }
    cs.all(|ch| ch.is_alphanumeric() || ch == '_') && !KW.contains(&s0)
}

/// `itertuples(index=True, name="Pandas")`: namedtuples whose fields are Index and the
/// columns (a name that is no identifier, starts with "_" or repeats becomes `_<position>`);
/// `name=None`: plain tuples.
fn itertuples(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let with_index = !key_is_bool(&c.kw("index").or_else(|| c.arg(0, None)), false);
    let plain = c.kw("name").map(|v| v.kv == an.none_v.kv && v.pts.is_empty()).unwrap_or(false);
    let mut rows_v = Value::new();
    for (j, f) in fs.iter().enumerate() {
        let base = st.row(*f, Ns::Item);
        let order = base.ordered_keys().filter(|_| base.closed());
        let mut fields: Vec<(KeyId, bool)> = Vec::new();
        if with_index {
            fields.push((mk("Index"), true));
        }
        match &order {
            Some(o) => {
                let mut seen: Vec<String> = Vec::new();
                for k in o {
                    let pos = fields.len();
                    let name = intern::key_py_str(*k);
                    let ok = intern::key_is_str(*k) && is_identifier(&name) && !name.starts_with('_') && !seen.contains(&name) && !(with_index && name == "Index");
                    seen.push(name.clone());
                    fields.push((if ok { *k } else { mk(&format!("_{}", pos)) }, base.definite.contains(*k)));
                }
            }
            None => {
                for k in base.present.iter() {
                    if intern::key_is_str(k) && is_identifier(&intern::key_py_str(k)) {
                        fields.push((k, base.definite.contains(k)));
                    }
                }
            }
        }
        let site = format!("{}:t{}", crate::interp::site_string(&c.node), j);
        let vrow = if plain {
            let t = an.alloc(Kind::Namespace, &site, st, &format!("tuple@L{}", c.line), Payload::None, Some(Row::new()), Ns::Attr, Some(false), None);
            if order.is_some() && base.present == base.definite {
                an.set_llen(t, st, Some(fields.len()));
                for i in 0..fields.len() {
                    st.set_slot(t, Ns::Pos, intern::key_int(i as i64), Value::data("cell"), true);
                }
            }
            t
        } else {
            let mut r = Row::new();
            for (k, def) in &fields {
                r = r.store(*k, *def);
            }
            if order.is_none() && base.open() {
                let mut d = r.data();
                d.sources = base.sources.clone();
                r = Row::from_data(d);
            }
            let t = an.alloc(Kind::Namespace, &site, st, &format!("row@L{}", c.line), Payload::None, Some(r.clone()), Ns::Attr, Some(false), None);
            for (k, _) in &fields {
                st.set_slot(t, Ns::Attr, *k, Value::data(&format!("cell {}", intern::key_py_str(*k))), false);
            }
            if order.is_some() {
                an.set_llen(t, st, Some(fields.len()));
            }
            t
        };
        mark_view(an, vrow, &[*f]);
        rows_v = rows_v.join(&Value::obj(vrow));
    }
    if rows_v.pts.is_empty() {
        return Value::data(&format!("itertuples@L{}", c.line));
    }
    let ctx = an.frame().ctx.clone();
    let vid = an.alloc_raw(Kind::View, &format!("view:elems:{}", c.site), &ctx, true, "itertuples", Payload::View(ViewKind::Elems, rows_v.clone()));
    an.obj_mut(vid).payload = Payload::View(ViewKind::Elems, rows_v);
    Value::obj(vid)
}

/// A list object of known literal items (a frame's columns / labels, in order).
fn lit_list(an: &mut Analyzer, st: &mut State, node: &Node, tag: &str, items: &[KeyId], kind: Kind) -> u32 {
    let site = format!("{}:{}", crate::interp::site_string(node), tag);
    let lid = an.alloc(kind, &site, st, &format!("{}@L{}", kind.name(), node.line), Payload::None, None, Ns::Item, None, None);
    an.set_llen(lid, st, Some(items.len()));
    for (i, k) in items.iter().enumerate() {
        st.set_slot(lid, Ns::Pos, intern::key_int(i as i64), Value::lit(*k), true);
    }
    st.elems.insert(lid, if items.is_empty() { KeyVal::Bot } else { KeyVal::lits(KeySet::from_iter(items.iter().copied())) });
    lid
}

/// A dict object with this row (each key's value: `val`).
fn dict_with(an: &mut Analyzer, st: &mut State, node: &Node, tag: &str, row: Row, val: &dyn Fn(KeyId) -> Value) -> u32 {
    let site = format!("{}:{}", crate::interp::site_string(node), tag);
    let did = an.alloc(Kind::Dict, &site, st, &format!("dict@L{}", node.line), Payload::None, Some(row.clone()), Ns::Item, None, None);
    for k in row.present.iter() {
        st.set_slot(did, Ns::Item, k, val(k), true);
    }
    did
}

fn to_dict(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let orient = c.arg(0, Some("orient"));
    let o = lit_key(&orient).map(intern::key_py_str).unwrap_or_else(|| "dict".into());
    let mut out = Value::new();
    for (j, f) in fs.iter().enumerate() {
        let base = st.row(*f, Ns::Item);
        let labs = labels_row(st, *f);
        let tag = |t: &str| format!("{}{}", t, j);
        let v = match o.as_str() {
            "records" => {
                let fv: Vec<(KeyId, Option<Known>)> = base.present.iter().map(|k| (k, vals(st, *f, k))).collect();
                let vrow = dict_with(an, st, &c.node, &tag("rec"), base.clone(), &|k| match fv.iter().find(|(x, _)| *x == k).and_then(|(_, v)| v.clone()) {
                    Some(v) if !v.may.is_empty() => Value::kv_only(KeyVal::lits(v.may.clone())),
                    _ => Value::data(&format!("cell {}", intern::key_py_str(k))),
                });
                mark_view(an, vrow, &[*f]);
                let site = format!("{}:{}", crate::interp::site_string(&c.node), tag("recs"));
                let lid = an.alloc(Kind::List, &site, st, &format!("records@L{}", c.line), Payload::None, None, Ns::Item, None, None);
                // one record per row (positions known when the row count is)
                let n = nrows(st, *f);
                an.set_llen(lid, st, n.map(|n| n as usize));
                if let Some(n) = n {
                    for i in 0..n {
                        st.set_slot(lid, Ns::Pos, intern::key_int(i), Value::obj(vrow), true);
                    }
                }
                st.set_slot(lid, Ns::Elem, intern::STAR, Value::obj(vrow), true);
                st.elems.insert(lid, DATA);
                Value::obj(lid)
            }
            "dict" | "list" | "series" => {
                // {column: {label: v}} / {column: [v, …]} / {column: Series}
                let mut inner: IndexMap<KeyId, Value> = IndexMap::new();
                for k in base.present.iter() {
                    let iv = match o.as_str() {
                        "dict" => Value::obj(dict_with(an, st, &c.node, &tag(&format!("c{}", intern::key_py_str(k))), labs.clone(), &|_| Value::data("cell"))),
                        "series" => column_series(an, st, *f, k, &c.node),
                        _ => Value::data("list"),
                    };
                    inner.insert(k, iv);
                }
                let oid = dict_with(an, st, &c.node, &tag("d"), Row::new().copy_from(&base, true, &KeySet::new(), EMPTY_LENS), &|k| inner.get(&k).cloned().unwrap_or_default());
                Value::obj(oid)
            }
            "index" => {
                let rec = dict_with(an, st, &c.node, &tag("r"), Row::new().copy_from(&base, true, &KeySet::new(), EMPTY_LENS), &|_| Value::data("cell"));
                let oid = dict_with(an, st, &c.node, &tag("i"), labs.clone(), &|_| Value::obj(rec));
                Value::obj(oid)
            }
            "split" | "tight" => {
                let cols = base.ordered_keys().filter(|_| base.closed() && base.present == base.definite);
                let cv = match &cols {
                    Some(cs) => Value::obj(lit_list(an, st, &c.node, &tag("cols"), cs, Kind::List)),
                    None => Value::data("columns"),
                };
                let mut keys = vec![mk("index"), mk("columns"), mk("data")];
                if o == "tight" {
                    keys.push(mk("index_names"));
                    keys.push(mk("column_names"));
                }
                let oid = dict_with(an, st, &c.node, &tag("s"), closed_ordered(&keys), &|k| if k == mk("columns") { cv.clone() } else { Value::data("split") });
                Value::obj(oid)
            }
            _ => Value::data("to_dict"),
        };
        out = out.join(&v);
    }
    if out.pts.is_empty() && out.kv == KeyVal::Bot {
        return Value::data("to_dict");
    }
    out
}

/// `to_records(index=True)`: a record array whose fields are the index name(s) and columns.
fn to_records(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let with_index = !key_is_bool(&c.kw("index").or_else(|| c.arg(0, None)), false);
    let mut out = Value::new();
    for (j, f) in fs.iter().enumerate() {
        let base = st.row(*f, Ns::Item);
        let mut fields: Vec<(KeyId, bool)> = Vec::new();
        if with_index {
            match index_levels(st, *f) {
                Some(lv) => {
                    for i in 0..lv.len() {
                        fields.push((level_column(&lv, i), true));
                    }
                }
                None => return Value::data("to_records"),
            }
        }
        let cols = base.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(base.present.iter()));
        for k in &cols {
            fields.push((*k, base.definite.contains(*k)));
        }
        let mut row = Row::new();
        for (k, d) in &fields {
            row = row.store(*k, *d);
        }
        if base.open() {
            let mut d = row.data();
            d.sources = base.sources.clone();
            row = Row::from_data(d);
        }
        let names: Option<Vec<KeyId>> = (base.closed() && base.present == base.definite).then(|| fields.iter().map(|(k, _)| *k).collect());
        let tv = match &names {
            Some(n) => Value::obj(lit_list(an, st, &c.node, &format!("n{}", j), n, Kind::Tuple)),
            None => Value::data("names"),
        };
        let fd = dict_with(an, st, &c.node, &format!("f{}", j), row.clone(), &|_| Value::data("field"));
        let dt = an.alloc(Kind::Namespace, &format!("{}:dt{}", crate::interp::site_string(&c.node), j), st, &format!("dtype@L{}", c.line), Payload::None, Some(closed_ordered(&[mk("names"), mk("fields")])), Ns::Attr, None, None);
        st.set_slot(dt, Ns::Attr, mk("names"), tv, true);
        st.set_slot(dt, Ns::Attr, mk("fields"), Value::obj(fd), true);
        let rrow = row.store(mk("dtype"), true);
        let rec = an.alloc(Kind::Namespace, &format!("{}:rec{}", crate::interp::site_string(&c.node), j), st, &format!("recarray@L{}", c.line), Payload::None, Some(rrow), Ns::Attr, None, None);
        for (k, _) in &fields {
            st.set_slot(rec, Ns::Attr, *k, Value::data("field"), true);
        }
        st.set_slot(rec, Ns::Attr, mk("dtype"), Value::obj(dt), true);
        st.set_slot(rec, Ns::Meta, mk("recfields"), Value::obj(fd), true);
        out = out.join(&Value::obj(rec));
    }
    if out.pts.is_empty() {
        return Value::data("to_records");
    }
    out
}

fn df_insert(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let col = c.arg(1, Some("column"));
    let val = c.arg(2, Some("value")).unwrap_or_default();
    for f in frames(an, c.recv.as_ref()) {
        if let Some(col) = &col {
            let single = an.obj(f).single;
            frame_setitem(an, f, col, &val, &c.node, st, single);
        }
    }
    an.none_v.clone()
}

fn df_pop(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let col = c.arg(0, Some("item"));
    for f in frames(an, c.recv.as_ref()) {
        if let Some(col) = &col {
            let single = an.obj(f).single;
            frame_delitem(an, f, col, &c.node, st, single, true);
        }
    }
    series(an)
}

// ------------------------------------------------------------- reshaping
/// The frame a reshaping call works on: the receiver, or the first argument (`pd.melt(df)`).
fn data_frames(an: &Analyzer, c: &Call) -> Vec<u32> {
    let r = frames(an, c.recv.as_ref());
    if !r.is_empty() {
        return r;
    }
    frames(an, c.args.first().or_else(|| c.kwargs.get(&intern::s("frame"))).or_else(|| c.kwargs.get(&intern::s("data"))))
}

/// A literal key or list of keys (in order).
fn keys_arg(an: &Analyzer, st: &State, v: &Option<Value>) -> Option<Vec<KeyId>> {
    let v = v.as_ref()?;
    if let (Some(k), true) = (v.kv.as_lit(), v.pts.is_empty()) {
        return Some(vec![k]);
    }
    lit_key_seq(an, st, v)
}

fn melt(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = data_frames(an, c);
    let off = if frames(an, c.recv.as_ref()).is_empty() { 1 } else { 0 };
    let idv = c.kw("id_vars").or_else(|| c.arg(off, None));
    let vv = c.kw("value_vars").or_else(|| c.arg(off + 1, None));
    let ids = match &idv {
        Some(v) if v.kv != an.none_v.kv || !v.pts.is_empty() => keys_arg(an, st, &idv),
        _ => Some(Vec::new()),
    };
    let Some(ids) = ids else { return open_frame_call(an, st, c) };
    let mut need = KeySet::from_iter(ids.iter().copied());
    if let Some(v) = keys_arg(an, st, &vv) {
        need = need.union(&KeySet::from_iter(v));
    }
    let fs = require_cols(an, st, c, &fs, &need);
    let var = match c.kw("var_name") {
        Some(v) => lit_key(&Some(v)),
        None => {
            let cn: Vec<Option<KeyId>> = fs.iter().map(|f| colname(st, *f)).collect();
            if cn.iter().all(|x| x.is_some() && *x == cn[0]) && !cn.is_empty() { cn[0] } else if cn.iter().all(|x| x.is_none()) { Some(mk("variable")) } else { None }
        }
    };
    let val = lit_key(&c.kw("value_name")).unwrap_or_else(|| mk("value"));
    let mut seq = ids.clone();
    let mut row = closed_ordered(&seq);
    match var {
        Some(k) => {
            seq.push(k);
            row = closed_ordered(&seq);
        }
        None => {
            let mut d = row.data();
            d.sources = intern::IdSet::one((Source::new(&format!("melt@L{}", c.line), SourceKind::Call), EMPTY_LENS));
            row = Row::from_data(d);
        }
    }
    row = row.store(val, true);
    let v = new_frame(an, st, c, row, None, None);
    fresh_index(st, &v);
    if let ([f], Some(nid), Some(k)) = (fs.as_slice(), v.single(), var) {
        // the variable column holds the folded columns' names
        let folded = match keys_arg(an, st, &vv) {
            Some(vs) => Some(Known::exact(KeySet::from_iter(vs))),
            None => {
                let r = st.row(*f, Ns::Item);
                r.closed().then(|| {
                    let ids_s = KeySet::from_iter(ids.iter().copied());
                    Known { may: r.present.difference(&ids_s), sure: r.definite.difference(&ids_s) }
                })
            }
        };
        // each name occurs only if the frame has rows
        let folded = if nrows(st, *f).map(|n| n > 0).unwrap_or(false) { folded } else { folded.map(|k| k.subset()) };
        set_vals(st, nid, k, folded.as_ref());
        for i in &ids {
            let vv2 = vals(st, *f, *i).map(|x| x.subset());
            set_vals(st, nid, *i, vv2.as_ref());
        }
    }
    v
}

/// Index of a level named / numbered by `lv` among `levels` (default: the last).
fn level_index(levels: &[Option<KeyId>], lv: &Option<Value>) -> Option<usize> {
    let Some(k) = lv.as_ref().and_then(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())) else { return levels.len().checked_sub(1) };
    if let Some(i) = intern::key_as_int(k).filter(|_| !intern::key_is_str(k)) {
        let i = if i < 0 { i + levels.len() as i64 } else { i };
        return (0..levels.len() as i64).contains(&i).then_some(i as usize);
    }
    levels.iter().position(|l| *l == Some(k))
}

/// `s.unstack(level)` / `df.unstack(level)`: an index level's values become the columns.
fn unstack(an: &mut Analyzer, st: &mut State, c: &Call, src: u32, is_series: bool) -> Value {
    let levels = index_levels(st, src).unwrap_or_else(|| vec![None, None]);
    let Some(i) = level_index(&levels, &c.kw("level").or_else(|| c.arg(0, None))) else { return open_frame_call(an, st, c) };
    let newvals = lvals(st, src, i);
    let srow = st.row(src, Ns::Item);
    let row = if is_series {
        known_or_open(newvals.clone(), &srow, c.line, "unstack")
    } else {
        // a frame: (column, value) pairs
        match &newvals {
            Some(k) if srow.closed() => {
                let mut out = Row::new();
                for col in srow.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(srow.present.iter())) {
                    for v in intern::sorted_by_str(k.may.iter()) {
                        out = out.store(intern::key_tuple(vec![col, v]), srow.definite.contains(col) && k.sure.contains(v));
                    }
                }
                out
            }
            _ => known_or_open(None, &srow, c.line, "unstack"),
        }
    };
    let v = new_frame(an, st, c, row, None, None);
    if let Some(nid) = v.single() {
        let rest: Vec<Option<KeyId>> = levels.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, l)| *l).collect();
        let kept: Vec<usize> = (0..levels.len()).filter(|j| *j != i).collect();
        set_index_levels(st, nid, Some(if rest.is_empty() { &[None][..] } else { &rest }));
        for (j, old) in kept.iter().enumerate() {
            let lv = lvals(st, src, *old);
            set_lvals(st, nid, j, lv.as_ref());
        }
        let lab = if kept.len() == 1 { lvals(st, src, kept[0]) } else { None };
        set_labels(st, nid, lab.as_ref());
        st.set_slot(nid, Ns::Meta, mk("colname"), Value::lit(levels[i].unwrap_or_else(none_key)), true);
    }
    v
}

/// `df.stack()`: the columns move into the index (a Series; MultiIndex columns: the last level).
fn stack_frame(an: &mut Analyzer, st: &mut State, c: &Call, f: u32) -> Value {
    let row = st.row(f, Ns::Item);
    let is_tuple = |x: KeyId| intern::with_key(x, |kk| matches!(kk, intern::Key::Tuple(_)));
    let mut levels = index_levels(st, f).unwrap_or_else(|| vec![None]);
    let n = levels.len();
    if row.present.iter().any(is_tuple) {
        // the last column level moves: a frame over the first level
        let first = |x: KeyId| intern::with_key(x, |kk| if let intern::Key::Tuple(t) = kk { t.first().copied() } else { Some(x) });
        let present = KeySet::from_iter(row.present.iter().filter_map(first));
        let definite = KeySet::from_iter(row.definite.iter().filter_map(first));
        let mut d = Row::closed_with(present).data();
        d.definite = definite.clone();
        d.likely = definite;
        let v = new_frame(an, st, c, Row::from_data(d), None, None);
        if let Some(nid) = v.single() {
            levels.push(None);
            set_index_levels(st, nid, Some(&levels));
        }
        return v;
    }
    let sid = new_series(an, st, &c.node, Row::with_source(Source::new("index", SourceKind::Data)), None, "k");
    levels.push(colname(st, f));
    set_index_levels(st, sid, Some(&levels));
    for i in 0..n {
        let v = lvals(st, f, i);
        set_lvals(st, sid, i, v.as_ref());
    }
    let cols = row.closed().then(|| Known { may: row.present.clone(), sure: row.definite.clone() });
    set_lvals(st, sid, n, cols.as_ref());
    Value::obj(sid)
}

/// The values of `col` in frame `f` encoded as dummy columns `prefix + sep + value`.
fn dummy_cols(st: &State, f: Option<u32>, col: KeyId, sv: Option<Known>, prefix: Option<String>, sep: &str, drop_first: bool) -> Option<Row> {
    let k = match sv {
        Some(k) => k,
        None => vals(st, f?, col)?,
    };
    let nk = none_key();
    let k = Known { may: k.may.remove(nk), sure: k.sure.remove(nk) };
    let name = |v: KeyId| -> KeyId {
        match &prefix {
            Some(p) => key_str_lit(&format!("{}{}{}", p, sep, intern::key_py_str(v))),
            None => v,
        }
    };
    let mut row = Row::new();
    let exact = k.may == k.sure;
    let first = intern::sorted_by_str(k.may.iter()).into_iter().next();
    for v in intern::sorted_by_str(k.may.iter()) {
        if drop_first && exact && Some(v) == first {
            continue;
        }
        let def = k.sure.contains(v) && (!drop_first || exact);
        row = row.store(name(v), def);
    }
    if drop_first && exact {
        if let Some(fk) = first {
            row = row.delete(name(fk), true);
        }
    }
    Some(row)
}

fn get_dummies(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let data = c.arg(0, Some("data")).unwrap_or_default();
    let sep = lit_key(&c.kw("prefix_sep")).and_then(intern::key_as_str).map(|s| s.to_string()).unwrap_or_else(|| "_".into());
    let drop_first = key_is_bool(&c.kw("drop_first"), true);
    let prefix_v = c.kw("prefix");
    let prefix_for = |an: &mut Analyzer, st: &State, col: KeyId, j: usize| -> Option<String> {
        match &prefix_v {
            None => Some(intern::key_py_str(col)),
            Some(p) => {
                if let Some(s0) = lit_key(&Some(p.clone())).and_then(intern::key_as_str) {
                    return Some(s0.to_string());
                }
                if let Some(seq) = lit_key_seq(an, st, p) {
                    return seq.get(j).map(|k| intern::key_py_str(*k));
                }
                if let Some(m) = an.literal_mapping(p, st) {
                    return m.iter().find(|(k, _)| *k == col).map(|(_, v)| intern::key_py_str(*v));
                }
                None
            }
        }
    };
    // a Series: its values are the columns
    let sids: Vec<u32> = data.pts.iter().filter(|o| is_series_obj(an, *o)).collect();
    if !sids.is_empty() && sids.len() == data.pts.len() {
        let prefix = if prefix_v.is_some() { prefix_for(an, st, vkey(), 0) } else { None };
        let row = dummy_cols(st, None, vkey(), vals_join(st, &sids, vkey()), prefix, &sep, drop_first);
        let row = row.unwrap_or_else(|| known_or_open(None, &Row::new(), c.line, "get_dummies"));
        return new_frame(an, st, c, row, None, None);
    }
    let fs = frames(an, Some(&data));
    if fs.len() > 1 {
        // each frame on its own, joined
        let mut rows = Vec::new();
        for f in &fs {
            let mut c1: Call = (*c).clone();
            if !c1.args.is_empty() {
                c1.args[0] = Value::obj(*f);
            } else {
                c1.kwargs.insert(intern::s("data"), Value::obj(*f));
            }
            let v = get_dummies(an, st, &c1);
            rows.extend(frames(an, Some(&v)).iter().map(|x| st.row(*x, Ns::Item)));
        }
        return new_frame(an, st, c, join_rows(rows.into_iter()), None, None);
    }
    let [f] = fs.as_slice() else { return open_frame_call(an, st, c) };
    let f = *f;
    let row = st.row(f, Ns::Item);
    let cols: Option<Vec<KeyId>> = match c.kw("columns") {
        Some(v) => keys_arg(an, st, &Some(v)),
        None => {
            // the object / category columns
            if row.closed() {
                let all = row.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(row.present.iter()));
                let mut out = Vec::new();
                let mut ok = true;
                for k in all {
                    match dtype(st, f, k).as_deref() {
                        Some("object") => out.push(k),
                        Some(_) => {}
                        None => ok = false,
                    }
                }
                ok.then_some(out)
            } else {
                None
            }
        }
    };
    let Some(cols) = cols else {
        let mut d = row.data();
        d.definite = KeySet::new();
        d.likely = KeySet::new();
        d.sources = d.sources.insert((Source::new(&format!("get_dummies@L{}", c.line), SourceKind::Call), EMPTY_LENS));
        return new_frame(an, st, c, Row::from_data(d), None, None);
    };
    let fs2 = require_cols(an, st, c, &[f], &KeySet::from_iter(cols.iter().copied()));
    let mut out = rows_of(st, &fs2);
    for (j, col) in cols.iter().enumerate() {
        out = out.delete(*col, true);
        let prefix = prefix_for(an, st, *col, j);
        match dummy_cols(st, Some(f), *col, None, prefix, &sep, drop_first) {
            Some(r) => out = out.copy_from(&r, true, &KeySet::new(), EMPTY_LENS),
            None => {
                let mut d = out.data();
                d.sources = d.sources.insert((Source::new(&format!("get_dummies@L{}", c.line), SourceKind::Call), EMPTY_LENS));
                out = Row::from_data(d);
            }
        }
    }
    derived(an, st, c, out, &[f], Rows::Same, true)
}

/// `pivot` / `pivot_table`: the values of columns= become the columns.
fn pivot(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = data_frames(an, c);
    let table = c.qual.ends_with("pivot_table");
    let off = if frames(an, c.recv.as_ref()).is_empty() { 1 } else { 0 };
    let (vpos, ipos, cpos) = if table { (off, off + 1, off + 2) } else { (99, 99, 99) };
    let index = keys_arg(an, st, &c.kw("index").or_else(|| c.arg(ipos, None)));
    let columns = keys_arg(an, st, &c.kw("columns").or_else(|| c.arg(cpos, None)));
    let values_v = c.kw("values").or_else(|| c.arg(vpos, None));
    let values = keys_arg(an, st, &values_v);
    let values_single = values_v.as_ref().map(|v| v.pts.is_empty() && v.kv.as_lit().is_some()).unwrap_or(false);
    let mut need = KeySet::new();
    for ks in [&index, &columns, &values].into_iter().flatten() {
        need = need.union(&KeySet::from_iter(ks.iter().copied()));
    }
    let fs = require_cols(an, st, c, &fs, &need);
    let base = rows_of(st, &fs);
    let margins = key_is_bool(&c.kw("margins"), true);
    let mname = lit_key(&c.kw("margins_name")).unwrap_or_else(|| mk("All"));
    let aggdict: Option<Vec<KeyId>> = c.kw("aggfunc").and_then(|a| a.single()).filter(|o| an.kind(*o) == Kind::Dict).map(|o| {
        let r = st.row(o, Ns::Item);
        r.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(r.present.iter()))
    });
    // the value columns: values=, the aggfunc dict's keys, else the rest
    let vcols: Option<Vec<KeyId>> = values.clone().or(aggdict).or_else(|| {
        base.closed().then(|| {
            let used = KeySet::from_iter(index.iter().flatten().chain(columns.iter().flatten()).copied());
            base.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(base.present.iter())).into_iter().filter(|k| !used.contains(*k)).collect()
        })
    });
    let colvals: Option<Known> = match &columns {
        Some(cs) if cs.len() == 1 && !fs.is_empty() => vals_join(st, &fs, cs[0]),
        _ => None,
    };
    let row = match &columns {
        None => match &vcols {
            Some(vc) => closed_ordered(vc),
            None => known_or_open(None, &base, c.line, "pivot_table"),
        },
        Some(_) => {
            let flat = values_single || (!table && values.as_ref().map(|v| v.len() == 1).unwrap_or(false) && values_single);
            match (&colvals, &vcols) {
                (Some(k), _) if flat => {
                    let mut r = k.row();
                    if margins {
                        r = r.store(mname, true);
                    }
                    r
                }
                (Some(k), Some(vc)) => {
                    let mut out = Row::new();
                    for v in vc {
                        for cv in intern::sorted_by_str(k.may.iter()) {
                            out = out.store(intern::key_tuple(vec![*v, cv]), k.sure.contains(cv));
                        }
                        if margins {
                            out = out.store(intern::key_tuple(vec![*v, mname]), true);
                        }
                    }
                    out
                }
                _ => {
                    let mut r = known_or_open(None, &base, c.line, if table { "pivot_table" } else { "pivot" });
                    if margins && flat {
                        r = r.store(mname, true);
                    }
                    r
                }
            }
        }
    };
    let v = new_frame(an, st, c, row, None, None);
    if let Some(nid) = v.single() {
        let lv: Vec<Option<KeyId>> = index.clone().unwrap_or_default().into_iter().map(Some).collect();
        set_index_levels(st, nid, Some(if lv.is_empty() { &[None][..] } else { &lv }));
        let lab = match index.as_deref() {
            Some([k]) if !fs.is_empty() => vals_join(st, &fs, *k).map(|mut x| {
                if margins {
                    x.may = x.may.insert(mname);
                    x.sure = x.sure.insert(mname);
                }
                x
            }),
            _ => None,
        };
        set_labels(st, nid, lab.as_ref());
        if let Some([k]) = index.as_deref() {
            let lv0 = if fs.is_empty() { None } else { vals_join(st, &fs, *k) };
            set_lvals(st, nid, 0, lv0.as_ref());
        }
        if let Some([cc]) = columns.as_deref() {
            st.set_slot(nid, Ns::Meta, mk("colname"), Value::lit(*cc), true);
        }
    }
    v
}

fn crosstab(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let idx = c.arg(0, Some("index")).unwrap_or_default();
    let cols = c.arg(1, Some("columns")).unwrap_or_default();
    let margins = key_is_bool(&c.kw("margins"), true);
    let mname = lit_key(&c.kw("margins_name")).unwrap_or_else(|| mk("All"));
    let iss: Vec<u32> = idx.pts.iter().filter(|o| is_series_obj(an, *o)).collect();
    let css: Vec<u32> = cols.pts.iter().filter(|o| is_series_obj(an, *o)).collect();
    let is_ = iss.first().copied();
    let cs_ = css.first().copied();
    let cats_of = |st: &State, o: u32| st.slot_opt(o, Ns::Meta, mk("cats")).filter(|v| v.pts.is_empty() && v.kv.is_literal()).map(|v| Known::exact(kv_literals(&v.kv)));
    let colvals = match (css.as_slice(), key_is_bool(&c.kw("dropna"), false)) {
        // dropna=False keeps every category of a categorical
        ([one], true) if cats_of(st, *one).is_some() => cats_of(st, *one),
        _ if css.is_empty() => None,
        _ => vals_join(st, &css, vkey()).map(|k| Known { may: k.may.remove(none_key()), sure: k.sure.remove(none_key()) }),
    };
    let mut row = known_or_open(colvals, &Row::new(), c.line, "crosstab");
    if margins {
        row = row.store(mname, true);
    }
    let v = new_frame(an, st, c, row, None, None);
    if let Some(nid) = v.single() {
        let rn = c.kw("rownames").and_then(|v| lit_key_seq(an, st, &v)).and_then(|s0| s0.first().copied());
        let iname = rn.or_else(|| is_.map(|s0| series_name(an, st, s0)).and_then(|n| n.kv.as_lit().filter(|_| n.pts.is_empty())).filter(|k| *k != none_key()));
        if is_.is_some() || rn.is_some() {
            set_index_levels(st, nid, Some(&[iname]));
        } else {
            set_index_levels(st, nid, None);
        }
        let lab = (!iss.is_empty()).then(|| vals_join(st, &iss, vkey())).flatten().map(|mut x| {
            if margins {
                x.may = x.may.insert(mname);
                x.sure = x.sure.insert(mname);
            }
            x
        });
        set_labels(st, nid, lab.as_ref());
        let cn = c.kw("colnames").and_then(|v| lit_key_seq(an, st, &v)).and_then(|s0| s0.first().copied()).or_else(|| cs_.map(|s0| series_name(an, st, s0)).and_then(|n| n.kv.as_lit().filter(|_| n.pts.is_empty())));
        if let Some(k) = cn {
            st.set_slot(nid, Ns::Meta, mk("colname"), Value::lit(k), true);
        }
    }
    v
}

fn wide_to_long(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.args.first().or_else(|| c.kwargs.get(&intern::s("df"))));
    let stubs = keys_arg(an, st, &c.kw("stubnames").or_else(|| c.arg(1, None)));
    let i = keys_arg(an, st, &c.kw("i").or_else(|| c.arg(2, None)));
    let j = lit_key(&c.kw("j").or_else(|| c.arg(3, None)));
    let sep = lit_key(&c.kw("sep")).and_then(intern::key_as_str).map(|s| s.to_string()).unwrap_or_default();
    let suffix = lit_key(&c.kw("suffix")).and_then(intern::key_as_str).map(|s| s.to_string()).unwrap_or_else(|| r"\d+".into());
    let (Some(stubs), Some(i), Some(j)) = (stubs, i, j) else { return open_frame_call(an, st, c) };
    let fs = require_cols(an, st, c, &fs, &KeySet::from_iter(i.iter().copied()));
    let base = rows_of(st, &fs);
    let esc: String = sep.chars().map(|ch| if ch.is_alphanumeric() { ch.to_string() } else { format!("\\{}", ch) }).collect();
    let pats: Vec<super::minire::Regex> = stubs.iter().filter_map(|s0| super::minire::Regex::new(&format!("^{}{}{}$", intern::key_py_str(*s0), esc, suffix))).collect();
    if pats.len() != stubs.len() {
        return open_frame_call(an, st, c);
    }
    let ids = KeySet::from_iter(i.iter().copied());
    let mut out = Row::new();
    for s0 in &stubs {
        out = out.store(*s0, true);
    }
    for k in base.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(base.present.iter())) {
        if ids.contains(k) {
            continue;
        }
        let name = intern::key_py_str(k);
        if pats.iter().any(|p0| p0.search(&name)) {
            out = out.delete(k, true);
            continue;
        }
        out = out.store(k, base.definite.contains(k));
    }
    if base.open() {
        let mut d = out.data();
        d.sources = base.sources.clone();
        out = Row::from_data(d);
    }
    let v = new_frame(an, st, c, out, None, None);
    if let Some(nid) = v.single() {
        let mut lv: Vec<Option<KeyId>> = i.iter().map(|k| Some(*k)).collect();
        lv.push(Some(j));
        set_index_levels(st, nid, Some(&lv));
        set_labels(st, nid, None);
    }
    v
}

/// The flattened keys of one literal record (nested dicts joined with `sep`).
fn flatten_keys(an: &Analyzer, st: &State, d: u32, prefix: &str, sep: &str, level: usize, max_level: Option<usize>, out: &mut Vec<(KeyId, bool)>) -> bool {
    let r = st.row(d, Ns::Item);
    if !r.closed() {
        return false;
    }
    for k in r.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(r.present.iter())) {
        let name = format!("{}{}", prefix, intern::key_py_str(k));
        let v = st.slot(d, Ns::Item, k);
        let nested = v.single().filter(|o| an.kind(*o) == Kind::Dict && !is_series_obj(an, *o));
        match nested {
            Some(o) if max_level.map(|m| level < m).unwrap_or(true) => {
                if !flatten_keys(an, st, o, &format!("{}{}", name, sep), sep, level + 1, max_level, out) {
                    return false;
                }
            }
            _ => out.push((key_str_lit(&name), r.definite.contains(k))),
        }
    }
    true
}

fn json_normalize(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let data = c.arg(0, Some("data")).unwrap_or_default();
    let sep = lit_key(&c.kw("sep")).and_then(intern::key_as_str).map(|s| s.to_string()).unwrap_or_else(|| ".".into());
    let max_level = c.kw("max_level").and_then(|v| v.kv.as_lit()).and_then(intern::key_as_int).map(|n| n as usize);
    let record_path = keys_arg(an, st, &c.kw("record_path"));
    let record_prefix = lit_key(&c.kw("record_prefix")).and_then(intern::key_as_str).map(|s| s.to_string()).unwrap_or_default();
    // the records: a list of dicts (or one dict)
    let recs: Vec<Value> = match data.single() {
        Some(o) if an.kind(o) == Kind::Dict => vec![data.clone()],
        Some(o) if matches!(an.kind(o), Kind::List | Kind::Tuple) => {
            let it = an.iterate(&data, &c.node, st);
            match &it.items {
                Some(items) => items.clone(),
                None => return open_frame_call(an, st, c),
            }
        }
        _ => return open_frame_call(an, st, c),
    };
    let mut rows: Vec<Result<Row, usize>> = Vec::new();
    for r in &recs {
        let Some(d) = r.single().filter(|o| an.kind(*o) == Kind::Dict) else { return open_frame_call(an, st, c) };
        let mut keys: Vec<(KeyId, bool)> = Vec::new();
        match &record_path {
            None => {
                if !flatten_keys(an, st, d, "", &sep, 0, max_level, &mut keys) {
                    return open_frame_call(an, st, c);
                }
                let mut row = Row::new();
                for (k, _) in &keys {
                    row = row.store(*k, true);
                }
                rows.push(Ok(row));
            }
            Some(path) => {
                // the records at the path, each with the meta fields
                let mut cur = Value::obj(d);
                for p0 in path {
                    let Some(o) = cur.single().filter(|o| an.kind(*o) == Kind::Dict) else { return open_frame_call(an, st, c) };
                    cur = st.slot(o, Ns::Item, *p0);
                }
                let it = an.iterate(&cur, &c.node, st);
                let Some(items) = it.items.clone() else { return open_frame_call(an, st, c) };
                let mut meta_cols: Vec<KeyId> = Vec::new();
                if let Some(m) = c.kw("meta") {
                    if let Some(mo) = m.single() {
                        let n = an.llen(mo, st).unwrap_or(0);
                        for i0 in 0..n {
                            let e = st.slot(mo, Ns::Pos, intern::key_int(i0 as i64));
                            match keys_arg(an, st, &Some(e)) {
                                Some(path) => meta_cols.push(key_str_lit(&path.iter().map(|k| intern::key_py_str(*k)).collect::<Vec<_>>().join(&sep))),
                                None => return open_frame_call(an, st, c),
                            }
                        }
                    }
                }
                for item in items {
                    let Some(io) = item.single().filter(|o| an.kind(*o) == Kind::Dict) else { return open_frame_call(an, st, c) };
                    let mut ks: Vec<(KeyId, bool)> = Vec::new();
                    if !flatten_keys(an, st, io, &record_prefix, &sep, 0, max_level, &mut ks) {
                        return open_frame_call(an, st, c);
                    }
                    let mut row = Row::new();
                    for (k, _) in &ks {
                        row = row.store(*k, true);
                    }
                    for k in &meta_cols {
                        row = row.store(*k, true);
                    }
                    rows.push(Ok(row));
                }
            }
        }
    }
    let v = new_frame(an, st, c, rows_frame(&rows), None, None);
    fresh_index(st, &v);
    v
}

fn explode(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    if let Some(col) = c.arg(0, Some("column")) {
        if let Some(ks) = an.lit_keys(&col, st) {
            let fs = require_cols(an, st, c, &fs, &ks);
            let row = rows_of(st, &fs);
            return new_frame(an, st, c, row, None, None);
        }
    }
    let row = recv_row(an, st, c);
    new_frame(an, st, c, row, None, None)
}

fn df_filter(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let items = c.kw("items").or_else(|| c.arg(0, None));
    let row = recv_row(an, st, c);
    let fs = frames(an, c.recv.as_ref());
    if axis_is_index(&c.kw("axis")) {
        return derived(an, st, c, row, &fs, Rows::Subset, true);
    }
    if let Some(items) = items {
        if let Some(ks) = an.lit_keys(&items, st) {
            // listed columns the frame lacks are skipped silently
            let mut d = Row::closed_with(ks.clone()).data();
            for k in ks.iter() {
                if row.closed() && !row.present.contains(k) {
                    d.present = d.present.remove(k);
                }
                if !row.definite.contains(k) {
                    d.definite = d.definite.remove(k);
                    d.likely = d.likely.remove(k);
                }
            }
            if let Some(seq) = lit_key_seq(an, st, &items) {
                d.order = seq.iter().copied().filter(|k| d.present.contains(*k)).collect::<Vec<_>>().into();
            }
            return new_frame(an, st, c, Row::from_data(d), None, None);
        }
    }
    // like= / regex=: the columns whose names match
    let test: Option<Box<dyn Fn(&str) -> bool>> = if let Some(l) = lit_key(&c.kw("like")).and_then(intern::key_as_str) {
        let l = l.to_string();
        Some(Box::new(move |name: &str| name.contains(&l)))
    } else if let Some(r) = lit_key(&c.kw("regex")).and_then(intern::key_as_str).and_then(|p| super::minire::Regex::new(&p)) {
        Some(Box::new(move |name: &str| r.search(name)))
    } else {
        None
    };
    if let Some(t) = test {
        let mut d = row.data();
        for k in row.present.iter() {
            if !t(&intern::key_py_str(k)) {
                d.present = d.present.remove(k);
                d.definite = d.definite.remove(k);
                d.likely = d.likely.remove(k);
                d.absent = d.absent.insert(k);
            }
        }
        d.terms = Default::default();
        d.dterms = Default::default();
        d.lterms = Default::default();
        if row.open() {
            // a matching column the input may have is not known: open, nothing new definite
            d.sources = row.sources.insert((Source::new(&format!("filter@L{}", c.line), SourceKind::Call), EMPTY_LENS));
        }
        return new_frame(an, st, c, Row::from_data(d), None, None);
    }
    let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.insert((Source::new(&format!("filter@L{}", c.line), SourceKind::Call), EMPTY_LENS)), mutators: row.mutators.clone(), ..Default::default() });
    new_frame(an, st, c, r, None, None)
}

fn reindex(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let row = recv_row(an, st, c);
    let cols = if axis_is_columns(&c.kw("axis")) { c.arg(0, Some("labels")) } else { c.kw("columns") };
    let idx = if axis_is_columns(&c.kw("axis")) { c.kw("index") } else { c.kw("index").or_else(|| c.arg(0, Some("labels"))) };
    let mut out = row.clone();
    let mut col_known = true;
    if let Some(cols) = cols {
        if let Some(seq) = lit_key_seq(an, st, &cols) {
            out = closed_ordered(&seq);
        } else if let Some(ks) = an.lit_keys(&cols, st) {
            out = Row::closed_with(ks);
        } else {
            col_known = false;
            out = Row::with_source(Source::new(&format!("reindex@L{}", c.line), SourceKind::Call));
        }
    }
    let v = derived(an, st, c, out, &fs, if idx.is_some() { Rows::Other } else { Rows::Same }, col_known);
    if let (Some(nid), Some(ix)) = (v.single(), idx) {
        let lab = an.lit_keys(&ix, st).map(Known::exact);
        set_labels(st, nid, lab.as_ref());
        set_nrows(st, nid, lit_key_seq(an, st, &ix).map(|s0| s0.len() as i64));
    }
    v
}

fn reindex_like(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let other = c.arg(0, Some("other"));
    let ofs = frames(an, other.as_ref());
    let row = if ofs.is_empty() { Row::with_source(Source::new(&format!("reindex_like@L{}", c.line), SourceKind::Call)) } else { rows_of(st, &ofs) };
    let v = new_frame(an, st, c, row, None, None);
    if let (Some(nid), [o]) = (v.single(), ofs.as_slice()) {
        let lab = labels(st, *o);
        set_labels(st, nid, lab.as_ref());
    }
    v
}

/// Does a dtype match a `select_dtypes` spec ("number", "int64", "float", "bool", "object", …)?
fn dtype_matches(dt: &str, spec: &str) -> Option<bool> {
    Some(match spec {
        "number" | "numeric" => matches!(dt, "int" | "float" | "num"),
        "int" | "int64" | "integer" | "signedinteger" if dt == "num" => return None,
        "float" | "float64" | "floating" if dt == "num" => return None,
        "int" | "int64" | "integer" | "signedinteger" => dt == "int",
        "float" | "float64" | "floating" => dt == "float",
        "bool" | "boolean" => dt == "bool",
        "object" | "O" => dt == "object",
        "datetime" | "datetime64" | "datetime64[ns]" | "category" | "timedelta" => false,
        _ => return None,
    })
}

fn dtype_specs(an: &mut Analyzer, st: &State, v: &Option<Value>) -> Option<Vec<String>> {
    let v = v.as_ref()?;
    let ks = an.lit_keys(v, st)?;
    ks.iter().map(|k| intern::key_as_str(k).map(|s| s.to_string())).collect()
}

fn select_dtypes(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let row = recv_row(an, st, c);
    let inc = c.kw("include").or_else(|| c.arg(0, None));
    let exc = c.kw("exclude").or_else(|| c.arg(1, None));
    let inc_s = if inc.is_some() { dtype_specs(an, st, &inc) } else { Some(vec![]) };
    let exc_s = if exc.is_some() { dtype_specs(an, st, &exc) } else { Some(vec![]) };
    let weak = || Row::from_data(RowData { present: row.present.clone(), sources: row.sources.clone(), mutators: row.mutators.clone(), top: row.top, ..Default::default() });
    let (Some(inc_s), Some(exc_s), Some(&f)) = (inc_s, exc_s, fs.first().filter(|_| fs.len() == 1)) else {
        return new_frame(an, st, c, weak(), None, None);
    };
    // per column: Some(true) kept, Some(false) dropped, None unknown
    let decide = |dt: &str| -> Option<bool> {
        let mut keep = if inc_s.is_empty() { true } else { false };
        for sp in &inc_s {
            if dtype_matches(dt, sp)? {
                keep = true;
            }
        }
        for sp in &exc_s {
            if dtype_matches(dt, sp)? {
                keep = false;
            }
        }
        Some(keep)
    };
    let mut d = row.data();
    for k in row.present.iter() {
        match dtype(st, f, k).and_then(|dt| decide(&dt)) {
            Some(true) => {}
            Some(false) => {
                d.present = d.present.remove(k);
                d.definite = d.definite.remove(k);
                d.likely = d.likely.remove(k);
                d.absent = d.absent.insert(k);
            }
            None => {
                d.definite = d.definite.remove(k);
                d.likely = d.likely.remove(k);
            }
        }
    }
    d.terms = Default::default();
    d.dterms = Default::default();
    d.lterms = Default::default();
    let present = d.present.clone();
    let v = new_frame(an, st, c, Row::from_data(d), None, None);
    if let Some(nid) = v.single() {
        copy_dtypes(st, f, nid, &present);
    }
    v
}

fn sort_values(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let by = c.kw("by").or_else(|| if c.qual.ends_with("nlargest") || c.qual.ends_with("nsmallest") { c.arg(1, Some("columns")) } else { c.arg(0, None) });
    let mut fs = fs;
    if let Some(by) = by {
        if let Some(ks) = an.lit_keys(&by, st) {
            fs = require_cols(an, st, c, &fs, &ks);
        }
    }
    let row = rows_of(st, &fs);
    derived(an, st, c, row, &fs, Rows::Same, true)
}

fn transform(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let row = recv_row(an, st, c);
    new_frame(an, st, c, row, None, None)
}

/// `update(other)`: values change in place, the columns do not (the other's extra columns
/// are ignored).
fn df_update(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let _ = st;
    let _ = c;
    an.none_v.clone()
}

/// `combine_first(other)` / `combine(other, f)`: the union of both frames' columns.
fn df_combine(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let os = frames(an, c.arg(0, Some("other")).as_ref());
    if fs.is_empty() || os.is_empty() {
        return open_frame_call(an, st, c);
    }
    let a = rows_of(st, &fs);
    let b = rows_of(st, &os);
    let mut out = Row::new();
    for k in a.present.union(&b.present).iter() {
        out = out.store(k, a.definite.contains(k) || b.definite.contains(k));
    }
    // a column neither side may have is not there
    for k in a.absent.union(&b.absent).iter() {
        if !side_status(&a, k).0 && !side_status(&b, k).0 {
            out = out.delete(k, true);
        }
    }
    if a.open() || b.open() {
        let mut d = out.data();
        d.sources = a.sources.union(&b.sources);
        out = Row::from_data(d);
    }
    let v = new_frame(an, st, c, out, None, None);
    if let Some(nid) = v.single() {
        let la = levels_of(st, &fs);
        let lb = levels_of(st, &os);
        if la == lb {
            set_index_levels(st, nid, la.as_deref());
        }
    }
    v
}

fn set_axis(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let axis = c.kw("axis");
    let labels = c.arg(0, Some("labels"));
    let row = recv_row(an, st, c);
    if axis_is_columns(&axis) {
        if let Some(l) = labels {
            if let Some(seq) = lit_key_seq(an, st, &l) {
                return new_frame(an, st, c, closed_ordered(&seq), None, None);
            }
            if let Some(ks) = an.lit_keys(&l, st) {
                return new_frame(an, st, c, row.project(&ks), None, None);
            }
            return new_frame(an, st, c, Row::with_source(Source::new(&format!("set_axis@L{}", c.line), SourceKind::Call)), None, None);
        }
    }
    new_frame(an, st, c, row, None, None)
}

fn df_generic(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual).to_string();
    let fs = frames(an, c.recv.as_ref());
    let row = recv_row(an, st, c);
    if NOEFFECT_OPS.contains(&name.as_str()) {
        return noeffect(an, st, c);
    }
    if matches!(name.as_str(), "rolling" | "expanding" | "ewm" | "resample") {
        return window_of(an, st, c, fs.clone(), None, None, &name);
    }
    if name == "value_counts" {
        let subset = keys_arg(an, st, &c.arg(0, Some("subset"))).or_else(|| row.closed().then(|| row.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(row.present.iter()))));
        let nm = if key_is_bool(&c.kw("normalize"), true) { "proportion" } else { "count" };
        let sid = new_series(an, st, &c.node, Row::with_source(Source::new("values", SourceKind::Data)), Some(Value::lit(mk(nm))), "v");
        match subset {
            Some(cols) => {
                let _ = require_cols(an, st, c, &fs, &KeySet::from_iter(cols.iter().copied()));
                let lv: Vec<Option<KeyId>> = cols.into_iter().map(Some).collect();
                set_index_levels(st, sid, Some(&lv));
            }
            None => set_index_levels(st, sid, None),
        }
        return Value::obj(sid);
    }
    if FRAME_REDUCTIONS.contains(&name.as_str()) && !axis_is_columns(&c.kw("axis")) {
        // one value per column: a Series over the columns (numeric_only: the numeric ones)
        let mut idx = row.clone();
        if key_is_bool(&c.kw("numeric_only"), true) {
            if let [f] = fs.as_slice() {
                let mut d = idx.data();
                for k in row.present.iter() {
                    match dtype(st, *f, k).as_deref() {
                        Some("int") | Some("float") | Some("bool") => {}
                        Some(_) => {
                            d.present = d.present.remove(k);
                            d.definite = d.definite.remove(k);
                            d.likely = d.likely.remove(k);
                        }
                        None => {
                            d.definite = d.definite.remove(k);
                            d.likely = d.likely.remove(k);
                        }
                    }
                }
                idx = Row::from_data(d);
            }
        }
        let mut d = idx.data();
        d.terms = Default::default();
        d.dterms = Default::default();
        d.lterms = Default::default();
        let sid = new_series(an, st, &c.node, Row::from_data(d), None, "r");
        return Value::obj(sid);
    }
    if SERIES_RESULT_OPS.contains(&name.as_str()) {
        return series(an);
    }
    if name == "dropna" && axis_is_columns(&c.kw("axis")) {
        // the columns holding a NaN go: which ones depends on the data
        let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.clone(), mutators: row.mutators.clone(), top: row.top, ..Default::default() });
        return new_frame(an, st, c, r, None, None);
    }
    if COPY_FRAME_OPS.contains(&name.as_str()) {
        let (rows, with_vals) = match name.as_str() {
            "copy" | "sort_index" | "infer_objects" | "swaplevel" | "reorder_levels" => (Rows::Same, true),
            "head" | "tail" | "sample" | "drop_duplicates" | "dropna" | "query" | "take" | "truncate" | "first" | "last" | "between_time" | "at_time" | "nlargest" | "nsmallest" => (Rows::Subset, true),
            "asfreq" | "align" | "reindex_like" | "combine_first" => (Rows::Other, false),
            _ => (Rows::Same, false),
        };
        let v = derived(an, st, c, row.clone(), &fs, rows, with_vals);
        if let Some(nid) = v.single() {
            match name.as_str() {
                // every column becomes bool
                "isna" | "isnull" | "notna" | "notnull" | "eq" | "ne" | "lt" | "le" | "gt" | "ge" | "isin" => {
                    for k in row.present.iter() {
                        set_dtype(st, nid, k, Some("bool"), true);
                    }
                }
                // astype(t) / astype({col: t})
                "astype" => {
                    if let [f] = fs.as_slice() {
                        copy_dtypes(st, *f, nid, &row.present);
                    }
                    let arg = c.arg(0, Some("dtype"));
                    let pairs: Option<Vec<(KeyId, Option<&'static str>)>> = arg.as_ref().and_then(|a| a.single()).filter(|o| an.kind(*o) == Kind::Dict).map(|o| {
                        let r = st.row(o, Ns::Item);
                        r.present.iter().map(|k| (k, astype_dtype(an, &Some(st.slot(o, Ns::Item, k))))).collect()
                    });
                    match pairs {
                        Some(ps) => {
                            for (k, d) in ps {
                                set_dtype(st, nid, k, d, true);
                            }
                        }
                        None => {
                            let d = astype_dtype(an, &arg);
                            for k in row.present.iter() {
                                set_dtype(st, nid, k, d, true);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if name == "fillna" {
            // (filling keeps a column's kind)
            if let (Some(nid), [f]) = (v.single(), fs.as_slice()) {
                copy_dtypes(st, *f, nid, &row.present);
            }
        }
        if matches!(name.as_str(), "head" | "tail" | "nlargest" | "nsmallest") {
            let n = c.arg(0, Some("n")).and_then(|x| x.kv.as_lit()).and_then(intern::key_as_int).or(if c.args.is_empty() && c.kw("n").is_none() { Some(5) } else { None });
            if let (Some(nid), Some(n)) = (v.single(), n) {
                if n >= 0 {
                    st.set_slot(nid, Ns::Meta, mk("maxrows"), Value::lit(intern::key_int(n)), true);
                    if let Some(total) = fs.first().filter(|_| fs.len() == 1).and_then(|f| nrows(st, *f)) {
                        set_nrows(st, nid, Some(total.min(n)));
                    }
                }
            }
        }
        return v;
    }
    if OPEN_FRAME_OPS.contains(&name.as_str()) {
        return match fs.first() {
            Some(f) => open_frame_from(an, *f, &c.node, st, &name),
            None => Value::data(&name),
        };
    }
    if name == "get" {
        let default = c.arg(1, Some("default")).unwrap_or_else(|| an.none_v.clone());
        if let Some(a) = c.args.first().or(c.kwargs.get(&intern::s("key"))) {
            if let (Some(k), true) = (a.kv.as_lit(), a.pts.is_empty()) {
                // the column when there, else the default (never raises)
                let mut out = Value::new();
                for f in &fs {
                    let tier = classify(&st.row(*f, Ns::Item), k).tier;
                    if tier != Tier::Missing {
                        out = out.join(&column_series(an, st, *f, k, &c.node));
                    }
                    if tier != Tier::Proven {
                        out = out.join(&default);
                    }
                }
                return out;
            }
            if let Some(seq) = lit_key_seq(an, st, a) {
                // a list: those columns (a KeyError for a missing one is turned into None)
                let ks = KeySet::from_iter(seq.iter().copied());
                let mut out = Value::new();
                for f in &fs {
                    let r = st.row(*f, Ns::Item);
                    if ks.iter().any(|k| classify(&r, k).tier != Tier::Proven) {
                        out = out.join(&default);
                    }
                    if ks.iter().all(|k| classify(&r, k).tier != Tier::Missing) {
                        let v = derived(an, st, c, closed_ordered(&seq), &[*f], Rows::Same, true);
                        out = out.join(&v);
                    }
                }
                return out;
            }
        }
        return series(an).join(&default);
    }
    if name == "squeeze" {
        let axis = c.kw("axis").or_else(|| c.arg(0, None));
        let rows_one = fs.len() == 1 && (nrows(st, fs[0]) == Some(1) || st.slot_opt(fs[0], Ns::Meta, mk("maxrows")).and_then(|v| v.kv.as_lit()).and_then(intern::key_as_int).map(|m| m <= 1).unwrap_or(false));
        let one_col = row.closed() && row.present.len() == 1 && row.present == row.definite;
        if axis_is_index(&axis) {
            if rows_one {
                // a one-row frame: that row (a Series over the columns)
                let f = fs[0];
                let sid = new_series(an, st, &c.node, row.clone(), None, "q");
                mark_view(an, sid, &[f]);
                return Value::obj(sid);
            }
            return derived(an, st, c, row, &fs, Rows::Same, true);
        }
        if axis_is_columns(&axis) || axis.is_none() {
            if one_col {
                let k = row.present.first().unwrap();
                if axis.is_none() && rows_one {
                    // 1 x 1: the cell
                    return match vals(st, fs[0], k) {
                        Some(v) if v.may.len() == 1 => Value::lit(v.may.first().unwrap()),
                        _ => Value::data(&format!("cell@L{}", c.line)),
                    };
                }
                return match fs.as_slice() {
                    [f] => column_series(an, st, *f, k, &c.node),
                    _ => series(an),
                };
            }
            if axis.is_none() && rows_one && fs.len() == 1 {
                let sid = new_series(an, st, &c.node, row.clone(), None, "q");
                return Value::obj(sid);
            }
            return derived(an, st, c, row, &fs, Rows::Same, true);
        }
    }
    if name == "items" || name == "keys" {
        let kind = if name == "items" { ViewKind::Items } else { ViewKind::Keys };
        let ids: Vec<String> = fs.iter().map(|f| f.to_string()).collect();
        let recv = c.recv.clone().unwrap_or_default();
        let vid = an.alloc_raw(Kind::View, &format!("view:{}:{}", name, ids.join(",")), "", true, &name, Payload::View(kind, recv.clone()));
        an.obj_mut(vid).payload = Payload::View(kind, recv);
        return Value::obj(vid);
    }
    if matches!(name.as_str(), "xs") {
        return new_frame(an, st, c, row, None, None);
    }
    an.note(format!("unmodelled DataFrame.{} at L{}: result treated as an open frame derived from the receiver", name, c.line));
    let r = Row::from_data(RowData {
        present: row.present.clone(),
        likely: row.definite.clone(),
        sources: row.sources.insert((Source::new(&format!("DataFrame.{}@L{}", name, c.line), SourceKind::Call), EMPTY_LENS)),
        mutators: row.mutators.clone(),
        ..Default::default()
    });
    let v = new_frame(an, st, c, r, None, None);
    if let Some(nid) = v.single() {
        // what the index became is not known either
        set_index_levels(st, nid, None);
    }
    v
}

// ------------------------------------------------------------- frame hooks used by the interpreter
pub fn frame_getattr(an: &mut Analyzer, oid: u32, name: &str, node: &Node, state: &mut State, guarded: bool) -> Value {
    if matches!(name, "loc" | "iloc" | "at" | "iat") {
        let label = format!("{} of {}", name, an.obj(oid).label);
        let lid = an.alloc_raw(Kind::Locidx, &format!("loc:{}:{}", name, oid), "", true, &label, Payload::Locidx(name.to_string(), oid));
        an.obj_mut(lid).payload = Payload::Locidx(name.to_string(), oid);
        return Value::obj(lid);
    }
    if name == "columns" {
        let vid = an.alloc_raw(Kind::View, &format!("view:keys:{}", oid), "", true, "columns", Payload::View(ViewKind::Keys, Value::obj(oid)));
        an.obj_mut(vid).payload = Payload::View(ViewKind::Keys, Value::obj(oid));
        return Value::obj(vid);
    }
    if matches!(name, "str" | "dt" | "cat") {
        return series(an);
    }
    if DF_ATTRS.contains(&name) || DF_METHODS.contains(&name) {
        if DF_ATTRS.contains(&name) && name != "columns" {
            if name == "T" {
                return transpose_of(an, state, node, &[oid]);
            }
            if name == "dtypes" {
                let row = state.row(oid, Ns::Item);
                let mut d = row.data();
                d.terms = Default::default();
                d.dterms = Default::default();
                d.lterms = Default::default();
                let sid = new_series(an, state, node, Row::from_data(d), None, "dt");
                return Value::obj(sid);
            }
            if name == "shape" {
                // (rows, columns): the column count when the columns are known
                let row = state.row(oid, Ns::Item);
                let tid = an.alloc_at(Kind::Tuple, node, state, "tuple", Payload::None, None, Ns::Item, None, None);
                an.set_llen(tid, state, Some(2));
                let n = match nrows(state, oid) {
                    Some(n) => Value::lit(intern::key_int(n)),
                    None => Value::data("rows"),
                };
                let ncols = if row.closed() && !row.symbolic() && row.present.len() - row.definite.len() <= 8 {
                    Value::kv_only(KeyVal::lits(KeySet::from_iter((row.definite.len()..=row.present.len()).map(|x| intern::key_int(x as i64)))))
                } else {
                    Value::data("columns")
                };
                state.set_slot(tid, Ns::Pos, intern::key_int(0), n, true);
                state.set_slot(tid, Ns::Pos, intern::key_int(1), ncols, true);
                return Value::obj(tid);
            }
            return Value::data(name);
        }
        return an.method_value(&format!("DataFrame.{}", name), &Value::obj(oid));
    }
    frame_getitem(an, oid, &Value::lit_str(name), node, state, guarded)
}

/// `df.columns.map(f)`: the names as a renaming of the frame's keys (None: not a frame's).
pub fn columns_map(an: &mut Analyzer, st: &mut State, c: &Call) -> Option<Value> {
    let vid = c.recv.as_ref()?.single()?;
    let Payload::View(ViewKind::Keys, recv) = an.obj(vid).payload.clone() else { return None };
    let fs: Vec<u32> = recv.pts.iter().filter(|o| an.kind(*o) == Kind::Frame).collect();
    if fs.is_empty() {
        return None;
    }
    let f = c.arg(0, None)?;
    match rename_step(an, st, &f, c, &fs) {
        Some(steps) => {
            let lens = intern::lens(crate::domain::Lens { steps });
            let mut ko = KeysOf::new(ObjSet::from_vec(fs), Ns::Item);
            ko.chain = lens;
            Some(Value::kv_only(KeyVal::KeysOf(ko)))
        }
        None => Some(Value::data(&format!("map@L{}", c.line))),
    }
}

/// `df.columns.name = v` / `df.index.name = v` (an attribute store on a frame's view).
pub fn view_setattr(an: &mut Analyzer, vid: u32, name: &str, v: &Value, state: &mut State) {
    let Payload::View(ViewKind::Keys, recv) = an.obj(vid).payload.clone() else { return };
    if name != "name" && name != "names" {
        return;
    }
    let is_index = an.obj(vid).label == "index";
    for f in recv.pts.iter() {
        if an.kind(f) == Kind::Frame {
            if is_index {
                let k = v.kv.as_lit().filter(|_| v.pts.is_empty()).map(|k| if k == none_key() { None } else { Some(k) });
                set_index_levels(state, f, k.map(|k| vec![k]).as_deref());
            } else {
                state.set_slot(f, Ns::Meta, mk("colname"), v.clone(), true);
            }
        }
    }
}

pub fn frame_setattr(an: &mut Analyzer, oid: u32, name: &str, v: &Value, node: &Node, state: &mut State, strong: bool) {
    if name == "columns" {
        let row = state.row(oid, Ns::Item);
        if let Some(seq) = lit_key_seq(an, state, v) {
            state.set_row(oid, Ns::Item, closed_ordered(&seq));
            return;
        }
        if let Some(ks) = an.lit_keys(v, state) {
            state.set_row(oid, Ns::Item, row.project(&ks));
            return;
        }
        let mut kv: Option<KeyVal> = if v.pts.is_empty() { Some(v.kv.clone()) } else { None };
        for x in v.pts.iter() {
            match an.kind(x) {
                Kind::List => {
                    let e = state.elems.get(&x).unwrap_or(DATA);
                    kv = Some(match kv {
                        None => e,
                        Some(k) => kv_join(&k, &e),
                    });
                }
                Kind::View => {
                    if let Payload::View(_, recv) = &an.obj(x).payload {
                        let e = KeyVal::KeysOf(KeysOf::new(recv.pts.clone(), Ns::Item));
                        kv = Some(match kv {
                            None => e,
                            Some(k) => kv_join(&k, &e),
                        });
                    }
                }
                _ => {}
            }
        }
        let kv = kv.unwrap_or(DATA);
        if let KeyVal::KeysOf(ko) = &kv {
            if ko.objs == ObjSet::one(oid) && ko.exclude.is_empty() {
                let mut r = row;
                for step in intern::lens_get(ko.chain).steps {
                    r = r.rename(&step);
                }
                state.set_row(oid, Ns::Item, r);
                return;
            }
        }
        state.set_row(oid, Ns::Item, Row::with_source(Source::new(&format!("columns=@L{}", node.line), SourceKind::Call)));
        an.note(format!("df.columns assigned from an unknown value at L{}", node.line));
        return;
    }
    // `df.x = v` sets column x when the frame has it, else an instance attribute (a pandas
    // UserWarning): the columns do not change either way
    let _ = (v, strong);
}

/// The top level of MultiIndex columns (tuple keys) as a row.
fn top_level(row: &Row) -> Row {
    let first = |x: KeyId| intern::with_key(x, |kk| if let intern::Key::Tuple(t) = kk { t.first().copied() } else { Some(x) });
    let mut d = row.data();
    d.present = KeySet::from_iter(row.present.iter().filter_map(first));
    d.definite = KeySet::from_iter(row.definite.iter().filter_map(first));
    d.likely = KeySet::from_iter(row.likely.iter().filter_map(first));
    d.absent = KeySet::new();
    d.order = Default::default();
    Row::from_data(d)
}

fn is_tuple_key(x: KeyId) -> bool {
    intern::with_key(x, |kk| matches!(kk, intern::Key::Tuple(_)))
}

/// How a column access `df[k]` classifies (a plain key on MultiIndex columns: the top level).
pub fn column_tier(row: &Row, k: KeyId) -> Tier {
    if !is_tuple_key(k) && row.present.iter().any(is_tuple_key) {
        return classify(&top_level(row), k).tier;
    }
    classify(row, k).tier
}

pub fn frame_getitem(an: &mut Analyzer, oid: u32, key: &Value, node: &Node, state: &mut State, guarded: bool) -> Value {
    frame_getitem_n(an, oid, key, node, state, guarded, true)
}

/// `narrow`: a failing access raises here (false for loads that never fail).
pub fn frame_getitem_n(an: &mut Analyzer, oid: u32, key: &Value, node: &Node, state: &mut State, guarded: bool, narrow: bool) -> Value {
    let row = state.row(oid, Ns::Item);
    let site = an.access_site(node);
    if let (Some(k), true) = (key.kv.as_lit(), key.pts.is_empty()) {
        // MultiIndex columns (tuple keys): a plain key selects the top level
        if !is_tuple_key(k) && row.present.iter().any(is_tuple_key) {
            let top = top_level(&row);
            let cls = classify(&top, k);
            if guarded && !crate::interp::disabled("guardedverdicts") {
                an.verdict_guarded(node, k, &cls, oid, Ns::Item);
            }
            if !guarded {
                an.verdict(node, k, &cls, oid, Ns::Item, None, None);
            }
            // the sub-frame: the second level of the matching columns
            let second = |x: KeyId| intern::with_key(x, |kk| if let intern::Key::Tuple(t) = kk { if t.first() == Some(&k) { if t.len() == 2 { t.get(1).copied() } else { Some(intern::key_tuple(t[1..].to_vec())) } } else { None } } else { None });
            let mut sd = Row::new().data();
            sd.present = KeySet::from_iter(row.present.iter().filter_map(second));
            sd.definite = KeySet::from_iter(row.definite.iter().filter_map(second));
            sd.likely = sd.definite.clone();
            if row.open() {
                sd.sources = row.sources.clone();
            }
            let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(Row::from_data(sd)), Ns::Item, None, None);
            derive(state, oid, nid, Rows::Same, &KeySet::new());
            return Value::obj(nid);
        }
        let cls = classify(&row, k);
        if guarded && !crate::interp::disabled("guardedverdicts") {
            an.verdict_guarded(node, k, &cls, oid, Ns::Item);
        }
        if !guarded {
            an.verdict(node, k, &cls, oid, Ns::Item, None, None);
            if cls.tier == Tier::Input {
                an.require_providers_k(&cls, &row, &site, true, state, Some(k));
            }
        }
        // the program went on: the column was there (also under a try that handles the miss)
        if narrow && cls.tier != Tier::Missing && an.obj(oid).single {
            state.set_row(oid, Ns::Item, row.narrow_in(k, true));
        }
        return column_series(an, state, oid, k, node);
    }
    if is_seriesish(an, key) || key.pts.iter().any(|x| an.kind(x) == Kind::Frame) {
        // a boolean mask: some of the rows
        let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(row.clone()), Ns::Item, None, None);
        derive(state, oid, nid, Rows::Subset, &row.present);
        return Value::obj(nid);
    }
    let ks = if !key.pts.is_empty() {
        an.lit_keys(key, state)
    } else if let KeyVal::Lits(l) = &key.kv {
        Some(l.clone())
    } else {
        None
    };
    if let Some(ks) = ks {
        if !key.pts.is_empty() || matches!(key.kv, KeyVal::Lits(_)) {
            let sure_keys = list_keys_split(an, state, key).map(|(_, s0)| s0).unwrap_or_else(|| ks.clone());
            for k in intern::sorted_by_str(ks.iter()) {
                if sure_keys.contains(k) {
                    frame_getitem_n(an, oid, &Value::lit(k), node, state, guarded, narrow);
                    continue;
                }
                // in the list on some paths only: read (and needed of an input) on those only
                let before: Vec<(crate::domain::SourceId, bool)> = classify(&state.row(oid, Ns::Item), k).providers.iter().map(|(src, _)| (*src, state.reqs.inner(src).map(|d| d.contains_key(&k)).unwrap_or(false))).collect();
                frame_getitem_n(an, oid, &Value::lit(k), node, state, guarded, false);
                for (src, had) in before {
                    if !had {
                        let tgt = state.reqs.writable(&src);
                        if let Some(cur) = tgt.get(&k).cloned() {
                            tgt.insert(k, crate::domain::ReqInfo { always: false, ..cur });
                        }
                    }
                }
            }
            let mut r = match lit_key_seq(an, state, key) {
                Some(seq) => closed_ordered(&seq),
                None => Row::closed_with(ks.clone()),
            };
            // a column the list holds on some paths only is selected on those only
            if let Some((_, sure)) = list_keys_split(an, state, key) {
                let mut d = r.data();
                d.definite = d.definite.intersection(&sure.union(&KeySet::from_iter(lit_key_seq(an, state, key).unwrap_or_default())));
                d.likely = d.definite.clone();
                r = Row::from_data(d);
            }
            let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(r), Ns::Item, None, None);
            derive(state, oid, nid, Rows::Same, &ks);
            return Value::obj(nid);
        }
    }
    if is_seriesish(an, key) || key.pts.iter().any(|x| an.kind(x) == Kind::Frame) {
        let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(row), Ns::Item, None, None);
        copy_meta(oid, nid, state);
        return Value::obj(nid);
    }
    if let KeyVal::KeysOf(_) = &key.kv {
        // one of the column names: that column (a Series named by the key)
        if key.pts.is_empty() {
            let idx = labels_row(state, oid);
            let sid = new_series(an, state, node, idx, Some(Value::kv_only(key.kv.clone())), "ck");
            if let Some(m) = state.slot_opt(oid, Ns::Meta, mk("index")) {
                state.set_slot(sid, Ns::Meta, mk("index"), m, true);
            }
            return Value::obj(sid);
        }
        let r = Row::new().copy_from(&row, false, &KeySet::new(), EMPTY_LENS);
        let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(r), Ns::Item, None, None);
        return Value::obj(nid);
    }
    // `df[list(df.columns)]` / `df[df.columns]`: every column of the frame itself
    let own_keys = key.pts.iter().all(|x| match an.kind(x) {
        Kind::List | Kind::Tuple => matches!(state.elems.get(&x), Some(KeyVal::KeysOf(ko)) if ko.objs == ObjSet::one(oid) && ko.exclude.is_empty() && ko.chain == EMPTY_LENS),
        Kind::View => matches!(&an.obj(x).payload, Payload::View(ViewKind::Keys, r) if r.pts == ObjSet::one(oid)),
        _ => false,
    });
    if own_keys && !key.pts.is_empty() && !crate::interp::disabled("ownkeys") {
        let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(row.clone()), Ns::Item, None, None);
        derive(state, oid, nid, Rows::Same, &row.present);
        return Value::obj(nid);
    }
    if key.pts.iter().any(|x| matches!(an.kind(x), Kind::List | Kind::View)) {
        let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.insert((Source::new(&format!("columns@L{}", node.line), SourceKind::Call), EMPTY_LENS)), mutators: row.mutators.clone(), ..Default::default() });
        let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(r), Ns::Item, None, None);
        return Value::obj(nid);
    }
    series(an)
}

pub fn frame_setitem(an: &mut Analyzer, oid: u32, key: &Value, v: &Value, node: &Node, state: &mut State, strong: bool) {
    let row = state.row(oid, Ns::Item);
    let ks = if !key.pts.is_empty() { an.lit_keys(key, state) } else { None };
    if let (Some(k), true) = (key.kv.as_lit(), key.pts.is_empty()) {
        state.set_row(oid, Ns::Item, row.store(k, strong));
        let dt = dtype_of_value(an, state, v);
        set_dtype(state, oid, k, dt, strong);
        // the column's values: a tracked Series' (a scalar: itself)
        let kv = match v.single().filter(|o| is_series_obj(an, *o)) {
            Some(o) => vals(state, o, vkey()),
            None => cell_lit(v).map(|l| Known::exact(KeySet::one(l))),
        };
        if strong {
            set_vals(state, oid, k, kv.as_ref());
        } else {
            set_vals(state, oid, k, None);
        }
        return;
    }
    let ks2 = match (&ks, &key.kv) {
        (Some(k), _) => Some(k.clone()),
        (None, KeyVal::Lits(l)) => Some(l.clone()),
        _ => None,
    };
    if let Some(ks) = ks2 {
        let mut r = row;
        for k in ks.iter() {
            r = r.store(k, strong);
        }
        state.set_row(oid, Ns::Item, r);
        return;
    }
    if let KeyVal::KeysOf(ko) = &key.kv {
        let srcrow = join_rows(ko.objs.iter().map(|x| state.row(x, ko.ns)));
        state.set_row(oid, Ns::Item, row.copy_from(&srcrow, false, &ko.exclude, ko.chain));
        return;
    }
    if is_seriesish(an, key) {
        return;
    }
    state.set_row(oid, Ns::Item, row.data_store(Source::new(&format!("store@L{}", node.line), SourceKind::Data)));
}

pub fn frame_delitem(an: &mut Analyzer, oid: u32, key: &Value, node: &Node, state: &mut State, strong: bool, report: bool) {
    let mut row = state.row(oid, Ns::Item);
    let is_lit = matches!(key.kv, KeyVal::Lit(_));
    for k in intern::sorted_by_str(kv_literals(&key.kv).iter()) {
        if report {
            let cls = classify(&row, k);
            an.verdict(node, k, &cls, oid, Ns::Item, None, None);
            if cls.tier == Tier::Input {
                let site = an.access_site(node);
                an.require_providers(&cls, &row, &site, false, state);
            }
        }
        row = row.delete(k, strong && is_lit);
    }
    state.set_row(oid, Ns::Item, row);
}

/// `df.loc[label]`: a KeyError when the label is surely not there (labels known).
fn label_verdict(an: &mut Analyzer, f: u32, k: KeyId, node: &Node, st: &mut State) {
    let Some(lab) = labels(st, f) else { return };
    let cls = classify(&lab.row(), k);
    if an.is_guarded(Ns::Item) {
        an.verdict_guarded(node, k, &cls, f, Ns::Pos);
    } else {
        an.verdict(node, k, &cls, f, Ns::Pos, None, None);
    }
}

/// A frame's row as a Series (`df.iloc[0]`, `df.loc[label]`): its index is the columns.
fn row_series(an: &mut Analyzer, foid: u32, node: &Node, state: &mut State) -> Value {
    let base = state.row(foid, Ns::Item);
    let vrow = an.alloc_at(Kind::Dict, node, state, &format!("row@L{}", node.line), Payload::None, Some(base.clone()), Ns::Item, Some(false), None);
    mark_view(an, vrow, &[foid]);
    for k in base.present.iter() {
        state.set_slot(vrow, Ns::Item, k, Value::data(&format!("cell {}", intern::key_py_str(k))), false);
    }
    Value::obj(vrow)
}

fn frame_with(an: &mut Analyzer, foid: u32, node: &Node, state: &mut State, row: Row) -> Value {
    let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(row), Ns::Item, None, None);
    copy_meta(foid, nid, state);
    Value::obj(nid)
}

pub fn loc_get(an: &mut Analyzer, lo: u32, key: &Value, node: &Node, state: &mut State) -> Value {
    let Payload::Locidx(name, foid) = an.obj(lo).payload.clone() else { return Value::new() };
    if name.starts_with('s') && is_series_obj(an, foid) {
        // an element of a tracked Series (a dict per row stays one)
        if key.pts.is_empty() && key.kv.as_lit().is_some() {
            return state.slot_opt(foid, Ns::Meta, mk("elem")).unwrap_or_else(|| Value::data(&format!("cell@L{}", node.line)));
        }
        let idx = state.row(foid, Ns::Item);
        let mut d = idx.data();
        d.definite = KeySet::new();
        d.likely = KeySet::new();
        let sid = new_series(an, state, node, Row::from_data(d), Some(series_name(an, state, foid)), "l");
        return Value::obj(sid);
    }
    let row = state.row(foid, Ns::Item);
    let positional = name == "iloc" || name == "iat";
    let mut parts: Option<(Value, Value)> = None;
    for x in key.pts.iter() {
        if an.kind(x) == Kind::Tuple && an.llen(x, state) == Some(2) {
            parts = Some((state.slot(x, Ns::Pos, intern::key_int(0)), state.slot(x, Ns::Pos, intern::key_int(1))));
        }
    }
    if parts.is_none() {
        if let Some(k) = key.kv.as_lit().filter(|_| key.pts.is_empty()) {
            // a literal tuple key `(rows, "col")` folded into one literal
            if let Some(items) = intern::with_key(k, |kk| if let intern::Key::Tuple(t) = kk { Some(t.clone()) } else { None }) {
                if items.len() == 2 {
                    parts = Some((Value::lit(items[0]), Value::lit(items[1])));
                }
            }
        }
    }
    let Some((rows, cols)) = parts else {
        // one key: rows. A scalar label / position is one row (a Series over the columns)
        if key.pts.is_empty() && key.kv.as_lit().is_some() && (positional || name == "loc" || name == "at") {
            if name == "loc" {
                label_verdict(an, foid, key.kv.as_lit().unwrap(), node, state);
            }
            return row_series(an, foid, node, state);
        }
        return frame_with(an, foid, node, state, row);
    };
    if !positional && rows.pts.is_empty() {
        if let Some(k) = rows.kv.as_lit() {
            label_verdict(an, foid, k, node, state);
        }
    }
    if is_seriesish(an, &cols) {
        // a boolean mask over the columns: some of them
        return frame_with(an, foid, node, state, select_positions(&row, &|_| None));
    }
    let one_row = rows.pts.is_empty() && rows.kv.as_lit().is_some();
    if positional {
        if let Some(i) = cols.kv.as_lit().filter(|_| cols.pts.is_empty()).and_then(intern::key_as_int) {
            if one_row {
                return Value::data(&format!("cell@L{}", node.line));
            }
            let _ = i;
            return series(an);
        }
        let picked = if is_slice(&cols) {
            let b = slice_bounds(&cols);
            select_positions(&row, &|n| b.and_then(|b| slice_positions(n, b)))
        } else if let Some(seq) = lit_key_seq(an, state, &cols) {
            let idx: Option<Vec<i64>> = seq.iter().map(|k| intern::key_as_int(*k)).collect();
            select_positions(&row, &|n| idx.as_ref().and_then(|ix| ix.iter().map(|i| { let j = if *i < 0 { i + n as i64 } else { *i }; (0..n as i64).contains(&j).then_some(j as usize) }).collect()))
        } else {
            select_positions(&row, &|_| None)
        };
        if one_row {
            return series(an);
        }
        return frame_with(an, foid, node, state, picked);
    }
    if cols.kv.as_lit().is_some() && cols.pts.is_empty() {
        let g = an.is_guarded(Ns::Item);
        return frame_getitem(an, foid, &cols, node, state, g);
    }
    if let Some(seq) = lit_key_seq(an, state, &cols) {
        let g = an.is_guarded(Ns::Item);
        for k in intern::sorted_by_str(seq.iter().copied()) {
            frame_getitem(an, foid, &Value::lit(k), node, state, g);
        }
        if one_row {
            return series(an);
        }
        return frame_with(an, foid, node, state, closed_ordered(&seq));
    }
    if let Some(ks) = an.lit_keys(&cols, state) {
        let g = an.is_guarded(Ns::Item);
        return frame_getitem(an, foid, &Value::with_kv(cols.pts.clone(), KeyVal::Lits(ks)), node, state, g);
    }
    if is_slice(&cols) {
        if slice_bounds(&cols) == Some((None, None, None)) {
            return if one_row { row_series(an, foid, node, state) } else { frame_with(an, foid, node, state, row) };
        }
        // a label slice: some of the columns
        let r = select_positions(&row, &|_| None);
        return frame_with(an, foid, node, state, r);
    }
    series(an)
}

pub fn loc_set(an: &mut Analyzer, lo: u32, key: &Value, v: &Value, node: &Node, state: &mut State) {
    let Payload::Locidx(name, foid) = an.obj(lo).payload.clone() else { return };
    for x in key.pts.iter() {
        if an.kind(x) == Kind::Tuple && an.llen(x, state) == Some(2) {
            let cols = state.slot(x, Ns::Pos, intern::key_int(1));
            if name == "iloc" || name == "iat" {
                return;
            }
            let single = an.obj(foid).single;
            frame_setitem(an, foid, &cols, v, node, state, single);
            return;
        }
    }
}
