# a job runner: specs validated with executor.map, run with submit + add_done_callback (results gathered
# under a lock), a contextmanager tracking each job, ExitStack cleanups, suppress, redirect_stdout for a
# dry run, as_completed, and a monitor thread counting kinds

import io
import threading
from concurrent.futures import ThreadPoolExecutor, as_completed
from contextlib import ExitStack, contextmanager, redirect_stdout, suppress

TASKS = {}


def task(kind):
    def register(fn):
        TASKS[kind] = fn
        return fn
    return register


@task("sum")
def task_sum(args):
    return {"total": sum(args["values"]), "n": len(args["values"])}     #@ possibly values
#@schema param task_sum.args: always={values} only={values}


@task("greet")
def task_greet(args):
    print(f"hello {args['who']}")                                       #@ ok who
    return {"greeting": "hello " + args["who"], "lang": args.get("lang", "en")}
#@schema param task_greet.args: always={who} only={who}


@task("lookup")
def task_lookup(args):
    table = {"a": 1, "b": 2}
    return {"key": args["key"], "value": table[args["key"]]}            #@ ok key ; possibly z
#@schema param task_lookup.args: always={key} only={key}


def validate(spec):
    problems = {}
    if spec.get("kind") not in TASKS:
        problems["kind"] = "unknown"
    if "args" not in spec:
        problems["args"] = "missing"
    out = {"name": spec["name"], "problems": problems}
    return out                                    #@row problems: present={kind,args} closed
#@schema param validate.spec: always={name} only={name}


@contextmanager
def tracked(registry, lock, name):
    entry = {"name": name, "state": "running"}
    with lock:
        registry[name] = entry
    try:
        yield entry
        entry["state"] = "done"
    except KeyError as e:
        entry["state"] = "failed"
        entry["missing"] = e.args[0]
        raise


def run_job(spec, registry, lock):
    fn = TASKS[spec["kind"]]
    with tracked(registry, lock, spec["name"]) as entry:
        value = fn(spec["args"])
        entry["keys"] = len(value)
    return {"job": spec["name"], "value": value}
#@schema param run_job.spec: always={kind,name,args} only={kind,name,args}


def run_all(specs, workers=2):
    registry = {}
    results = {}
    futures = {}
    lock = threading.Lock()

    def on_done(fut):
        name = futures[fut]
        with lock:
            if fut.exception() is None:
                results[name] = {"ok": True, **fut.result()}
            else:
                results[name] = {"ok": False, "error": repr(fut.exception())}

    with ThreadPoolExecutor(max_workers=workers) as ex:
        for spec in specs:
            fut = ex.submit(run_job, spec, registry, lock)
            futures[fut] = spec["name"]
            fut.add_done_callback(on_done)
    return results, registry


def monitor(stats, specs):
    for s in specs:
        stats[s["kind"]] = stats.get(s["kind"], 0) + 1


def kind_stats(specs):
    stats = {}
    t = threading.Thread(target=monitor, args=(stats, specs), daemon=True)
    t.start()
    t.join()
    return stats


def run_with_cleanup(specs):
    log = []
    with ExitStack() as stack:
        for s in specs:
            stack.callback(log.append, s["name"])
        with ThreadPoolExecutor(max_workers=2) as ex:
            checks = list(ex.map(validate, specs))
        valid = [s for s, c in zip(specs, checks) if not c["problems"]]
        results, registry = run_all(valid)
    return results, registry, log, checks


def first_total(results, name):
    with suppress(KeyError):
        return results[name]["value"]["total"]
    return None


def dry_run(spec):
    buf = io.StringIO()
    with redirect_stdout(buf):
        value = TASKS[spec["kind"]](spec["args"])
    return value, buf.getvalue()


def quick_totals(specs):
    with ThreadPoolExecutor() as ex:
        futs = [ex.submit(task_sum, s["args"]) for s in specs]
        totals = sorted(f.result()["total"] for f in as_completed(futs))
    return totals


SPECS = [
    {"name": "j1", "kind": "sum", "args": {"values": [1, 2, 3]}},
    {"name": "j2", "kind": "greet", "args": {"who": "ann", "lang": "de"}},
    {"name": "j3", "kind": "lookup", "args": {"key": "z"}},
    {"name": "x1", "kind": "nope"},
    {"name": "x2", "kind": "sum", "args": {}},
]
SPECS_B = [{"name": "k1", "kind": "lookup", "args": {"key": "b"}, "retries": 2}]

tasks = TASKS                                      #@row tasks: present={sum,greet,lookup} definite={sum,greet,lookup} closed

if __name__ == "__main__":
    results, registry, log, checks = run_with_cleanup(SPECS)
    #@row results: present={j1,j2,j3,x2} definite_within={j1,j2,j3,x2} closed
    #@todo row results: definite={j1,j2,j3,x2}  (x2's KeyError, raised in a summarised task under a re-raising @contextmanager, reaches the future without its value)
    r1 = results["j1"]
    #@row r1: present={ok,job,value} definite={ok,job,value} closed
    v1 = r1["value"]                               #@row v1: present={total,n} definite={total,n} closed
    tot = v1["total"]                              #@value tot: exactly={6}
    r2v = results["j2"]["value"]                   #@row r2v: present={greeting,lang} definite={greeting,lang} closed
    lang = r2v["lang"]                             #@value lang: exactly={"de"}
    r3 = results["j3"]                             #@row r3: present={ok,error} definite={ok,error} closed
    err = r3["error"]                              #@value err: exactly={"KeyError('z')"}
    ok3 = r3["ok"]                                 #@value ok3: exactly={False}
    try:
        results["x1"]                              #@ missing x1
    except KeyError:
        pass
    e1 = registry["j1"]                            #@row e1: present={name,state,keys} definite={name,state,keys} closed
    st1 = e1["state"]                              #@value st1: exactly={"done"}
    e3 = registry["j3"]                            #@row e3: present={name,state,missing} definite={name,state,missing} closed
    miss = e3["missing"]                           #@value miss: exactly={"z"}
    order = "|".join(log)                          #@value order: exactly={"x2|x1|j3|j2|j1"}
    c3 = checks[3]["problems"]                     #@row c3: present={kind,args} definite={kind,args} closed
    c0 = checks[0]["problems"]                     #@row c0: present={} closed
    ft = first_total(results, "j1")                #@value ft: exactly={6}
    fn = first_total(results, "j3")                #@value fn: exactly={None}

    results_b, registry_b, log_b, _ = run_with_cleanup(SPECS_B)
    #@row results_b: present={k1} definite={k1} closed
    kb = results_b["k1"]["value"]                  #@row kb: present={key,value} definite={key,value} closed
    kv = kb["value"]                               #@value kv: exactly={2}

    stats = kind_stats(SPECS)                      #@row stats: present={sum,greet,lookup,nope} definite={sum,greet,lookup,nope} closed
    nsum = stats["sum"]                            #@value nsum: exactly={2}

    value, printed = dry_run(SPECS[1])             #@value printed: exactly={"hello ann\n"}
    #@row value: present={greeting,lang} definite={greeting,lang} closed
    totals = quick_totals([{"args": {"values": [5]}}, {"args": {"values": [1, 1]}}])
    t0 = totals[0]                                 #@todo value t0: exactly={2}  (as_completed's results come in any order: sorted() of them is not modelled)
