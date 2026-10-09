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
    "merge", "join", "rename", "drop", "assign", "copy", "head", "tail", "sample", "sort_values", "sort_index", "drop_duplicates", "fillna",
    "dropna", "astype", "reset_index", "set_index", "groupby", "apply", "applymap", "map", "pipe", "iterrows", "itertuples", "to_dict", "to_csv",
    "to_parquet", "to_excel", "to_json", "to_sql", "to_numpy", "to_records", "to_string", "to_html", "to_markdown", "query", "eval", "where",
    "mask", "pivot", "pivot_table", "melt", "stack", "unstack", "transpose", "explode", "describe", "corr", "cov", "agg", "aggregate", "filter",
    "select_dtypes", "insert", "pop", "nlargest", "nsmallest", "isna", "isnull", "notna", "notnull", "round", "abs", "clip", "replace", "shift",
    "diff", "cumsum", "cumprod", "cummax", "cummin", "rank", "pct_change", "rolling", "expanding", "ewm", "resample", "mean", "sum", "min", "max",
    "count", "median", "std", "var", "nunique", "idxmax", "idxmin", "any", "all", "value_counts", "memory_usage", "info", "equals", "eq", "ne",
    "lt", "le", "gt", "ge", "add", "sub", "mul", "div", "truediv", "floordiv", "mod", "pow", "dot", "squeeze", "transform", "reindex",
    "reindex_like", "set_axis", "add_prefix", "add_suffix", "align", "combine_first", "update", "interpolate", "bfill", "ffill", "pad", "backfill",
    "convert_dtypes", "infer_objects", "iloc", "loc", "at", "iat", "xs", "get", "items", "keys", "lookup", "first", "last", "nth", "take",
    "truncate", "between_time", "at_time", "asfreq", "tz_localize", "tz_convert", "plot", "hist", "boxplot", "style", "compare", "duplicated",
    "isin", "swaplevel", "droplevel", "reorder_levels", "sort", "sem", "skew", "kurt", "prod", "quantile", "mode", "cumcount", "ngroup",
    "corrwith", "to_period", "to_timestamp", "columns", "index", "shape", "values", "dtypes", "T", "empty", "size", "ndim", "axes",
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
const SERIES_RESULT_OPS: &[&str] = &[
    "mean", "sum", "min", "max", "count", "median", "std", "var", "nunique", "idxmax", "idxmin", "any", "all", "value_counts", "memory_usage", "sem",
    "skew", "kurt", "prod", "quantile", "mode", "squeeze", "duplicated", "dot", "corrwith", "size", "cumcount", "ngroup",
];
const NOEFFECT_OPS: &[&str] = &[
    "to_csv", "to_parquet", "to_excel", "to_json", "to_sql", "to_numpy", "to_records", "to_string", "to_html", "to_markdown", "info", "equals", "plot",
    "hist", "boxplot", "to_pickle", "to_feather", "to_clipboard", "to_latex", "to_stata", "to_hdf", "to_xml", "to_orc",
];

pub fn install(r: &mut dyn FnMut(&[&'static str], Model)) {
    r(FRAME_PRODUCERS, read_frame);
    r(&["pandas.DataFrame", "pandas.DataFrame.from_records", "pandas.DataFrame.from_dict"], pd_dataframe);
    r(&["pandas.merge"], pd_merge);
    r(&["pandas.concat"], concat);
    r(&["pandas.json_normalize", "pandas.get_dummies", "pandas.crosstab", "pandas.pivot_table", "pandas.wide_to_long", "pandas.read_html_table"], open_frame_call);
    r(
        &[
            "pandas.Series", "pandas.to_datetime", "pandas.to_numeric", "pandas.isna", "pandas.notna", "pandas.isnull", "pandas.notnull", "pandas.cut",
            "pandas.qcut", "pandas.date_range", "pandas.Timestamp", "pandas.Timedelta", "pandas.to_timedelta", "pandas.factorize", "pandas.unique",
            "pandas.array", "pandas.Categorical", "pandas.Index", "pandas.MultiIndex", "pandas.NamedAgg", "pandas.Grouper", "pandas.set_option",
            "pandas.option_context", "pandas.testing.assert_frame_equal", "pandas.ExcelWriter", "pandas.eval",
        ],
        series_call,
    );
    r(&["Series.*"], series_method);
    r(&["Series.str.*", "Series.dt.*", "Series.cat.*", "Series.sparse.*", "Series.list.*", "Series.struct.*"], accessor_method);
    r(&["DataFrame.*"], df_dispatch);
    r(&["groupby.*"], groupby_method);
}

/// Every `DataFrame.<m>(…)`: the op's model; with `inplace=True` its result replaces the
/// receiver's columns (and what is known of its rows) and the call returns None.
fn df_dispatch(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
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
        "insert" => df_insert,
        "pop" => df_pop,
        "melt" => melt,
        "explode" => explode,
        "filter" => df_filter,
        "reindex" => reindex,
        "select_dtypes" => select_dtypes,
        "sort_values" | "nlargest" | "nsmallest" => sort_values,
        "transform" => transform,
        "update" => df_update,
        "set_axis" => set_axis,
        "rename_axis" => rename_axis,
        "transpose" | "T" => transpose,
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

fn series_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
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
            new_frame(an, st, c, row, None, None)
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
                st.set_slot(sid, Ns::Meta, mk("index"), v, true);
            }
            Value::obj(sid)
        }
        "value_counts" => {
            let vs = ss.iter().map(|x| vals(st, *x, vkey())).collect::<Option<Vec<Known>>>();
            let row = match vs {
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
        "copy" | "sort_values" | "sort_index" | "astype" | "fillna" | "round" | "abs" | "clip" | "cumsum" | "cumprod" | "cummax" | "cummin" | "shift" | "diff" | "rank" | "pct_change" | "isna" | "isnull" | "notna" | "notnull" | "replace" | "where" | "mask" | "map" | "apply" | "transform" | "isin" | "between" | "eq" | "ne" | "lt" | "le" | "gt" | "ge" | "add" | "sub" | "mul" | "div" | "truediv" | "floordiv" | "mod" | "pow" | "interpolate" | "ffill" | "bfill" | "convert_dtypes" | "infer_objects" | "explode" | "rolling" | "expanding" | "ewm" | "mean_" => {
            let sid = series_like_id(an, st, c, &ss, idx, "m");
            if !matches!(name.as_str(), "copy" | "sort_values" | "sort_index") {
                set_vals(st, sid, vkey(), None);
                st.set_slot(sid, Ns::Meta, dtype_key(vkey()), Value::data("dtype"), true);
            }
            Value::obj(sid)
        }
        "unique" | "tolist" | "to_list" | "to_numpy" => Value::data(&format!("{}@L{}", name, c.line)),
        _ if SERIES_SCALAR_OPS.contains(&name.as_str()) => Value::data(&format!("{}@L{}", name, c.line)),
        _ => series(an),
    }
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

fn copy_meta(src: u32, dst: u32, state: &mut State) {
    if let Some(m) = state.slot_opt(src, Ns::Meta, key_str_lit("index")) {
        state.set_slot(dst, Ns::Meta, key_str_lit("index"), m, true);
    }
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
    if v.pts.is_empty() {
        return if v.kv.is_literal() { dtype_of_lits(&kv_literals(&v.kv)) } else { None };
    }
    let o = v.single()?;
    if !matches!(an.kind(o), Kind::List | Kind::Tuple) {
        return None;
    }
    if an.obj(o).literal_len == Some(0) {
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

fn new_frame(an: &mut Analyzer, st: &mut State, c: &Call, row: Row, label: Option<&str>, src: Option<u32>) -> Value {
    let label = label.map(|l| l.to_string()).unwrap_or_else(|| format!("frame@L{}", c.line));
    let nid = an.alloc_at(Kind::Frame, &c.node, st, &label, Payload::None, Some(row), Ns::Item, None, None);
    if let Some(s) = src {
        copy_meta(s, nid, st);
    }
    Value::obj(nid)
}

fn recv_row(an: &Analyzer, st: &State, c: &Call) -> Row {
    let fs = frames(an, c.recv.as_ref());
    if fs.is_empty() {
        Row::top()
    } else {
        join_rows(fs.iter().map(|f| st.row(*f, Ns::Item)))
    }
}

fn open_frame_from(an: &mut Analyzer, oid: u32, node: &Node, state: &mut State, op: &str) -> Value {
    let row = state.row(oid, Ns::Item);
    let r = Row::from_data(RowData { sources: intern::IdSet::one((Source::new(&format!("{}@L{}", op, node.line), SourceKind::Call), EMPTY_LENS)), mutators: row.mutators.clone(), ..Default::default() });
    let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(r), Ns::Item, None, None);
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
    let n = an.obj(o).literal_len?;
    (0..n).map(|i| st.slot(o, Ns::Pos, intern::key_int(i as i64)).kv.as_lit().filter(|_| true)).collect()
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

fn read_frame(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
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
    for x in v.pts.iter() {
        match an.kind(x) {
            Kind::Dict | Kind::Frame => out.push(Ok(st.row(x, Ns::Item))),
            Kind::List | Kind::Tuple => out.push(Err(an.obj(x).literal_len?)),
            _ => return None,
        }
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
            Ok(r) => {
                let mut d = Row::new().copy_from(r, true, &KeySet::new(), EMPTY_LENS).data();
                // a key some record has is a column of the frame
                d.definite = d.present.clone();
                d.likely = d.likely.union(&d.present);
                row = row.copy_from(&Row::from_data(d), true, &KeySet::new(), EMPTY_LENS);
            }
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
        new_frame(an, st, c, row, None, None)
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
            if let Some(len) = an.obj(d).literal_len {
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

fn concat(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(objs) = c.arg(0, Some("objs")) else { return open_frame_call(an, st, c) };
    let axis = c.kw("axis");
    let join = c.kw("join");
    let inner = key_is(&join, "inner");
    let mut fs: Vec<u32> = Vec::new();
    let it = an.iterate(&objs, &c.node, st);
    if let Some(items) = &it.items {
        for item in items {
            fs.extend(frames(an, Some(item)));
        }
    } else if let Elem::One(e) = &it.elem {
        fs.extend(frames(an, Some(e)));
    }
    if fs.is_empty() {
        return open_frame_call(an, st, c);
    }
    let rows: Vec<Row> = fs.iter().map(|f| st.row(*f, Ns::Item)).collect();
    if inner && (axis.is_none() || axis_is_index(&axis)) {
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
        if it.items.is_none() {
            row = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.clone(), mutators: row.mutators.clone(), top: row.top, ..Default::default() });
        }
        return new_frame(an, st, c, row, None, None);
    }
    let mut row = Row::new();
    for r in &rows {
        row = row.copy_from(r, true, &KeySet::new(), EMPTY_LENS);
    }
    new_frame(an, st, c, row, None, None)
}

/// The op reads these columns of each frame (raising for a missing one): verdicts, and the
/// frames the program can go on with (those not surely lacking one; all when none is left).
fn require_cols(an: &mut Analyzer, st: &mut State, c: &Call, fs: &[u32], keys: &KeySet) -> Vec<u32> {
    let guarded = an.is_guarded(Ns::Item);
    let mut alive = Vec::new();
    let unsure = fs.iter().any(|f| keys.iter().any(|k| !matches!(classify(&st.row(*f, Ns::Item), k).tier, Tier::Proven | Tier::Likely)));
    for f in fs {
        let dead = keys.iter().any(|k| classify(&st.row(*f, Ns::Item), k).tier == Tier::Missing);
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
        join_rows(fs.iter().map(|f| st.row(*f, Ns::Item)))
    }
}

fn merge_impl(an: &mut Analyzer, st: &mut State, c: &Call, left: Value, right: Value, mode: usize) -> Value {
    let lf = frames(an, Some(&left));
    let rf = frames(an, Some(&right));
    let mut on = c.kw("on");
    if on.is_none() && c.args.len() > (if mode == 0 { 1 } else { 2 }) {
        on = Some(c.args[if mode == 0 { 1 } else { 2 }].clone());
    }
    let left_on = c.kw("left_on");
    let right_on = c.kw("right_on");
    let on_keys = on.as_ref().and_then(|o| an.lit_keys(o, st));
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
    let lrow = if lf.is_empty() { Row::top() } else { join_rows(lf.iter().map(|f| st.row(*f, Ns::Item))) };
    let rrow = if rf.is_empty() { Row::top() } else { join_rows(rf.iter().map(|f| st.row(*f, Ns::Item))) };
    let mut suffixes = ("_x".to_string(), "_y".to_string());
    if let Some(sv) = c.kw("suffixes") {
        for x in sv.pts.iter() {
            if matches!(an.kind(x), Kind::Tuple | Kind::List) && an.obj(x).literal_len == Some(2) {
                let a = st.slot(x, Ns::Pos, intern::key_int(0));
                let b = st.slot(x, Ns::Pos, intern::key_int(1));
                if let (Some(ka), Some(kb)) = (a.kv.as_lit(), b.kv.as_lit()) {
                    let sa = intern::with_key(ka, |k| if matches!(k, intern::Key::None) { String::new() } else { intern::key_py_str(ka) });
                    let sb = intern::with_key(kb, |k| if matches!(k, intern::Key::None) { String::new() } else { intern::key_py_str(kb) });
                    suffixes = (sa, sb);
                }
            }
        }
    }
    let mut keys_both: KeySet = on_keys.clone().unwrap_or_default();
    if on_keys.is_none() && lkeys.is_none() && rkeys.is_none() {
        keys_both = if lrow.closed() && rrow.closed() { lrow.present.intersection(&rrow.present) } else { KeySet::new() };
    }
    let mut row = Row::new().copy_from(&lrow, true, &KeySet::new(), EMPTY_LENS).copy_from(&rrow, true, &KeySet::new(), EMPTY_LENS);
    let overlap = lrow.present.intersection(&rrow.present).difference(&keys_both);
    for col in intern::sorted_by_str(overlap.iter()) {
        let both_def = lrow.definite.contains(col) && rrow.definite.contains(col);
        if let Some(cs) = intern::key_as_str(col) {
            if !suffixes.0.is_empty() {
                row = row.store(key_str_lit(&format!("{}{}", cs, suffixes.0)), both_def);
            }
            if !suffixes.1.is_empty() {
                row = row.store(key_str_lit(&format!("{}{}", cs, suffixes.1)), both_def);
            }
        }
        if both_def {
            row = row.delete(col, true);
        } else {
            let mut d = row.data();
            d.definite = d.definite.remove(col);
            d.likely = d.likely.remove(col);
            d.terms = Default::default();
            d.dterms = Default::default();
            d.lterms = Default::default();
            d.pstores = Default::default();
            row = Row::from_data(d);
        }
    }
    if on_keys.is_none() && (lrow.open() || rrow.open()) && lkeys.is_none() && rkeys.is_none() {
        an.note(format!("merge@L{}: join keys unknown (no on=)", c.line));
    }
    new_frame(an, st, c, row, None, None)
}

fn pd_merge(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let left = c.args.first().cloned().unwrap_or_default();
    let right = c.args.get(1).cloned().or_else(|| c.kw("right")).unwrap_or_default();
    merge_impl(an, st, c, left, right, 1)
}

fn df_merge(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let left = c.recv.clone().unwrap_or_default();
    let right = c.args.first().cloned().or_else(|| c.kw("right")).unwrap_or_default();
    merge_impl(an, st, c, left, right, 0)
}

fn df_join(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let other = c.args.first().cloned().or_else(|| c.kw("other")).unwrap_or_default();
    let lf = frames(an, c.recv.as_ref());
    let rf = frames(an, Some(&other));
    let on = c.kw("on");
    let on_keys = on.as_ref().and_then(|o| an.lit_keys(o, st));
    let lf = match &on_keys {
        Some(ok) => require_cols(an, st, c, &lf, ok),
        None => lf,
    };
    let lrow = if lf.is_empty() { Row::top() } else { join_rows(lf.iter().map(|f| st.row(*f, Ns::Item))) };
    let rrow = if rf.is_empty() { Row::top() } else { join_rows(rf.iter().map(|f| st.row(*f, Ns::Item))) };
    let lsuf = c.kw("lsuffix").and_then(|v| v.kv.as_lit()).map(intern::key_py_str).unwrap_or_default();
    let rsuf = c.kw("rsuffix").and_then(|v| v.kv.as_lit()).map(intern::key_py_str).unwrap_or_default();
    let mut row = Row::new().copy_from(&lrow, true, &KeySet::new(), EMPTY_LENS).copy_from(&rrow, true, &KeySet::new(), EMPTY_LENS);
    let overlap = lrow.present.intersection(&rrow.present).difference(&on_keys.unwrap_or_default());
    for col in intern::sorted_by_str(overlap.iter()) {
        let both_def = lrow.definite.contains(col) && rrow.definite.contains(col);
        if lsuf.is_empty() && rsuf.is_empty() {
            if both_def {
                let target = lf.first().copied().or_else(|| rf.first().copied()).unwrap_or(0);
                an.verdict(&c.node, col, &Classification::with_note(Tier::Missing, "join: overlapping column without suffix raises ValueError"), target, Ns::Item, None, None);
            }
            continue;
        }
        let cs = intern::key_py_str(col);
        if !lsuf.is_empty() {
            row = row.store(key_str_lit(&format!("{}{}", cs, lsuf)), both_def);
        }
        if !rsuf.is_empty() {
            row = row.store(key_str_lit(&format!("{}{}", cs, rsuf)), both_def);
        }
        if both_def && !lsuf.is_empty() && !rsuf.is_empty() {
            row = row.delete(col, true);
        }
    }
    new_frame(an, st, c, row, None, None)
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
                let row = recv_row(an, st, c);
                return new_frame(an, st, c, row, None, fs.first().copied());
            }
        }
    }
    let row = recv_row(an, st, c);
    let Some(cols) = cols else { return new_frame(an, st, c, row, None, fs.first().copied()) };
    let steps = rename_step(an, st, &cols, c, &fs);
    let Some(steps) = steps else {
        an.note(format!("rename@L{}: unknown column mapping", c.line));
        let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.insert((Source::new(&format!("rename@L{}", c.line), SourceKind::Call), EMPTY_LENS)), mutators: row.mutators.clone(), ..Default::default() });
        return new_frame(an, st, c, r, None, None);
    };
    let mut out = row;
    for s in &steps {
        out = out.rename(s);
    }
    new_frame(an, st, c, out, None, fs.first().copied())
}

fn add_prefix(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fnk = if c.qual.ends_with("prefix") { FnKind::Prefix } else { FnKind::Suffix };
    let arg = c.arg(0, Some(fnk.name()));
    let row = recv_row(an, st, c);
    if let Some(k) = lit_key(&arg) {
        let r = row.rename(&Step::RenameFn(RenameFn::with_arg(fnk, k)));
        return new_frame(an, st, c, r, None, None);
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
        return new_frame(an, st, c, row, None, fs.first().copied());
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
    new_frame(an, st, c, row, None, fs.first().copied())
}

fn df_assign(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let mut row = recv_row(an, st, c);
    for k in c.kwargs.keys() {
        row = row.store(intern::key(intern::Key::Str(intern::str_get(*k))), true);
    }
    let fs = frames(an, c.recv.as_ref());
    new_frame(an, st, c, row, None, fs.first().copied())
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
    let mut out = Row::new();
    for k in &new_cols {
        out = out.store(*k, true);
    }
    let mut d = out.copy_from(&row, true, &KeySet::new(), EMPTY_LENS).data();
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
        }
    }
    v
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

pub fn groupby_select(an: &mut Analyzer, gid: u32, key: &Value, node: &Node, state: &mut State) -> Value {
    let Payload::Groupby(fs, ks, ai, _sel) = an.obj(gid).payload.clone() else { return Value::new() };
    let cols = if !key.pts.is_empty() { an.lit_keys(key, state) } else { Some(kv_literals(&key.kv)) };
    let cols = cols.filter(|c| !c.is_empty());
    if let Some(cs) = &cols {
        let call = Call { qual: "groupby.select".into(), recv: None, args: vec![], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: *node, call_ast: None, site: format!("L{}", node.line), line: node.line };
        let _ = require_cols(an, state, &call, &fs, cs);
    }
    let sel = cols.map(|cs| (cs, matches!(key.kv, KeyVal::Lit(_)) && key.pts.is_empty()));
    let payload = Payload::Groupby(fs, ks, ai, sel);
    let nid = an.alloc_at(Kind::Groupby, node, state, &format!("groupby@L{}", node.line), payload.clone(), None, Ns::Item, None, None);
    an.obj_mut(nid).payload = payload;
    Value::obj(nid)
}

fn groupby_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let mut name = last(&c.qual).to_string();
    let recv = c.recv.clone().unwrap_or_default();
    let gids: Vec<u32> = recv.pts.iter().filter(|o| an.kind(*o) == Kind::Groupby).collect();
    if gids.is_empty() {
        return Value::data(&format!("L{}", c.line));
    }
    let Payload::Groupby(fs, ks, ai, sel) = an.obj(gids[0]).payload.clone() else { return Value::new() };
    let base = if fs.is_empty() { Row::top() } else { join_rows(fs.iter().map(|f| st.row(*f, Ns::Item))) };
    let keys = ks.clone().unwrap_or_default();
    let by_seq: Option<Vec<KeyId>> = st.slot_opt(gids[0], Ns::Meta, mk("by")).and_then(|v| v.kv.as_lit()).and_then(|k| intern::with_key(k, |kk| if let intern::Key::Tuple(t) = kk { Some(t.clone()) } else { None }));
    let levels: Option<Vec<Option<KeyId>>> = by_seq.as_ref().map(|s0| s0.iter().map(|k| Some(*k)).collect());
    // the group labels: the values of a single key column, when known
    let group_labels: Option<Known> = match (fs.as_slice(), by_seq.as_deref()) {
        ([f], Some([k])) => vals(st, *f, *k).map(|v| {
            let nk = none_key();
            if key_is_bool(&c.kw("dropna"), false) { v } else { Known { may: v.may.remove(nk), sure: v.sure.remove(nk) } }
        }),
        _ => None,
    };
    let with_index = |an: &mut Analyzer, st: &mut State, row: Row| -> Value {
        let v = new_frame(an, st, c, row, None, None);
        let nid = v.pts.first().unwrap();
        if ai && !keys.is_empty() {
            set_index_levels(st, nid, levels.as_deref());
            set_labels(st, nid, group_labels.as_ref());
        }
        v
    };
    if name == "agg" || name == "aggregate" {
        if !c.kwargs.is_empty() {
            let mut named: Vec<KeyId> = Vec::new();
            for v in c.kwargs.values() {
                for x in v.pts.iter() {
                    if an.kind(x) == Kind::Tuple && an.obj(x).literal_len == Some(2) {
                        let col = st.slot(x, Ns::Pos, intern::key_int(0));
                        if let Some(k) = col.kv.as_lit() {
                            named.push(k);
                        }
                    }
                }
            }
            let _ = require_cols(an, st, c, &fs, &KeySet::from_vec(named));
            let mut row = Row::new();
            for k in c.kwargs.keys() {
                row = row.store(intern::key(intern::Key::Str(intern::str_get(*k))), true);
            }
            if !ai {
                for k in keys.iter() {
                    row = row.store(k, true);
                }
            }
            return with_index(an, st, row);
        }
        if let Some(spec) = c.args.first() {
            let _m = an.literal_mapping(spec, st);
            let recs = an.record_objects(spec);
            if !recs.is_empty() {
                let mut row = Row::new();
                for x in &recs {
                    let r = st.row(*x, Ns::Item);
                    for k in r.present.iter() {
                        row = row.store(k, r.definite.contains(k));
                    }
                }
                let _ = require_cols(an, st, c, &fs, &row.present.clone());
                if !ai {
                    for k in keys.iter() {
                        row = row.store(k, true);
                    }
                }
                return with_index(an, st, row);
            }
        }
        name = "sum".into();
    }
    match name.as_str() {
        "sum" | "mean" | "min" | "max" | "count" | "median" | "std" | "var" | "first" | "last" | "nunique" | "prod" | "sem" | "skew" | "any" | "all" | "quantile" | "size" | "idxmax" | "idxmin" => {
            if name == "size" {
                return series(an);
            }
            let mut row = match &sel {
                Some((cols, single)) => {
                    if *single {
                        return series(an);
                    }
                    Row::closed_with(cols.clone())
                }
                None => {
                    let mut r = base.clone();
                    for k in keys.iter() {
                        r = r.delete(k, true);
                    }
                    if base.closed() {
                        r = Row::from_data(RowData { present: r.present.clone(), definite: r.definite.clone(), likely: r.likely.clone(), ..Default::default() });
                    }
                    r
                }
            };
            if !ai {
                for k in keys.iter() {
                    row = row.store(k, true);
                }
            }
            with_index(an, st, row)
        }
        "apply" | "pipe" => {
            if let Some(fnv) = c.args.first() {
                if !fnv.pts.is_empty() {
                    let vid = an.alloc_at(Kind::Frame, &c.node, st, &format!("group@L{}", c.line), Payload::None, Some(base.clone()), Ns::Item, None, None);
                    let mut oargs = vec![Value::obj(vid)];
                    oargs.extend(c.args[1..].iter().cloned());
                    let res = an.call_value(fnv, &c.node, None, st, Some(oargs));
                    if !frames(an, Some(&res)).is_empty() {
                        return res;
                    }
                    let recs = an.record_objects(&res);
                    if !recs.is_empty() {
                        let row = join_rows(recs.iter().map(|x| st.row(*x, Ns::Item)));
                        return with_index(an, st, row);
                    }
                }
            }
            series(an)
        }
        "transform" | "rank" | "cumsum" | "cumcount" | "shift" | "diff" | "pct_change" | "ngroup" | "fillna" | "ffill" | "bfill" => {
            if let Some((_, true)) = &sel {
                return series(an);
            }
            let row = match &sel {
                None => base,
                Some((cols, _)) => Row::closed_with(cols.clone()),
            };
            new_frame(an, st, c, row, None, None)
        }
        "head" | "tail" | "nth" | "sample" | "filter" | "get_group" | "take" => new_frame(an, st, c, base, None, None),
        "reset_index" => Value::data("gb.reset_index"),
        "describe" => new_frame(an, st, c, Row::with_source(Source::new(&format!("describe@L{}", c.line), SourceKind::Call)), None, None),
        "__getitem__" => {
            let key = c.args.first().cloned().unwrap_or_default();
            groupby_select(an, gids[0], &key, &c.node, st)
        }
        _ => series(an),
    }
}

fn mark_view(an: &mut Analyzer, vrow: u32, fs: &[u32]) {
    an.obj_mut(vrow).iter_elem = true;
    if fs.len() == 1 {
        an.obj_mut(vrow).derived_from = Some(fs[0]);
    }
}

fn df_apply(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let fnv = c.arg(0, Some("func"));
    let rowwise = axis_is_columns(&c.kw("axis"));
    let Some(fnv) = fnv.filter(|f| !f.pts.is_empty()) else { return series(an) };
    if rowwise {
        let base = recv_row(an, st, c);
        let label = format!("row of {}", fs.first().map(|f| an.obj(*f).label.clone()).unwrap_or_else(|| "frame".into()));
        let vrow = an.alloc_at(Kind::Dict, &c.node, st, &label, Payload::None, Some(base.clone()), Ns::Item, Some(false), None);
        mark_view(an, vrow, &fs);
        for k in base.present.iter() {
            st.set_slot(vrow, Ns::Item, k, Value::data(&format!("cell {}", intern::key_py_str(k))), false);
        }
        let mut oargs = vec![Value::obj(vrow)];
        oargs.extend(c.args[1..].iter().cloned());
        let res = an.call_value(&fnv, &c.node, None, st, Some(oargs));
        if key_is(&c.kw("result_type"), "expand") && !an.record_objects(&res).is_empty() {
            let recs = an.record_objects(&res);
            let row = join_rows(recs.iter().map(|x| st.row(*x, Ns::Item)));
            return new_frame(an, st, c, row, None, None);
        }
        let recs = an.record_objects(&res);
        if !frames(an, Some(&res)).is_empty() || !recs.is_empty() {
            if !recs.is_empty() {
                let row = join_rows(recs.iter().map(|x| st.row(*x, Ns::Item)));
                return new_frame(an, st, c, row, None, None);
            }
        }
        return series(an);
    }
    let mut oargs = vec![series(an)];
    oargs.extend(c.args[1..].iter().cloned());
    an.call_value(&fnv, &c.node, None, st, Some(oargs));
    let row = recv_row(an, st, c);
    new_frame(an, st, c, row, None, None)
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

fn itertuples(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let base = recv_row(an, st, c);
    let fs = frames(an, c.recv.as_ref());
    let vrow = an.alloc_at(Kind::Namespace, &c.node, st, &format!("row@L{}", c.line), Payload::None, Some(base.clone()), Ns::Attr, Some(false), None);
    mark_view(an, vrow, &fs);
    for k in base.present.iter() {
        if intern::key_is_str(k) {
            st.set_slot(vrow, Ns::Attr, k, Value::data(&format!("cell {}", intern::key_py_str(k))), false);
        }
    }
    let r = st.row(vrow, Ns::Attr).store(key_str_lit("Index"), true);
    st.set_row(vrow, Ns::Attr, r);
    let ctx = an.frame().ctx.clone();
    let vid = an.alloc_raw(Kind::View, &format!("view:elems:{}", c.site), &ctx, true, "itertuples", Payload::View(ViewKind::Elems, Value::obj(vrow)));
    an.obj_mut(vid).payload = Payload::View(ViewKind::Elems, Value::obj(vrow));
    Value::obj(vid)
}

fn to_dict(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let base = recv_row(an, st, c);
    let orient = c.arg(0, Some("orient"));
    let o = lit_key(&orient).map(intern::key_py_str).unwrap_or_else(|| "dict".into());
    if o == "records" {
        let fs = frames(an, c.recv.as_ref());
        let vrow = an.alloc_at(Kind::Dict, &c.node, st, &format!("record@L{}", c.line), Payload::None, Some(base.clone()), Ns::Item, Some(false), None);
        mark_view(an, vrow, &fs);
        for k in base.present.iter() {
            st.set_slot(vrow, Ns::Item, k, Value::data(&format!("cell {}", intern::key_py_str(k))), false);
        }
        let lid = an.alloc_simple(Kind::List, &c.node, st, &format!("records@L{}", c.line));
        an.obj_mut(lid).literal_len = None;
        st.set_slot(lid, Ns::Elem, intern::STAR, Value::obj(vrow), true);
        st.elems.insert(lid, DATA);
        return Value::obj(lid);
    }
    if matches!(o.as_str(), "dict" | "list" | "series" | "split" | "tight" | "index") {
        let row = if matches!(o.as_str(), "dict" | "list" | "series") { base } else { Row::with_source(Source::new(&format!("to_dict@L{}", c.line), SourceKind::Call)) };
        let oid = an.alloc_at(Kind::Dict, &c.node, st, &format!("to_dict@L{}", c.line), Payload::None, Some(row), Ns::Item, None, None);
        return Value::obj(oid);
    }
    Value::data("to_dict")
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

fn melt(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let idv = c.kw("id_vars").or_else(|| c.arg(0, None));
    let ids = match &idv {
        Some(v) => an.lit_keys(v, st),
        None => Some(KeySet::new()),
    };
    let Some(ids) = ids else {
        let call = Call { qual: "melt".into(), recv: None, args: vec![], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: c.node, call_ast: None, site: c.site.clone(), line: c.line };
        return open_frame_call(an, st, &call);
    };
    let _ = require_cols(an, st, c, &fs, &ids);
    let vn = lit_key(&c.kw("var_name")).unwrap_or_else(|| key_str_lit("variable"));
    let vln = lit_key(&c.kw("value_name")).unwrap_or_else(|| key_str_lit("value"));
    let ks = ids.insert(vn).insert(vln);
    new_frame(an, st, c, Row::closed_with(ks), None, None)
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
        return new_frame(an, st, c, row, None, fs.first().copied());
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
    let row = recv_row(an, st, c);
    if let Some(cols) = c.kw("columns") {
        if let Some(seq) = lit_key_seq(an, st, &cols) {
            return new_frame(an, st, c, closed_ordered(&seq), None, None);
        }
        if let Some(ks) = an.lit_keys(&cols, st) {
            return new_frame(an, st, c, row.project(&ks), None, None);
        }
    }
    new_frame(an, st, c, row, None, None)
}

/// Does a dtype match a `select_dtypes` spec ("number", "int64", "float", "bool", "object", …)?
fn dtype_matches(dt: &str, spec: &str) -> Option<bool> {
    Some(match spec {
        "number" | "numeric" => matches!(dt, "int" | "float"),
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
    let v = new_frame(an, st, c, row.clone(), None, fs.first().copied());
    if let (Some(nid), [f]) = (v.single(), fs.as_slice()) {
        copy_dtypes(st, *f, nid, &row.present);
    }
    v
}

fn transform(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let row = recv_row(an, st, c);
    new_frame(an, st, c, row, None, None)
}

fn df_update(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if let Some(other) = c.args.first() {
        for f in frames(an, c.recv.as_ref()) {
            for g in frames(an, Some(other)) {
                let r = st.row(f, Ns::Item).copy_from(&st.row(g, Ns::Item), false, &KeySet::new(), EMPTY_LENS);
                st.set_row(f, Ns::Item, r);
            }
        }
    }
    an.none_v.clone()
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
        return derived(an, st, c, row, &fs, rows, with_vals);
    }
    if OPEN_FRAME_OPS.contains(&name.as_str()) {
        return match fs.first() {
            Some(f) => open_frame_from(an, *f, &c.node, st, &name),
            None => Value::data(&name),
        };
    }
    if name == "get" {
        if let Some(a) = c.args.first() {
            if a.kv.as_lit().is_some() {
                let _ = fs;
                return Value::data(&format!("get@L{}", c.line)).join(&an.none_v.clone());
            }
        }
        return series(an);
    }
    if name == "items" || name == "keys" {
        let kind = if name == "items" { ViewKind::Items } else { ViewKind::Keys };
        let ids: Vec<String> = fs.iter().map(|f| f.to_string()).collect();
        let recv = c.recv.clone().unwrap_or_default();
        let vid = an.alloc_raw(Kind::View, &format!("view:{}:{}", name, ids.join(",")), "", true, &name, Payload::View(kind, recv.clone()));
        an.obj_mut(vid).payload = Payload::View(kind, recv);
        return Value::obj(vid);
    }
    if matches!(name.as_str(), "rolling" | "expanding" | "ewm" | "resample" | "squeeze" | "xs") {
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
    new_frame(an, st, c, r, None, None)
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
            return Value::data(name);
        }
        return an.method_value(&format!("DataFrame.{}", name), &Value::obj(oid));
    }
    frame_getitem(an, oid, &Value::lit_str(name), node, state, guarded)
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

pub fn frame_getitem(an: &mut Analyzer, oid: u32, key: &Value, node: &Node, state: &mut State, guarded: bool) -> Value {
    frame_getitem_n(an, oid, key, node, state, guarded, true)
}

/// `narrow`: a failing access raises here (false for loads that never fail).
pub fn frame_getitem_n(an: &mut Analyzer, oid: u32, key: &Value, node: &Node, state: &mut State, guarded: bool, narrow: bool) -> Value {
    let row = state.row(oid, Ns::Item);
    let site = an.access_site(node);
    if let (Some(k), true) = (key.kv.as_lit(), key.pts.is_empty()) {
        let cls = classify(&row, k);
        if guarded && !crate::interp::disabled("guardedverdicts") {
            an.verdict_guarded(node, k, &cls, oid, Ns::Item);
        }
        if !guarded {
            an.verdict(node, k, &cls, oid, Ns::Item, None, None);
            if cls.tier == Tier::Input {
                an.require_providers(&cls, &row, &site, true, state);
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
            for k in intern::sorted_by_str(ks.iter()) {
                frame_getitem_n(an, oid, &Value::lit(k), node, state, guarded, narrow);
            }
            let r = match lit_key_seq(an, state, key) {
                Some(seq) => closed_ordered(&seq),
                None => Row::closed_with(ks.clone()),
            };
            let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(r), Ns::Item, None, None);
            return Value::obj(nid);
        }
    }
    if is_seriesish(an, key) || key.pts.iter().any(|x| an.kind(x) == Kind::Frame) {
        let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(row), Ns::Item, None, None);
        copy_meta(oid, nid, state);
        return Value::obj(nid);
    }
    if let KeyVal::KeysOf(_) = &key.kv {
        let r = Row::new().copy_from(&row, false, &KeySet::new(), EMPTY_LENS);
        let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(r), Ns::Item, None, None);
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
    let row = state.row(foid, Ns::Item);
    let positional = name == "iloc" || name == "iat";
    let mut parts: Option<(Value, Value)> = None;
    for x in key.pts.iter() {
        if an.kind(x) == Kind::Tuple && an.obj(x).literal_len == Some(2) {
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
            return row_series(an, foid, node, state);
        }
        return frame_with(an, foid, node, state, row);
    };
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
        if an.kind(x) == Kind::Tuple && an.obj(x).literal_len == Some(2) {
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
