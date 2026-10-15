"""A plugin-based data platform as a package of ten modules (`dp/`): a plugin registry with
decorator-registered handler classes in several modules (and two in this driver), a settings
layer (defaults + a JSON text + environment overrides merged and validated by pydantic),
ingestion plugins (CSV through io.StringIO, JSON lines, a regex-parsed log format), pydantic
validation with the ValidationError lists collected, pandas processing (groupby, pivot_table,
merge) next to pure-Python aggregation (collections, itertools), exporters (json.dumps with
sorted keys, csv.DictWriter) and a FastAPI app driven through TestClient.  The scenarios
come from a JSON plan text; three run the pipeline with different configs and inputs.

Expectations were written from the program text before running anything."""
import json
import os

from fastapi.testclient import TestClient

import dp
from dp import REGISTRY, Plugin, register, run_pipeline, try_settings
from dp.aggregate import all_tags, counts_by, first_ids, merge_counts, running_totals, source_mix
from dp.api import STATE, app
from dp.ingest import parse_log_line
from dp.processing import daily
from dp.settings import env_layer, merged_config
# (L110 first written `ok expect`: os.environ may hold a DP_BATCH_SIZE that fails validation
# for alpha too, whose spec has no `expect`: possibly missing, which the oracle cannot show)
@register("exporter", "text")
class TextExporter(Plugin):
    """Aligned key / value lines."""

    def render(self, payload, cfg):
        keys = sorted(payload) if cfg.sort_keys else list(payload)
        lines = []
        for k in keys:
            v = payload[k]
            if isinstance(v, dict):
                v = ", ".join(f"{a}={b}" for a, b in v.items())
            elif isinstance(v, list):
                v = f"{len(v)} rows"
            lines.append(f"{k:<10}{v}")
        return "\n".join(lines)


@register("transform", "one_per_user")
class OnePerUser(Plugin):
    """Keep the largest event of every user."""

    def apply(self, df, settings):
        out = df.sort_values(["amount", "id"], ascending=[False, True]).drop_duplicates("user")
        return out.sort_values("id").reset_index(drop=True)


TEXTS = {
    "csv_a": """id,user,amount,region,ts
1,Ana,12.5,north,2024-03-01
2,bo,7,south,2024-03-01
3,ana,abc,north,2024-03-02
,cy,3,east,2024-03-02
4, Cy ,20,,2024-03-02
2,bo,7,south,2024-03-01
""",
    "jsonl_a": """{"id": 10, "user": "dee", "amount": 5, "region": "south", "ts": "2024-03-01T09:00", "tags": ["web"]}
# replayed from the edge
{"id": 11, "user": "Ana", "ts": "2024-03-02T11:30", "tags": ["app", "web"]}
not json
{"id": 12, "user": "bo", "amount": -4, "ts": "2024-03-03"}
""",
    "log_a": """2024-03-01T10:00 INFO user=bo id=20 amount=30 region=west
2024-03-02T08:15 DEBUG user=ana id=21 amount=1
2024-03-02T09:00 WARN user=dee id=22 amount=-2.5
garbage line
2024-03-03T12:00 INFO user=Ana id=23 amount=8.25 region=north
""",
    "log_b": """2024-04-01T08:00 INFO user=eve id=30 amount=10 region=north
2024-04-01T09:30 INFO user=bo id=31 amount=4 region=south
2024-04-02T10:00 WARN user=eve id=32 amount=6
""",
    "csv_b": """id,user,amount,region,ts
40,fay,15,north,2024-04-02
41,  ,2,south,2024-04-02
42,bo,9,south,2024-04-03
""",
}

CONFIGS = {
    "alpha": '{"name": "alpha", "sources": ["csv", "ndjson", "log"], "tags": {"team": "data", "tier": "gold"}, "export": {"indent": 0}}',
    "beta": '{"name": "beta", "sources": ["log", "comma"], "strict": true, "owner": "ops"}',
    "gamma": '{"sources": ["csv", "xml"], "batch_size": 0, "export": {"format": "xml"}}',
}

PLAN_TEXT = """{
  "profiles": [
    {"user": "ana", "tier": "gold", "since": 2019},
    {"user": "bo", "tier": "silver", "since": 2021},
    {"user": "eve", "tier": "gold", "since": 2020}
  ],
  "scenarios": [
    {"label": "alpha", "config": "alpha", "inputs": {"csv": "csv_a", "jsonl": "jsonl_a", "log": "log_a"}, "transforms": ["threshold", "convert"]},
    {"label": "beta", "config": "beta", "env": {"DP_CURRENCY": "nok", "DP_EXPORT_FORMAT": "text", "DP_THRESHOLD": "5"}, "inputs": {"log": "log_b", "csv": "csv_b"}},
    {"label": "gamma", "config": "gamma", "env": {"DP_CURRENCY": "euro"}, "inputs": {},
     "expect": [["sources", "value_error"], ["currency", "value_error"], ["batch_size", "greater_than"], ["export.format", "literal_error"]]}
  ]
}"""


def run_scenario(spec, profiles):
    out = {"label": spec["label"], "ok": False}
    names = spec["inputs"]
    settings, errors = try_settings(CONFIGS[spec["config"]], spec.get("env"))
    if settings is None:
        out["errors"] = dict(errors)
        assert errors == [tuple(e) for e in spec["expect"]], errors  #@ sound expect
    else:
        inputs = {src: TEXTS[name] for src, name in names.items()}
        #@row settings: present={name,sources,currency,batch_size,strict,threshold,export,tags} definite={name,sources,currency,batch_size,strict,threshold,export,tags} closed
        pipe, rep, exported, line = run_pipeline(settings, inputs, profiles, spec.get("transforms", ["threshold"]))
        #@row rep: present={name,currency,n_events,n_rejected,n_kept,total,users,by_region,by_day,top,pivot,errors,tags} definite={name,currency,n_events,n_rejected,n_kept,total,users,by_region,by_day,top,pivot,errors} closed
        #@row exported: present={format,exporter,chars,text} definite={format,exporter,chars,text} closed
        out.update(ok=True, settings=settings, pipe=pipe, report=rep, export=exported, summary=line)
    _ = 0  #@row out: present={label,ok,errors,settings,pipe,report,export,summary} definite={label,ok} closed
    return out


if __name__ == "__main__":
    os.environ["DP_CURRENCY"] = "usd"
    os.environ["DP_EXPORT_FORMAT"] = "csv"
    plan = json.loads(PLAN_TEXT)
    # (first written as schemas of json.loads@L125: a literal text is no input, rowan requires
    # nothing of it — the plan's keys are known)
    profiles = plan["profiles"]
    outcomes = {}
    for spec in plan["scenarios"]:
        outcomes[spec["label"]] = run_scenario(spec, profiles)

    # ---------------------------------------------------------------- registry
    kinds = REGISTRY.kinds()
    n_transforms = kinds["transform"]                   #@value n_transforms: exactly={4}
    n_exporters = kinds["exporter"]
    version = dp.VERSION
    try:
        register("source", "csv")(TextExporter)
    except ValueError:
        pass
    csv_cls = REGISTRY.resolve("source", "comma")
    csv_name = csv_cls.plugin_name
    probe = REGISTRY.create("exporter", "text", width=10)
    probe_info = probe.info()
    probe_kind = probe_info["kind"]

    # ---------------------------------------------------------------- settings layer
    env1 = env_layer({"DP_EXPORT_FORMAT": "json", "DP_EXPORT_INDENT": "4", "HOME": "/root"})
    env1_export = env1["export"]                        #@row env1_export: present={format,indent} definite={format,indent} closed
    merged_b = merged_config(CONFIGS["beta"], {"DP_STRICT": "0"})
    #@row merged_b: present={name,sources,currency,batch_size,strict,threshold,export,tags,owner} definite={name,sources,currency,batch_size,strict,threshold,export,tags,owner} closed
    owner = merged_b["owner"]
    strict_b = merged_b["strict"]
    merged_export = merged_b["export"]

    alpha = outcomes["alpha"]
    beta = outcomes["beta"]
    gamma = outcomes["gamma"]
    #@row gamma: present={label,ok,errors} definite={label,ok,errors} closed
    gamma_errors = gamma["errors"]                      #@row gamma_errors: present={sources,currency,batch_size,"export.format"} definite={sources,currency,batch_size,"export.format"} closed
    batch_err = gamma_errors["batch_size"]
    settings_a = alpha["settings"]
    desc_a = settings_a.describe()                      #@value desc_a: exactly={"alpha [csv+jsonl+log] USD -> csv"}
    desc_b = beta["settings"].describe()
    export_a = settings_a.export                        #@row export_a: present={format,indent,sort_keys} definite={format,indent,sort_keys} closed
    indent_a = export_a.indent
    threshold_b = beta["settings"].threshold

    # ---------------------------------------------------------------- ingestion
    pipe_a = alpha["pipe"]
    #@row pipe_a: present={settings,registry,sources,batches,events,rejected,log,frame} definite={settings,registry,sources,batches,events,rejected,log,frame} closed
    batches_a = pipe_a.batches                          #@row batches_a: present={csv,jsonl,log} definite={csv,jsonl,log} closed
    csv_src = pipe_a.sources["csv"]
    csv_seen = csv_src.seen                             #@value csv_seen: exactly={6}
    csv_err = csv_src.errors[0]
    csv_err_line = csv_err["line"]
    rec_csv = batches_a["csv"][3]                       #@row rec_csv: present={id,user,amount,region,ts,source,seq} definite={id,user,amount,region,ts,source,seq} closed
    rec_csv_region = rec_csv["region"]
    rec_csv_seq = rec_csv["seq"]
    rec_json = batches_a["jsonl"][2]
    rec_json_tags = batches_a["jsonl"][1]               #@row rec_json_tags: present={id,user,amount,region,ts,tags,source,seq} definite={id,user,amount,region,ts,tags,source,seq} closed
    # (a batch is the records cut at a size the environment may set: item 1, where there is one)
    #@schema param JsonLinesSource.normalize.obj: always={id,user,ts} only={id,user,ts}
    json_err = pipe_a.sources["jsonl"].errors[0]["error"]  #@value json_err: exactly={"bad json: Expecting value"}
    rec_log = batches_a["log"][1]                       #@row rec_log: present={id,user,amount,ts,region,level,source,seq} definite={id,user,amount,ts,region,level,source,seq} closed
    rec_log_level = rec_log["level"]
    parsed = parse_log_line("2024-03-02T08:15 DEBUG user=ana id=21 amount=1")
    #@row parsed: present={ts,level,user,id,amount,region} definite={ts,level,user,id,amount,region} closed
    parsed_region = parsed["region"]
    mix = source_mix(batches_a)                         #@row mix: present={csv,jsonl,log} definite={csv,jsonl,log} closed
    mix_csv = mix["csv"]
    share_csv = mix_csv["share"]                        #@value share_csv: includes={0.455}
    # (not exactly 0.455, a to-do before: alpha's batch size may come from os.environ)

    # ---------------------------------------------------------------- validation
    events_a = pipe_a.events
    ev0 = events_a[0]                                   #@row ev0: present={id,user,amount,region,ts,source,seq,level,tags} definite={id,user,amount,region,ts,source,seq,level,tags} closed
    ev0_user = ev0.user
    ev_ids = first_ids(events_a, 4)
    n_events_a = len(events_a)
    rej0 = pipe_a.rejected[0]
    rej0_err = rej0.first()                             #@row rej0_err: present={loc,type,msg} definite={loc,type,msg} closed
    rej0_type = rej0_err["type"]                        #@todo value rej0_type: exactly={"float_parsing"}
    #@value rej0_type: includes={"float_parsing"}
    rej1_loc = pipe_a.rejected[1].errors[0]["loc"]
    tags_a = all_tags(events_a)
    src_counts = merge_counts(counts_by(events_a, "source"), counts_by(beta["pipe"].events, "source"))
    #@todo row src_counts: present={csv,jsonl,log} definite={csv,log} closed  (corrected from definite jsonl: a DP_STRICT in os.environ ends alpha's validation at csv's bad amount)
    #@row src_counts: possible={csv,jsonl,log}
    log_count = src_counts["log"]
    rej_b = beta["pipe"].rejected[0].errors[0]["loc"]
    totals_b = running_totals(beta["pipe"].events)
    last_total_b = totals_b[-1]                         #@value last_total_b: exactly={35.0}
    # (validation decides each record: `ev.key() in seen` of a set of tuples)

    # ---------------------------------------------------------------- processing
    frame_a = pipe_a.frame                              #@row frame_a: present={id,user,amount,region,ts,source,day,currency} definite={id,user,amount,region,ts,source,day,currency} closed
    frame_b = beta["pipe"].frame                        #@row frame_b: present={id,user,amount,region,ts,source,day} definite={id,user,amount,region,ts,source,day} absent={currency} closed
    day_b = daily(frame_b)
    rep_a = alpha["report"]
    rep_b = beta["report"]
    tags_rep = rep_a["tags"]
    try:
        rep_b["tags"]                                   #@ missing tags
    except KeyError:
        pass
    user0 = rep_a["users"][0]                           #@row user0: present={user,n,total,regions,tier,since} definite={user,n,total,regions,tier,since} closed
    user0_tier = user0["tier"]
    total_a = rep_a["total"]                            #@value total_a: includes={124.12}
    # (not exactly 124.12, a to-do before: DP_BATCH_SIZE / DP_THRESHOLD may be in os.environ)
    by_region_a = rep_a["by_region"]                    #@todo row by_region_a: present={north,south,unknown,west} definite={north,south} closed  (corrected: a batch size of 1 from os.environ leaves out unknown, DP_STRICT west)
    #@row by_region_a: present={north,south,unknown,west} closed
    unknown_a = by_region_a["unknown"]
    by_region_b = rep_b["by_region"]                    #@row by_region_b: present={north,south,unknown} definite={north,south,unknown} absent={west} closed
    # (beta's environment is the scenario's own: every value is known)
    by_day_a = rep_a["by_day"]                          #@todo row by_day_a: present={"2024-03-01","2024-03-02","2024-03-03"} definite={"2024-03-01"} closed  (corrected: a batch size of 1 from os.environ keeps the first day only)
    #@row by_day_a: possible={"2024-03-01","2024-03-02","2024-03-03"}
    day1 = by_day_a["2024-03-01"]
    day1_count = day1["count"]
    by_day_b = rep_b["by_day"]
    pivot_a = rep_a["pivot"]                            #@todo row pivot_a: present={csv,jsonl,log} definite={} closed  (corrected: a DP_THRESHOLD from os.environ may drop every row)
    #@row pivot_a: possible={csv,jsonl,log}
    pivot_b = rep_b["pivot"]                            #@todo row pivot_b: present={csv,log} definite={csv,log} absent={jsonl} closed
    #@row pivot_b: possible={csv,log}
    pivot_b_log = pivot_b["log"]
    top_a = rep_a["top"]
    top_b0 = rep_b["top"][0]
    errors_a = rep_a["errors"]                          #@todo row errors_a: present={"csv:float_parsing","jsonl:value_error"} definite={} closed  (corrected: a batch size under 3 from os.environ rejects nothing)
    #@row errors_a: possible={"csv:float_parsing","jsonl:value_error"}

    # ---------------------------------------------------------------- export
    exp_a = alpha["export"]
    chars_a = exp_a["chars"]                            #@value chars_a: includes={119}
    # (not exactly 119, a to-do before: alpha's report depends on os.environ)
    exporter_a = exp_a["exporter"]
    exp_b = beta["export"]
    chars_b = exp_b["chars"]                            #@todo value chars_b: exactly={104}
    #@value chars_b: includes={104}
    exporter_b = exp_b["exporter"]
    #@schema param CsvExporter.render.payload: always={rows} only={rows}
    #@schema param TextExporter.render.payload: only={}
    #@schema param summary_line.report: always={name,n_events,n_rejected,total,currency} only={name,n_events,n_rejected,total,currency}
    #@schema param export_payload.report: always={name,users,by_region,errors} only={name,users,by_region,errors}
    round_trip = json.loads(dp.export.JsonExporter().render({"b": 1, "a": [rep_b["n_kept"]]}, export_a))

    # ---------------------------------------------------------------- HTTP API
    STATE["profiles"] = profiles
    client = TestClient(app)
    plugins = client.get("/plugins").json()             #@row plugins: present={source,transform,exporter} definite={source,transform,exporter} closed
    desc0 = client.get("/plugins/source").json()[0]     #@row desc0: present={name,cls,doc} definite={name,cls,doc} closed
    desc0_doc = desc0["doc"]
    no_kind = client.get("/plugins/widgets").status_code
    body = {"config": CONFIGS["alpha"], "inputs": {"csv": TEXTS["csv_a"], "log": TEXTS["log_b"]}, "transforms": ["threshold", "flag"]}
    web = client.post("/runs/web", json=body).json()    #@row web: present={name,events,rejected,kept,sources} definite={name,events,rejected,kept,sources} closed
    web_events = web["events"]
    picked = client.get("/runs/web", params={"fields": "name,total,nope"}).json()
    #@row picked: present={name,total} definite={name,total} closed
    picked_total = picked["total"]
    full = client.get("/runs/web").json()
    #@row full: present={name,currency,n_events,n_rejected,n_kept,total,by_region,by_day,top,pivot,errors,tags} definite={name,currency,n_events,n_rejected,n_kept,total,by_region,by_day,top,pivot,errors,tags} closed
    full_currency = full["currency"]
    missing_run = client.get("/runs/zzz").status_code
    bad = client.post("/runs/bad", json={"config": CONFIGS["gamma"]})
    bad_status = bad.status_code
    bad_detail = bad.json()["detail"]
    n_bad = len(bad_detail)                             #@value n_bad: exactly={3}
    bad_t = client.post("/runs/t", json={"inputs": {"csv": TEXTS["csv_b"]}, "transforms": ["nope"]}).json()["detail"]
    #@value bad_t: exactly={"no transform plugin named nope"}
    one = client.post("/runs/one", json={"inputs": {"csv": TEXTS["csv_a"]}, "transforms": ["one_per_user"]}).json()
    one_kept = one["kept"]                              #@todo value one_kept: exactly={3}
    #@value one_kept: includes={3}
    csv_out = client.post("/runs/web/export", json={"format": "csv"}).json()
    #@row csv_out: present={name,format,lines,text} definite={name,format,lines,text} closed
    csv_lines = csv_out["lines"]
    text_out = client.post("/runs/web/export", json={"format": "text", "sort_keys": False}).json()
    text_lines = text_out["lines"]
    xml_status = client.post("/runs/web/export", json={"format": "xml"}).status_code
    try:
        client.post("/runs/web/export", json={"indent": 4})  #@ missing format
    except KeyError:
        pass
    #@schema param export_run.payload: always={format} only={format}
    calls = STATE["calls"]                              #@value calls: exactly={11}
