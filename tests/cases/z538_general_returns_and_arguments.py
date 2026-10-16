# A function's value for any call (`#@general`) starts after any of the program's functions ran.
# What they may change of a global is read from their code: a global one of them returns changes
# only where the code that gets the result changes it (its callers, when the function is only
# ever called by name), and a global given to one of the program's functions changes only as
# that function's code changes its parameter.
import random
import sys

USERS = {1: {"id": 1, "name": "ann"}, 2: {"id": 2, "name": "bob"}}


def find_user(uid):
    return USERS.get(uid)


def name_of(uid):
    u = find_user(uid)
    if u is None:
        return ""
    return u["name"]


def first_id():
    n = USERS[1]["id"]
    # (find_user's callers only read the record it returns)
    #@value n: exactly={1}
    #@general n: exactly={1}
    return n


SIZES = [8, 16, 32]


def total(sizes):
    t = 0
    for s in sizes:
        t += s
    return t


def run():
    return total(SIZES)


def widths():
    w = SIZES[0]
    # (total only iterates what it is given)
    #@value w: exactly={8}
    #@general w: exactly={8}
    return w


REG = {"a": {"on": True}}


def entry(k):
    return REG[k]


def disable(k):
    entry(k)["on"] = False


def is_on():
    v = REG["a"]["on"]
    # (disable changes the record entry returns: it may have run)
    #@value v: exactly={True}
    #@general v: includes={True, False}
    return v


ORDER = [3, 1, 2]


def order():
    return ORDER


def add(x):
    order().append(x)


def shuffled():
    random.shuffle(order())


def last():
    z = ORDER[-1]
    # (add appends anything to what order returns)
    #@value z: exactly={2}
    #@general z: unknown
    return z


DATA = {"x": 1}


def lookup():
    return DATA


HOOKS = [lookup]


def poke():
    for f in HOOKS:
        f()["x"] = 2


def peek():
    x = DATA["x"]
    # (lookup is called through a list: whoever calls it may change what it returns)
    #@value x: exactly={1}
    #@general x: includes={1, 2}
    return x


TABLE = {"k": 0}


def ident(d):
    return d


def bump():
    ident(TABLE)["k"] = 5


def table_k():
    k = TABLE["k"]
    # (ident gives back what it is given: bump changes TABLE through it)
    #@value k: exactly={0}
    #@general k: includes={0, 5}
    return k


ITEMS = {1: {"name": "pen"}}
PEN = ITEMS[1]


def get_item(iid):
    return ITEMS.get(iid, {})


GETTERS = [get_item]


def rename():
    for g in GETTERS:
        g(1)["name"] = "cup"


def pen_name():
    p = PEN["name"]
    # (get_item is called through a list: its callers may change the record it returns)
    #@value p: exactly={"pen"}
    #@general p: includes={"pen", "cup"}
    return p


TEAMS = {"a": {"members": ["ann"]}}
TEAM_A = TEAMS["a"]
MEMBERS_A = TEAM_A["members"]


def team(k):
    return TEAMS.get(k)


def join(k, who):
    t = team(k)
    t["members"].append(who)


def team_sizes():
    keys = len(TEAM_A)
    n = len(MEMBERS_A)
    # (join changes what the record team returns holds, not the record's own keys)
    #@value keys: exactly={1}
    #@general keys: exactly={1}
    #@value n: exactly={1}
    #@general n: unknown
    return keys, n


CONF = {"mode": "a"}


def conf():
    return CONF


def via_module():
    me = sys.modules[__name__]
    me.conf()["mode"] = "b"


def mode():
    md = CONF["mode"]
    # (via_module changes what conf returns: it calls conf through the module object)
    #@value md: exactly={"a"}
    #@general md: includes={"a", "b"}
    return md


if __name__ == "__main__":
    assert name_of(1) == "ann" and first_id() == 1
    assert run() == 56 and widths() == 8
    assert is_on()
    assert last() == 2
    assert peek() == 1
    assert table_k() == 0
    assert mode() == "a"
    assert pen_name() == "pen"
    assert team_sizes() == (1, 1)
