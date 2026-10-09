# ThreadPoolExecutor.submit(f, d).result(): the worker runs with the submitted dict; result() is its return value

from concurrent.futures import ThreadPoolExecutor


def enrich(job):
    out = {"id": job["id"], "status": "done"}
    if job.get("priority", 0) > 5:
        out["urgent"] = True
    return out                                    #@row out: present={id,status,urgent} definite={id,status} closed
#@schema param enrich.job: always={id} only={id}


def run_all(jobs):
    with ThreadPoolExecutor(max_workers=2) as ex:
        futs = [ex.submit(enrich, j) for j in jobs]
        results = [f.result() for f in futs]
    return results


def fail_safe(jobs):
    with ThreadPoolExecutor() as ex:
        fut = ex.submit(enrich, jobs[0])
        try:
            res = fut.result()
        except KeyError:
            res = {"id": None, "status": "failed"}
    n = len(res)                                  #@row res: present={id,status} definite={id,status} closed
    return res


rs = run_all([{"id": 1, "priority": 9}, {"id": 2}])
r0 = rs[0]                                      #@row r0: present={id,status,urgent} definite={id,status,urgent} closed
r1 = rs[1]                                      #@row r1: present={id,status} definite={id,status} closed
st = r1["status"]                               #@value st: exactly={"done"}
f1 = fail_safe([{"id": 3, "priority": 1}])      #@row f1: present={id,status} definite={id,status} closed
f2 = fail_safe([{"name": "x"}])                 #@row f2: present={id,status} definite={id,status} closed
fs = f2["status"]                               #@value fs: exactly={"failed"}
try:
    r1["urgent"]                                #@ missing urgent
except KeyError:
    pass
