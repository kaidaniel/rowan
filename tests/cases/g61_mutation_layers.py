"""In-place mutation through several call layers and through containers / attributes."""


def set_a(d):
    d["a"] = 1


def set_b(d):
    set_a(d)
    d["b"] = 2


def set_all(d):
    set_b(d)
    d["c"] = 3
    return d


x = {}
set_all(x)
x["a"]                                   #@ ok a
x["b"]                                   #@ ok b
x["c"]                                   #@ ok c
x["d"]                                   #@ missing d


class Holder:
    def __init__(self):
        self.store = {"init": True}


def mutate_attr(h):
    h.store["mut"] = True


h = Holder()
mutate_attr(h)
h.store["mut"]                           #@ ok mut
h.store["init"]                          #@ ok init
h.store["nope"]                          #@ missing nope

bag = [{"i": 0}]


def touch_first(lst):
    lst[0]["touched"] = True


touch_first(bag)
bag[0]["touched"]                        #@ ok touched
bag[0]["x"]                              #@ missing x

alias_src = {"s": 1}
container = {"inner": alias_src}


def via_container(c):
    c["inner"]["via"] = True


via_container(container)
alias_src["via"]                         #@ ok via
alias_src["nope"]                        #@ missing nope


def maybe_set(d, flag):
    if flag:
        d["maybe"] = 1


m = {}
maybe_set(m, True)
m["maybe"]                               #@ ok maybe
m2 = {}
maybe_set(m2, cond)
m2["maybe"]                              #@ possibly maybe
# the access above did not fail, so from here on the key is assumed present (narrowing by
# access, as in a01); maybe_set(m2, False) is pruned and changes nothing
maybe_set(m2, False)
m2["maybe"]                              #@ ok maybe
m4 = {}
maybe_set(m4, False)
m4["maybe"]                              #@ missing maybe


def clear_and_fill(d):
    d.clear()
    d["fresh"] = 1


cf = {"old": 1}
clear_and_fill(cf)
cf["old"]                                #@ missing old
cf["fresh"]                              #@ ok fresh


def pop_it(d):
    return d.pop("gone")


pi = {"gone": 1, "stays": 2}
pop_it(pi)                               #@ ok gone
pi["gone"]                               #@ missing gone
pi["stays"]                              #@ ok stays


def swap_keys(d):
    d["new"] = d.pop("old")
    return d


sw = swap_keys({"old": 1})
sw["new"]                                #@ ok new
sw["old"]                                #@ missing old
