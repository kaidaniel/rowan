import os


def make(tag, n):
    for i in range(n):
        d = {"tag": tag}
        if i >= 0:
            return d
    return {"none": 1}


def inner(tag, n):
    return make(tag, n)


def outer(tag, n):
    return inner(tag, n)


n = len(os.environ.get("ROWAN_CASE_N", "abcdefgh"))
a = outer("a", n)
b = outer("b", n)
t = a.get("tag")
#@value t: exactly={"a", None}
u = b.get("tag")
#@value u: exactly={"b", None}
