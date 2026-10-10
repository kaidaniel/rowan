"""Models built inside a store method called from several places (directly and from a loop in
another function): each construction keeps its own input and values (`**data.model_dump()`
spread, a mutable list default copied per instance, a list field re-validated from a dump)."""
from pydantic import BaseModel


class TaskIn(BaseModel):
    title: str
    labels: list[str] = []


class Task(TaskIn):
    id: int


class Store:
    def __init__(self):
        self.tasks = {}
        self.seq = 0

    def create(self, data):
        self.seq += 1
        task = Task(id=self.seq, **data.model_dump())
        self.tasks[task.id] = task
        return task


STORE = Store()


def import_all(items):
    ids = []
    for raw in items:
        ids.append(STORE.create(TaskIn.model_validate(raw)).id)
    return ids


if __name__ == "__main__":
    first = STORE.create(TaskIn(title="renew cert", labels=["urgent"]))
    ids = tuple(import_all([{"title": "a"}, {"title": "b", "labels": ["x"]}]))   #@value ids: exactly={(2,3)}
    t1 = STORE.tasks[1]
    l1 = tuple(t1.labels)                        #@value l1: exactly={("urgent",)}
    t2 = STORE.tasks[2]
    title2 = t2.title                            #@value title2: exactly={"a"}
    l2 = tuple(t2.labels)                        #@value l2: exactly={()}
    t3 = STORE.tasks[3]
    title3 = t3.title                            #@value title3: exactly={"b"}
    l3 = tuple(t3.labels)                        #@value l3: exactly={("x",)}
    last = STORE.create(TaskIn(title="direct")).labels
    n = len(last)                                #@value n: exactly={0}
