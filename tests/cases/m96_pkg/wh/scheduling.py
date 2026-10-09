"""A day schedule: tasks in slots, a priority queue built from dicts, generator pipelines
over the tasks, a context-manager timer that records into a dict, and a daily digest."""
from collections import defaultdict

SLOTS = ("morning", "midday", "afternoon")

PRIORITY = {"rush": 0, "normal": 1, "low": 2}


class Timer:
    def __init__(self, store, name):
        self.store = store
        self.name = name

    def __enter__(self):
        self.store[self.name] = {"started": True}
        return self

    def __exit__(self, exc_type, exc, tb):
        self.store[self.name]["done"] = True
        if exc_type is not None:
            self.store[self.name]["failed"] = True
        return False


class Task:
    def __init__(self, kind, ref, priority="normal", slot=None):
        self.kind = kind
        self.ref = ref
        self.priority = priority
        self.slot = slot
        self.state = "todo"
        self.result = None

    def rank(self):
        return PRIORITY.get(self.priority, 9)

    def to_dict(self):
        d = {"kind": self.kind, "ref": self.ref, "priority": self.priority, "state": self.state}
        if self.slot is not None:
            d["slot"] = self.slot
        if self.result is not None:
            d["result"] = self.result
        return d


class Schedule:
    def __init__(self, cfg):
        self.cfg = cfg
        self.tasks = []
        self.slots = {slot: [] for slot in SLOTS}
        self.timings = {}
        self.done = []

    def add(self, task):
        self.tasks.append(task)
        slot = task.slot or SLOTS[0]
        self.slots[slot].append(task)
        return task

    def queue(self):
        return sorted(self.tasks, key=lambda t: (t.rank(), t.ref))

    def pending(self):
        for t in self.queue():
            if t.state == "todo":
                yield t

    def of_kind(self, kind):
        for t in self.pending():
            if t.kind == kind:
                yield t

    def run(self, handlers):
        for t in list(self.pending()):
            handler = handlers.get(t.kind)
            if handler is None:
                t.state = "skipped"
                continue
            with Timer(self.timings, t.kind + ":" + t.ref):
                t.result = handler(t)
            t.state = "done"
            self.done.append(t.ref)
        return self.digest()

    def digest(self):
        by_state = defaultdict(int)
        by_slot = {}
        for t in self.tasks:
            by_state[t.state] += 1
        for slot, tasks in self.slots.items():
            if tasks:
                by_slot[slot] = len(tasks)
        d = {"tasks": len(self.tasks), "by_state": dict(by_state), "by_slot": by_slot, "done": list(self.done)}
        if self.timings:
            d["timed"] = sorted(self.timings)
        return d


def plan_day(cfg, receipts, orders):
    sched = Schedule(cfg)
    for ref in receipts:
        sched.add(Task("receive", ref, "normal", "morning"))
    for order in orders:
        sched.add(Task("pick", str(order["id"]), order["priority"], "midday"))
        sched.add(Task("ship", str(order["id"]), order["priority"], "afternoon"))
    return sched


def task_counts(sched):
    counts = {}
    for t in sched.tasks:
        counts[t.kind] = counts.get(t.kind, 0) + 1
    return counts
