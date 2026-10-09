# threading.Thread(target=f, args=(d,), kwargs=…): the target mutates the dict it was given; join() makes it visible

import threading

LOCK = threading.Lock()


def worker(state, *, tag):
    with LOCK:
        state["seen"] = state.get("seen", 0) + 1
        state[tag] = True


def run(state, tags):
    threads = [threading.Thread(target=worker, args=(state,), kwargs={"tag": t}) for t in tags]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    return state


def fetch(src, out):
    out["value"] = src["value"] * 2               #@ possibly value
#@schema param fetch.src: always={value} only={value}


def fetch_in_thread(src):
    out = {}
    t = threading.Thread(target=fetch, args=(src, out))
    t.start()
    t.join()
    return out


s = run({"name": "x"}, ["a", "b"])               #@row s: present={name,seen,a,b} definite={name,seen,a,b} closed
seen = s["seen"]                                 #@value seen: exactly={2}
e = run({}, [])                                  #@row e: present={} closed
r = fetch_in_thread({"value": 4})                #@row r: present={value} definite={value} closed
v = r["value"]                                   #@value v: exactly={8}
threading.excepthook = lambda args: None
r2 = fetch_in_thread({"other": 1})               #@row r2: present={} closed
