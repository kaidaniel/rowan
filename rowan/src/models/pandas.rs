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
            "pandas.option_context", "pandas.testing.assert_frame_equal", "pandas.ExcelWriter", "pandas.eval", "Series.*",
        ],
        series_call,
    );
    r(&["DataFrame.merge"], df_merge);
    r(&["DataFrame.join"], df_join);
    r(&["DataFrame.rename"], df_rename);
    r(&["DataFrame.add_prefix", "DataFrame.add_suffix"], add_prefix);
    r(&["DataFrame.drop"], df_drop);
    r(&["DataFrame.assign"], df_assign);
    r(&["DataFrame.reset_index"], reset_index);
    r(&["DataFrame.set_index"], set_index);
    r(&["DataFrame.groupby"], groupby);
    r(&["DataFrame.apply"], df_apply);
    r(&["DataFrame.pipe"], df_pipe);
    r(&["DataFrame.iterrows"], iterrows);
    r(&["DataFrame.itertuples"], itertuples);
    r(&["DataFrame.to_dict"], to_dict);
    r(&["DataFrame.insert"], df_insert);
    r(&["DataFrame.pop"], df_pop);
    r(&["DataFrame.melt"], melt);
    r(&["DataFrame.explode"], explode);
    r(&["DataFrame.filter"], df_filter);
    r(&["DataFrame.reindex"], reindex);
    r(&["DataFrame.select_dtypes"], select_dtypes);
    r(&["DataFrame.sort_values", "DataFrame.nlargest", "DataFrame.nsmallest"], sort_values);
    r(&["DataFrame.transform"], transform);
    r(&["DataFrame.update"], df_update);
    r(&["DataFrame.set_axis"], set_axis);
    r(&["DataFrame.*"], df_generic);
    r(&["groupby.*"], groupby_method);
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
    let mut ks: Option<KeySet> = None;
    if let Some(u) = c.kw("usecols") {
        ks = an.lit_keys(&u, st);
    }
    if let Some(n) = c.kw("names") {
        ks = an.lit_keys(&n, st);
    }
    let row = match ks {
        Some(ks) => Row::closed_with(ks),
        None => Row::with_source(Source::new(&format!("{}@L{}", tag, c.line), SourceKind::External)),
    };
    new_frame(an, st, c, row, Some(&format!("{}@L{}", tag, c.line)), None)
}

fn pd_dataframe(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let data = c.arg(0, Some("data"));
    if let Some(cols) = c.kw("columns") {
        if let Some(ks) = an.lit_keys(&cols, st) {
            return new_frame(an, st, c, Row::closed_with(ks), None, None);
        }
    }
    let Some(data) = data else { return new_frame(an, st, c, Row::new(), None, None) };
    let recs = an.record_objects(&data);
    if !recs.is_empty() {
        let mut row = Row::new();
        for x in recs {
            row = row.copy_from(&st.row(x, Ns::Item), true, &KeySet::new(), EMPTY_LENS);
        }
        return new_frame(an, st, c, row, None, None);
    }
    for x in data.pts.iter() {
        if matches!(an.kind(x), Kind::List | Kind::Tuple) {
            let it = an.iterate(&data, &c.node, st);
            if let Some(items) = &it.items {
                let mut rows: Vec<Row> = Vec::new();
                for item in items {
                    let rs = an.record_objects(item);
                    rows.push(if rs.is_empty() { Row::top() } else { join_rows(rs.iter().map(|y| st.row(*y, Ns::Item))) });
                }
                let mut row = Row::new();
                for rr in &rows {
                    row = row.copy_from(rr, true, &KeySet::new(), EMPTY_LENS);
                }
                return new_frame(an, st, c, row, None, None);
            }
            let rs = match &it.elem {
                Elem::One(e) => an.record_objects(e),
                Elem::Pair(..) => Vec::new(),
            };
            if !rs.is_empty() {
                let mut row = Row::new();
                for y in rs {
                    row = row.copy_from(&st.row(y, Ns::Item), true, &KeySet::new(), EMPTY_LENS);
                }
                return new_frame(an, st, c, row, None, None);
            }
        }
    }
    new_frame(an, st, c, Row::with_source(Source::new(&format!("DataFrame@L{}", c.line), SourceKind::Call)), None, None)
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

fn require_cols(an: &mut Analyzer, st: &mut State, c: &Call, fs: &[u32], keys: &KeySet) {
    let guarded = an.is_guarded(Ns::Item);
    for f in fs {
        for k in intern::sorted_by_str(keys.iter()) {
            frame_getitem(an, *f, &Value::lit(k), &c.node, st, guarded);
        }
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
    if let Some(ok) = &on_keys {
        require_cols(an, st, c, &lf, ok);
        require_cols(an, st, c, &rf, ok);
    }
    if let Some(lk) = &lkeys {
        require_cols(an, st, c, &lf, lk);
    }
    if let Some(rk) = &rkeys {
        require_cols(an, st, c, &rf, rk);
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
    if let Some(ok) = &on_keys {
        require_cols(an, st, c, &lf, ok);
    }
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
    if !ignore {
        require_cols(an, st, c, &fs, &ks);
    }
    let mut row = recv_row(an, st, c);
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
    let mut row = recv_row(an, st, c);
    if key_is_bool(&c.kw("drop"), true) {
        return new_frame(an, st, c, row, None, None);
    }
    let meta = if fs.is_empty() { Value::new() } else { join_values(fs.iter().map(|f| st.slot(*f, Ns::Meta, key_str_lit("index")))) };
    let ks = kv_literals(&meta.kv);
    if !ks.is_empty() {
        let single = matches!(meta.kv, KeyVal::Lit(_));
        for k in ks.iter() {
            row = row.store(k, single);
        }
    } else {
        row = row.store(key_str_lit("index"), false);
        if row.closed() {
            let mut d = row.data();
            d.sources = d.sources.insert((Source::new(&format!("reset_index@L{}", c.line), SourceKind::Call), EMPTY_LENS));
            row = Row::from_data(d);
        }
    }
    new_frame(an, st, c, row, None, None)
}

fn set_index(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let keys = c.arg(0, Some("keys"));
    let ks = keys.as_ref().and_then(|k| an.lit_keys(k, st));
    let row = recv_row(an, st, c);
    let keep = key_is_bool(&c.kw("drop"), false);
    let Some(ks) = ks else {
        let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.clone(), mutators: row.mutators.clone(), ..Default::default() });
        return new_frame(an, st, c, r, None, None);
    };
    require_cols(an, st, c, &fs, &ks);
    let mut row = recv_row(an, st, c);
    if !keep {
        for k in ks.iter() {
            row = row.delete(k, true);
        }
    }
    let v = new_frame(an, st, c, row, None, None);
    let nid = v.pts.first().unwrap();
    st.set_slot(nid, Ns::Meta, key_str_lit("index"), Value::kv_only(KeyVal::lits(ks)), true);
    v
}

fn groupby(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let by = c.arg(0, Some("by"));
    let ks = by.as_ref().and_then(|b| an.lit_keys(b, st));
    if let Some(k) = &ks {
        require_cols(an, st, c, &fs, k);
    }
    let ai = !key_is_bool(&c.kw("as_index"), false);
    let payload = Payload::Groupby(fs.clone(), ks, ai, None);
    let gid = an.alloc_at(Kind::Groupby, &c.node, st, &format!("groupby@L{}", c.line), payload.clone(), None, Ns::Item, None, None);
    an.obj_mut(gid).payload = payload;
    Value::obj(gid)
}

pub fn groupby_select(an: &mut Analyzer, gid: u32, key: &Value, node: &Node, state: &mut State) -> Value {
    let Payload::Groupby(fs, ks, ai, _sel) = an.obj(gid).payload.clone() else { return Value::new() };
    let cols = if !key.pts.is_empty() { an.lit_keys(key, state) } else { Some(kv_literals(&key.kv)) };
    let cols = cols.filter(|c| !c.is_empty());
    if let Some(cs) = &cols {
        let call = Call { qual: "groupby.select".into(), recv: None, args: vec![], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: *node, call_ast: None, site: format!("L{}", node.line), line: node.line };
        require_cols(an, state, &call, &fs, cs);
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
    let with_index = |an: &mut Analyzer, st: &mut State, row: Row| -> Value {
        let v = new_frame(an, st, c, row, None, None);
        let nid = v.pts.first().unwrap();
        if !keys.is_empty() {
            st.set_slot(nid, Ns::Meta, key_str_lit("index"), Value::kv_only(KeyVal::lits(keys.clone())), true);
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
            require_cols(an, st, c, &fs, &KeySet::from_vec(named));
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
                require_cols(an, st, c, &fs, &row.present.clone());
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
    require_cols(an, st, c, &fs, &ids);
    let vn = lit_key(&c.kw("var_name")).unwrap_or_else(|| key_str_lit("variable"));
    let vln = lit_key(&c.kw("value_name")).unwrap_or_else(|| key_str_lit("value"));
    let ks = ids.insert(vn).insert(vln);
    new_frame(an, st, c, Row::closed_with(ks), None, None)
}

fn explode(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    if let Some(col) = c.arg(0, Some("column")) {
        if let Some(ks) = an.lit_keys(&col, st) {
            require_cols(an, st, c, &fs, &ks);
        }
    }
    let row = recv_row(an, st, c);
    new_frame(an, st, c, row, None, None)
}

fn df_filter(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let items = c.kw("items").or_else(|| c.arg(0, None));
    let row = recv_row(an, st, c);
    if let Some(items) = items {
        if let Some(ks) = an.lit_keys(&items, st) {
            let p = if row.closed() { ks.intersection(&row.present) } else { ks };
            return new_frame(an, st, c, row.project(&p), None, None);
        }
    }
    let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.insert((Source::new(&format!("filter@L{}", c.line), SourceKind::Call), EMPTY_LENS)), mutators: row.mutators.clone(), ..Default::default() });
    new_frame(an, st, c, r, None, None)
}

fn reindex(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let row = recv_row(an, st, c);
    if let Some(cols) = c.kw("columns") {
        if let Some(ks) = an.lit_keys(&cols, st) {
            return new_frame(an, st, c, row.project(&ks), None, None);
        }
    }
    new_frame(an, st, c, row, None, None)
}

fn select_dtypes(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let row = recv_row(an, st, c);
    let r = Row::from_data(RowData { present: row.present.clone(), sources: row.sources.clone(), mutators: row.mutators.clone(), ..Default::default() });
    new_frame(an, st, c, r, None, None)
}

fn sort_values(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fs = frames(an, c.recv.as_ref());
    let by = c.kw("by").or_else(|| if c.qual.ends_with("nlargest") || c.qual.ends_with("nsmallest") { c.arg(1, Some("columns")) } else { c.arg(0, None) });
    if let Some(by) = by {
        if let Some(ks) = an.lit_keys(&by, st) {
            require_cols(an, st, c, &fs, &ks);
        }
    }
    let row = recv_row(an, st, c);
    new_frame(an, st, c, row, None, fs.first().copied())
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
    if COPY_FRAME_OPS.contains(&name.as_str()) {
        return new_frame(an, st, c, row, None, fs.first().copied());
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
                return match fs.first() {
                    Some(f) => frame_getitem(an, *f, a, &c.node, st, true),
                    None => Value::new(),
                };
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
                return open_frame_from(an, oid, node, state, "T");
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
    if DF_ATTRS.contains(&name) {
        return;
    }
    frame_setitem(an, oid, &Value::lit_str(name), v, node, state, strong);
}

pub fn frame_getitem(an: &mut Analyzer, oid: u32, key: &Value, node: &Node, state: &mut State, guarded: bool) -> Value {
    let row = state.row(oid, Ns::Item);
    let site = an.access_site(node);
    if let (Some(k), true) = (key.kv.as_lit(), key.pts.is_empty()) {
        let cls = classify(&row, k);
        if !guarded {
            an.verdict(node, k, &cls, oid, Ns::Item, None, None);
            if cls.tier == Tier::Input {
                an.require_providers(&cls, &row, &site, true, state);
            }
            if cls.tier != Tier::Missing && an.obj(oid).single {
                state.set_row(oid, Ns::Item, row.narrow_in(k, true));
            }
        }
        return series(an);
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
                frame_getitem(an, oid, &Value::lit(k), node, state, guarded);
            }
            let row = state.row(oid, Ns::Item);
            let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(row.project(&ks)), Ns::Item, None, None);
            return Value::obj(nid);
        }
    }
    if key.pts.contains(an.series) || key.pts.iter().any(|x| an.kind(x) == Kind::Frame) {
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

pub fn frame_setitem(an: &mut Analyzer, oid: u32, key: &Value, _v: &Value, node: &Node, state: &mut State, strong: bool) {
    let row = state.row(oid, Ns::Item);
    let ks = if !key.pts.is_empty() { an.lit_keys(key, state) } else { None };
    if let (Some(k), true) = (key.kv.as_lit(), key.pts.is_empty()) {
        state.set_row(oid, Ns::Item, row.store(k, strong));
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
    if key.pts.contains(an.series) {
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

pub fn loc_get(an: &mut Analyzer, lo: u32, key: &Value, node: &Node, state: &mut State) -> Value {
    let Payload::Locidx(name, foid) = an.obj(lo).payload.clone() else { return Value::new() };
    let row = state.row(foid, Ns::Item);
    let mut parts: Option<(Value, Value)> = None;
    for x in key.pts.iter() {
        if an.kind(x) == Kind::Tuple && an.obj(x).literal_len == Some(2) {
            parts = Some((state.slot(x, Ns::Pos, intern::key_int(0)), state.slot(x, Ns::Pos, intern::key_int(1))));
        }
    }
    let Some((_, cols)) = parts else {
        let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(row), Ns::Item, None, None);
        return Value::obj(nid);
    };
    if name == "iloc" || name == "iat" {
        return series(an);
    }
    if cols.kv.as_lit().is_some() && cols.pts.is_empty() {
        let g = an.is_guarded(Ns::Item);
        return frame_getitem(an, foid, &cols, node, state, g);
    }
    if let Some(ks) = an.lit_keys(&cols, state) {
        let g = an.is_guarded(Ns::Item);
        return frame_getitem(an, foid, &Value::with_kv(cols.pts.clone(), KeyVal::Lits(ks)), node, state, g);
    }
    if let KeyVal::Data(s) = &cols.kv {
        if cols.pts.is_empty() && intern::str_get(*s).starts_with("slice") {
            let nid = an.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(row), Ns::Item, None, None);
            return Value::obj(nid);
        }
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
