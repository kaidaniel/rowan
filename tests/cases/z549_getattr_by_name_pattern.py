"""getattr by a name known only in part (`f"run_{mode}"`, mode from the environment or "a")
is one of the attributes whose names fit (`run_a`, `run_b`), not only the one of the default:
calling it may run `run_b`, which sets `extra`. rowan took the known name's method or an
unknown callable that reaches nothing: `j.extra` was reported missing on every path.

Expectations were written from the program text before running the engine."""
import os

mode = os.environ.get("Z549_MODE", "a")


class Job:
    def __init__(self):
        self.n = 0

    def run_a(self):
        return 1

    def run_b(self):
        self.extra = 1
        return 2

    def stop(self):
        self.stopped = True

    def go(self):
        return getattr(self, f"run_{mode}")()


j = Job()
r = j.go()                               #@value r: includes={1, 2}
try:
    print(j.extra)                       #@ possibly extra
except AttributeError:
    pass
try:
    print(j.stopped)                     #@ missing stopped
except AttributeError:
    pass
