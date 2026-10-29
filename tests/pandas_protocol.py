"""The pandas model (`models/pandas.rs`), checked exhaustively against real pandas in the style
of `lib_protocol.py`:

- an input frame (`INPUTS`): literal frames (from a dict of lists, from records, empty, with
  None), one of two joined by a branch, frames read from known CSV text, and frames of
  unknown data (`read_csv` / `json.loads` of an environment variable: its run's value fixed);
- one operation or a short chain of them on it (`OPS`) whose result is a scalar, a list or a
  dict: the columns after selecting, assigning, renaming, dropping, `del`, `pop`, `insert`,
  merging, grouping and aggregating, indexing, concatenating, filtering, setting `.columns`;
  a column's values, sums, maxima, uniques, counts; records; with missing columns too;
- in a `try` naming the class of what it raises.

Observation point: the result (a numpy scalar as its Python value; a scalar, a container's
items, `lib_protocol.ENC`) or the class it raised.

    uv run --no-project --with pandas python tests/pandas_protocol.py [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from lib_protocol import ENC_HELPERS, TAIL_ENC, program
from protocol_lib import cli

NAMES = ["O"]

LIB = {
    "imports": ["import io", "import json", "import pandas as pd"],
    "inputs": {
        "literal": "pd.DataFrame({'a': [1, 2, 3], 'b': ['x', 'y', 'x']})",
        "floats": "pd.DataFrame({'a': [1.5, None], 'c': [True, False]})",
        "empty": "pd.DataFrame()",
        "records": "pd.DataFrame([{'a': 1, 'b': 2}, {'a': 3}])",
        "joined": "pd.DataFrame({'a': [1]}) if c1 else pd.DataFrame({'a': [2], 'b': ['z']})",
        "csv": "pd.read_csv(io.StringIO('a,b\\n1,x\\n2,y\\n'))",
        "unknown": "pd.read_csv(io.StringIO(os.environ.get('RP{N}', 'a,b\\n3,q\\n')))",
        "json": "pd.DataFrame(json.loads(os.environ.get('RJ{N}', '[{\"a\": 1, \"b\": \"k\"}]')))",
        "maybe_col": "MAYBE(pd.DataFrame({'a': [3, 1], 'b': ['p', 'q']}))",
        "order": "pd.DataFrame({'b': ['u'], 'a': [5]}) if c2 else pd.DataFrame({'a': [6], 'b': ['w']})",
    },
    "ops": {
        "cols": "list(v.columns)", "len": "len(v)", "shape": "v.shape", "col": "v['a'].tolist()",
        "col_missing": "v['zz'].tolist()", "attr": "v.b.tolist()", "sel": "list(v[['a']].columns)",
        "sel_missing": "list(v[['a', 'zz']].columns)", "assign": "list(v.assign(c=1).columns)",
        "assign_fn": "v.assign(c=lambda d: d['a'] * 2)['c'].tolist()", "setitem": "SET(v)",
        "rename": "list(v.rename(columns={'a': 'z'}).columns)",
        "rename_raise": "list(v.rename(columns={'q': 'z'}, errors='raise').columns)",
        "drop": "list(v.drop(columns=['b']).columns)", "drop_ignore": "list(v.drop(columns=['b'], errors='ignore').columns)",
        "del": "DEL(v)", "pop": "POP(v)", "insert": "INS(v)", "get": "v.get('zz') is None", "get_b": "v.get('b') is None",
        "in": "'b' in v", "iter": "list(v)",
        "merge": "list(v.merge(pd.DataFrame({'a': [1, 2], 'm': [5, 6]}), on='a').columns)",
        "merge_suffix": "list(v.merge(pd.DataFrame({'a': [1], 'b': [0]}), on='a').columns)",
        "merge_len": "len(v.merge(pd.DataFrame({'a': [1, 2], 'm': [5, 6]}), on='a'))",
        "merge_left": "list(v.merge(pd.DataFrame({'k': [1]}), left_on='a', right_on='k', how='left').columns)",
        "groupby_sum": "list(v.groupby('a').sum().reset_index().columns)",
        "groupby_agg": "list(v.groupby('a').agg(n=('b', 'count')).columns)",
        "groupby_size": "v.groupby('a').size().tolist()", "groupby_keys": "sorted(v.groupby('a').groups)",
        "groupby_dict": "list(v.groupby('a', as_index=False).agg({'b': 'first'}).columns)",
        "set_index": "list(v.set_index('a').columns)", "reset_index": "list(v.reset_index().columns)",
        "concat": "list(pd.concat([v, pd.DataFrame({'q': [1]})]).columns)",
        "concat_len": "len(pd.concat([v, v]))", "concat_axis1": "list(pd.concat([v, pd.DataFrame({'q': [1]})], axis=1).columns)",
        "filter": "list(v[v['a'] > 1].columns)", "filter_len": "len(v[v['a'] > 1])",
        "dropna": "list(v.dropna(subset=['b']).columns)", "astype": "v.astype({'a': float})['a'].tolist()",
        "sort": "v.sort_values('a')['a'].tolist()", "sum": "v['a'].sum()", "max": "v['a'].max()",
        "records": "v.to_dict('records')", "todict": "v.to_dict('list')", "loc": "v.loc[:, 'a'].tolist()",
        "iloc_row": "v.iloc[0].tolist()", "prefix": "list(v.add_prefix('p_').columns)", "columns_set": "SETCOLS(v)",
        "select_dtypes": "list(v.select_dtypes('number').columns)", "unique": "v['b'].unique().tolist()",
        "value_counts": "v['b'].value_counts().to_dict()", "apply_row": "v.apply(lambda r: r['a'], axis=1).tolist()",
        "itertuples": "[t.a for t in v.itertuples()]", "iterrows": "[r['a'] for _, r in v.iterrows()]",
        "query": "len(v.query('a > 1'))", "fillna": "v.fillna(0)['a'].tolist()", "copy_set": "COPYSET(v)",
        "columns_attr": "v.columns.tolist()", "dtypes": "str(v['a'].dtype)", "empty": "v.empty",
        "mean": "v['a'].mean()", "head": "len(v.head(1))", "rename_fn": "list(v.rename(columns=str.upper).columns)",
        "rename_clash": "list(v.rename(columns={'a': 'b'}).columns)", "merge_outer": "list(v.merge(pd.DataFrame({'a': [9]}), how='outer', indicator=True).columns)",
        "merge_inner_len": "len(v.merge(pd.DataFrame({'a': [1, 1]}), on='a', how='inner'))", "groupby_two": "v.groupby(['a', 'b']).size().tolist()",
        "melt": "list(v.melt(id_vars=['a']).columns)", "dummies": "list(pd.get_dummies(v).columns)", "dropna_len": "len(v.dropna())",
        "reorder": "list(v[['b', 'a']].columns)", "reindex": "list(v.reindex(columns=['a', 'n']).columns)", "filter_like": "list(v.filter(like='a').columns)",
        "frame_sum": "v.sum().to_dict()", "nunique": "v.nunique().to_dict()", "isna": "v.isna().sum().to_dict()",
        "dedup": "len(v.drop_duplicates(subset=['a']))", "sort_two": "v.sort_values(['b', 'a'])['a'].tolist()", "agg_dict": "v.agg({'a': 'sum'}).to_dict()",
        "astype_str": "v.astype(str)['a'].tolist()", "map": "v['a'].map({1: 'one'}).tolist()", "apply_col": "v['a'].apply(lambda x: x * 2).tolist()",
        "str_upper": "v['b'].str.upper().tolist()", "col_fillna": "v['a'].fillna(0).tolist()", "iloc_col": "v.iloc[:, 0].tolist()",
        "at": "v.at[0, 'a']", "loc_cell": "v.loc[0, 'a']", "first_col": "v.columns[0]", "index": "v.index.tolist()",
        "set_index_idx": "v.set_index('a').index.tolist()", "values": "v.values.tolist()", "describe": "list(v.describe().columns)",
        "explode": "len(v.explode('b'))", "mask_assign": "MASKSET(v)", "loc_assign": "LOCSET(v)", "col_eq": "(v['a'] == 1).tolist()",
        "sum_bool": "int((v['a'] > 1).sum())", "series_name": "v['a'].name", "series_idx": "v['a'][0]", "itertuples_idx": "[t.Index for t in v.itertuples()]",
        "to_csv": "v.to_csv(index=False)", "to_json": "v.to_json(orient='records')", "columns_in": "'a' in v.columns",
        "merge_missing": "list(v.merge(pd.DataFrame({'k': [1]}), on='k').columns)", "join_overlap": "list(v.join(v).columns)",
        "join_suffix": "list(v.join(v, rsuffix='_r').columns)", "drop_missing": "list(v.drop(columns=['zz']).columns)",
        "set_index_missing": "list(v.set_index('zz').columns)", "sort_missing": "v.sort_values('zz')['a'].tolist()",
        "groupby_missing": "v.groupby('zz').size().tolist()", "astype_missing": "list(v.astype({'zz': int}).columns)",
        "fillna_dict": "v.fillna({'a': 0})['a'].tolist()", "assign_over": "v.assign(a=9)['a'].tolist()", "insert_new": "INSNEW(v)",
        "pop_missing": "POPZ(v)", "loc_missing": "v.loc[99, 'a']", "iloc_far": "v.iloc[9].tolist()", "iloc_col_far": "v.iloc[:, 5].tolist()",
        "series_iloc_far": "v['a'].iloc[5]", "nlargest": "v.nlargest(1, 'a')['a'].tolist()", "melt_missing": "list(v.melt(value_vars=['zz']).columns)",
        "attr_set": "ATTRSET(v)", "rename_index": "v.rename(index={0: 'r0'}).index.tolist()", "cols_after_merge_suffix": "list(v.merge(v, on='a', suffixes=('', '_2')).columns)",
        "pivot": "list(v.pivot(columns='b', values='a').columns)", "pivot_table": "list(v.pivot_table(index='b', values='a', aggfunc='sum').columns)",
        "value_counts_norm": "v['a'].value_counts(normalize=True).to_dict()", "shift": "v['a'].shift(1).tolist()", "cumsum": "v['a'].cumsum().tolist()",
        "drop_rows": "len(v.drop(index=[0]))", "drop_rows_missing": "len(v.drop(index=[99]))", "loc_label_list": "len(v.loc[[0, 99]])",
        "get_default": "v.get('zz', 'dflt')", "squeeze": "v[['a']].squeeze().tolist()", "items": "[k for k, _ in v.items()]",
        "dtypes_dict": "{k: str(x) for k, x in v.dtypes.items()}", "bool_frame": "bool(v)",
    },
    "helpers": [
        "def SET(v):", "    v = v.copy()", "    v['c'] = v['a'] + 1", "    return list(v.columns)",
        "def DEL(v):", "    del v['b']", "    return list(v.columns)",
        "def POP(v):", "    x = v.pop('a')", "    return [list(v.columns), x.tolist()]",
        "def INS(v):", "    v.insert(0, 'a', 1)", "    return list(v.columns)",
        "def SETCOLS(v):", "    v.columns = ['x', 'y']", "    return list(v.columns)",
        "def COPYSET(v):", "    w = v.copy()", "    w['n'] = 0", "    return [list(v.columns), list(w.columns)]",
        "def MAYBE(v):", "    if c1:", "        v['n'] = 0", "    return v",
        "def MASKSET(v):", "    v.loc[v['a'] > 1, 'b'] = 'big'", "    return v['b'].tolist()",
        "def LOCSET(v):", "    v.loc[:, 'z'] = 1", "    return list(v.columns)",
        "def INSNEW(v):", "    v.insert(0, 'n', 1)", "    return list(v.columns)",
        "def POPZ(v):", "    v.pop('zz')", "    return list(v.columns)",
        "def ATTRSET(v):", "    v.newattr = 1", "    return ['newattr' in v.columns, v.newattr]",
        *ENC_HELPERS,
    ],
}

# (a numpy scalar as its Python value: `int` / `float` / `bool` are what a Python program sees
# printed, and what rowan models)
NORM = ["        r = r.item() if hasattr(r, 'item') and not isinstance(r, (list, dict, tuple, str)) else r"]


def programs():
    for (ik, iv), (ok, ov) in itertools.product(LIB["inputs"].items(), LIB["ops"].items()):
        p = program(LIB, iv, ov, f"pandas {ik} {ok}", TAIL_ENC)
        i = p.lines.index("    except TypeError:")
        p.lines[i:i] = NORM
        yield p


if __name__ == "__main__":
    cli(programs, NAMES)
