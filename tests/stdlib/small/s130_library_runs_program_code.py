"""Library calls that run the program's code: a `threading.Timer`'s function (unless it is
cancelled first), a `threading.Thread` subclass's `run` on `start()`, a thread pool's
`initializer` (when a worker starts), `pprint`'s printers (`__repr__`). These were taken to
have no effect."""
import concurrent.futures
import pprint
import threading


def work(d):
    del d["k"]


d1 = {"k": 1}
tm = threading.Timer(0.0, work, args=(d1,))
tm.start()
tm.join()
r1 = d1.get("k", "gone")
#@value r1: includes={"gone"}


class Worker(threading.Thread):
    def __init__(self, d):
        super().__init__()
        self.d = d

    def run(self):
        del self.d["k"]


d2 = {"k": 1}
w = Worker(d2)
w.start()
w.join()
r2 = d2.get("k", "gone")
#@value r2: exactly={"gone"}

d3 = {"k": 1}
with concurrent.futures.ThreadPoolExecutor(initializer=work, initargs=(d3,)) as ex:
    n = ex.submit(len, "ab").result()
r3 = d3.get("k", "gone")
#@value r3: includes={"gone"}

d4 = {"k": 1}


class R:
    def __repr__(self):
        d4.pop("k", None)
        return "R()"


pprint.pp(R())
r4 = d4.get("k", "gone")
#@value r4: exactly={"gone"}
