"""In-memory task store with sequential ids; creating and completing fire the hooks."""
from collections import Counter
from datetime import date

from tasks.hooks import fire
from tasks.models import Status, Task

TODAY = date(2024, 6, 1)


class TaskStore:
    def __init__(self):
        self.tasks = {}
        self.seq = 0
        self.events = Counter()

    def create(self, data):
        self.seq += 1
        task = Task(id=self.seq, **data.model_dump())
        #@row task: present={title,priority,due,labels,id,status,closed_by} definite={title,priority,due,labels,id,status,closed_by} closed
        self.tasks[task.id] = task
        self.events["created"] += 1
        return task, fire("created", task, {"today": TODAY})

    def complete(self, tid, user):
        task = self.tasks[tid]
        if task.status is Status.DONE:
            return None
        task.status = Status.DONE
        task.closed_by = user
        self.events["completed"] += 1
        return fire("completed", task, {"today": TODAY, "user": user})

    def stats(self):
        by_status = Counter(t.status.value for t in self.tasks.values())
        return {"total": len(self.tasks), "by_status": dict(by_status), "events": dict(self.events)}
