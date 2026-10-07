"""Turn analysis results into text / JSON (SPECIFICATION.md §3, §13)."""
from __future__ import annotations

import json
from typing import Any, Dict, List, Optional

from .domain import (DATAKEY, INPUT, LIKELY, LIKELY_MISSING, MAP, MISSING, POSSIBLY, PROVEN, UNKNOWN,
                     TIER_RANK, TIER_SYMBOL, Inv, Source, key_str, keys_str)
from .interp import Analyzer, Verdict
from .state import RECORD_KINDS

TIER_TEXT = {
    PROVEN: "ok",
    LIKELY: "ok (likely — unknown code ran in between)",
    POSSIBLY: "possibly missing",
    MISSING: "definitely missing",
    LIKELY_MISSING: "likely missing",
    INPUT: "depends on input",
    DATAKEY: "data-derived key into a closed record",
    MAP: "map key",
    UNKNOWN: "unknown",
}


def dedupe(verdicts: List[Verdict]) -> List[Verdict]:
    """One verdict per (access site, key, call chain): per-target verdicts are combined (§13)."""
    groups: Dict[Any, List[Verdict]] = {}
    for v in verdicts:
        groups.setdefault((v.file, v.line, v.col, key_str(v.key), v.chain, v.ns), []).append(v)
    out: List[Verdict] = []
    for key, vs in groups.items():
        by_obj: Dict[int, Verdict] = {}
        for v in vs:
            cur = by_obj.get(v.oid)
            if cur is None or TIER_RANK[v.tier] > TIER_RANK[cur.tier]:
                by_obj[v.oid] = v
        labels = [v.obj for v in by_obj.values()]
        if len(set(labels)) != len(labels):
            names = {o: f"{v.obj}#{o}" for o, v in by_obj.items()}
        else:
            names = {o: v.obj for o, v in by_obj.items()}
        tiers = {v.tier for v in by_obj.values()}
        if UNKNOWN in tiers:
            continue      # one possible target is not checkable (Any-typed field, opaque value): nothing to say
        first = vs[0]
        if len(tiers) == 1:
            v = next(iter(by_obj.values()))
            objs = sorted(names.values())
            out.append(Verdict(v.line, v.col, v.key, v.tier, ", ".join(objs), v.chain, v.note, v.providers, v.ns, v.text, v.oid, v.file))
            continue
        breakdown = "; ".join(f"{TIER_SYMBOL[v.tier]} for {names[o]}" for o, v in sorted(by_obj.items()))
        if tiers <= {PROVEN, LIKELY}:
            tier = LIKELY
        elif tiers <= {INPUT, PROVEN, LIKELY}:
            tier = INPUT
        else:
            tier = POSSIBLY
        provs = tuple(sorted({p for v in by_obj.values() for p in v.providers}))
        out.append(Verdict(first.line, first.col, first.key, tier, ", ".join(sorted(names.values())), first.chain,
                           f"depends on which object: {breakdown}", provs, first.ns, first.text, first.oid, first.file))
    return sorted(out, key=lambda v: (v.file, v.line, v.col, str(v.key), v.chain))


def schema_rows(an: Analyzer) -> Dict[str, Dict[str, Any]]:
    """Source label -> {key -> {'always': bool, 'sites': [...], 'alternatives': [...], 'caveats': [...]}}"""
    st = an.final_state
    out: Dict[str, Dict[str, Any]] = {}
    if st is None:
        return out
    for src, d in st.reqs.items():
        if src.is_data:
            continue
        label = str(src)
        entry = out.setdefault(label, {})
        for k, info in d.items():
            entry[key_str(k)] = {
                "always": info.always,
                "sites": sorted(info.sites),
                "alternatives": sorted(str(a) for a in info.alternatives),
                "caveats": sorted(info.caveats),
                "alt_keys": sorted(key_str(x) for x in info.alt_keys),
            }
    return out


def _group_alt_keys(entry: Dict[str, Any]):
    """Collapse keys that are alternatives of one another into one display item."""
    seen = set()
    items = []
    for k in sorted(entry):
        if k in seen:
            continue
        group = [k] + [a for a in entry[k]["alt_keys"] if a != k and a in entry]
        for g in group:
            seen.add(g)
        label = " | ".join(group) if len(group) > 1 else k
        items.append((label, entry[k]))
    return items


def format_text(an: Analyzer, show_rows: bool = True, show_summaries: bool = True) -> str:
    lines: List[str] = []
    vs = [v for v in dedupe(an.verdicts) if v.tier not in (PROVEN, MAP)]
    oks = [v for v in dedupe(an.verdicts) if v.tier == PROVEN]
    lines.append("== diagnostics ==")
    if not vs:
        lines.append("  (none)")
    for v in vs:
        chain = f"  via {'>'.join(f'L{c}' for c in v.chain)}" if v.chain else ""
        prov = f"  ← {', '.join(v.providers)}" if v.providers else ""
        note = f"  ({v.note})" if v.note else ""
        where = f"{v.file}:" if v.file else ""
        lines.append(f"  {where}L{v.line:<4} {TIER_SYMBOL[v.tier]:<5} {v.text:<40} {v.ns}[{key_str(v.key)}] "
                     f"{TIER_TEXT[v.tier]}{prov}{note}  [{v.obj}]{chain}")
    lines.append(f"  ({len(oks)} accesses proven ok)")
    sch = schema_rows(an)
    lines.append("")
    lines.append("== inferred schemas (requirements on external inputs) ==")
    if not sch:
        lines.append("  (none)")
    for label in sorted(sch):
        entry = sch[label]
        grouped = _group_alt_keys(entry)
        always = [k for k, e in grouped if e["always"]]
        sometimes = [k for k, e in grouped if not e["always"]]
        lines.append(f"  {label}:")
        if always:
            lines.append(f"      always   : {', '.join(always)}")
        if sometimes:
            lines.append(f"      sometimes: {', '.join(sometimes)}")
        for k, e in grouped:
            extra = []
            if e["alternatives"]:
                extra.append("or provided by " + ", ".join(e["alternatives"]))
            if e["caveats"]:
                extra.append("unless added by " + ", ".join(e["caveats"]))
            if extra:
                lines.append(f"      {k}: {'; '.join(extra)}")
    if show_summaries and an.summaries:
        lines.append("")
        lines.append("== function results (per call context) ==")
        for (qual, ctx), (ret, rows) in sorted(an.summaries.items(), key=lambda kv: (kv[0][0], kv[0][1])):
            if not rows:
                continue
            for oid, row in rows.items():
                lines.append(f"  {qual} [{ctx}] -> {an.objects[oid].label}: {row}")
    if show_rows and an.final_state is not None:
        lines.append("")
        lines.append("== module-level records at exit ==")
        st = an.final_state
        mod = an.main_module
        names = sorted(k for k, _v in st.obj_slots(mod, "attr"))
        for name in names:
            v = st.slot(mod, "attr", name)
            for oid in sorted(v.pts):
                o = an.objects[oid]
                if o.kind in ("dict", "frame", "unknown", "namespace"):
                    lines.append(f"  {name} = {o.label}: {st.row(oid, 'item')}")
                elif o.kind == "instance":
                    lines.append(f"  {name} = {o.label}: attrs {st.row(oid, 'attr')}")
    if an.notes:
        lines.append("")
        lines.append("== notes ==")
        for n in an.notes:
            lines.append(f"  {n}")
    return "\n".join(lines)


def to_json(an: Analyzer) -> List[Dict[str, Any]]:
    out: List[Dict[str, Any]] = []
    for v in dedupe(an.verdicts):
        out.append({"kind": "verdict", "site": f"{v.file or an.filename}:{v.line}:{v.col}", "key": key_str(v.key),
                    "ns": v.ns, "tier": v.tier, "object": v.obj, "chain": list(v.chain), "note": v.note,
                    "providers": list(v.providers), "text": v.text})
    for label, entry in schema_rows(an).items():
        out.append({"kind": "schema", "source": label, "keys": entry})
    for (qual, ctx), (ret, rows) in an.summaries.items():
        out.append({"kind": "summary", "function": qual, "context": ctx,
                    "returns": {an.objects[o].label: str(r) for o, r in rows.items()}})
    return out
