"""Tasks from a literal list processed by worker objects chosen round-robin in a while loop
over a pending list; results keyed by task id; per-worker done lists.

Expectations were written from the program text before running the engine."""


class Worker:
    def __init__(self, name):
        self.name = name
        self.done = []

    def handle(self, task):
        res = {"task": task["id"], "worker": self.name}
        if task.get("retry"):
            res["retried"] = True
        self.done.append(res)
        return res


TASKS = [{"id": "t1"}, {"id": "t2", "retry": 1}, {"id": "t3"}]


def run(tasks, workers):
    pending = list(tasks)
    results = {}
    i = 0
    while pending:
        task = pending.pop(0)
        w = workers[i % len(workers)]
        results[task["id"]] = w.handle(task)
        i += 1
    return results


ws = [Worker("a"), Worker("b")]
# Corrected on re-reading (r140 unrolls `while` loops whose test is decided at every head): `pending` is a list of
# known length popped once per iteration, so every task is handled and t1 (no retry) has no
# "retried".
res = run(TASKS, ws)                      #@row res: present={t1,t2,t3} definite={t1,t2,t3} closed
r1 = res["t1"]                            #@row r1: present={task,worker} definite={task,worker} closed
wa = ws[0]                                #@row wa: present={name,done} definite={name,done} closed
done0 = wa.done[0]                        #@row done0: present={task,worker} definite={task,worker} closed
res["t4"]                                 #@ missing t4
r1["worker"]                              #@ ok worker
