# A global dict or list the program's functions change in place (`REG[k] = v`, `del REG[k]`,
# `REG.pop(k)`, `LOG.append(x)`) changes its items, not the objects they are: those keep their
# values for any call; one the code takes out and changes (`TASKS.pop()["done"] = True`) does not.
# One only iterated (`for r in ROWS: if r["id"] > 0: …`) keeps its records as they are.

REG = {"a": {"v": 1}, "b": {"v": 2}}
FIRST = REG["a"]


def register(name, meta):
    REG[name] = meta


def drop(name):
    del REG[name]
    REG.pop("b", None)


def first_v():
    v = FIRST["v"]
    # (register / drop change REG's items: the record FIRST names is as it was)
    #@value v: exactly={1}
    #@general v: exactly={1}
    return v


def any_v():
    w = REG["a"]["v"]  #@ ok a
    # (verdicts are the program's calls': drop never runs; for any call REG["a"] may be anything)
    #@general w: unknown
    return w


LOG = [{"n": 0}]
ENTRY = LOG[0]


def log(x):
    LOG.append(x)


def entry_n():
    n = ENTRY["n"]
    #@value n: exactly={0}
    #@general n: exactly={0}
    return n


TASKS = [{"done": False}]
HEAD = TASKS[0]


def finish():
    t = TASKS.pop()
    t["done"] = True


def head_done():
    d = HEAD["done"]
    # (finish takes the task out of the list and changes it)
    #@value d: exactly={False}
    #@general d: includes={False, True}
    return d


ROWS = [{"id": 2, "tags": ["a"]}, {"id": 1, "tags": []}]


def positive():
    n = 0
    for r in ROWS:
        if r["id"] > 0:
            n += 1
    return n


def tagged():
    return len([r for r in ROWS if r["tags"]])


def first_tags():
    k = len(ROWS[0]["tags"])
    # (positive and tagged iterate ROWS and read its records)
    #@value k: exactly={1}
    #@general k: exactly={1}
    return k


if __name__ == "__main__":
    assert first_v() == 1 and any_v() == 1
    assert entry_n() == 0
    assert head_done() is False
    assert first_tags() == 1
