"""A read on an unknown that may be any object may raise into the try around it: an element
unpacked from what `json.loads` of unknown text gives, a `for` target over such a list, an
operator's result on unknowns. Each handler below runs in CPython (Z785_INPUT unset), so rowan
must reach it. (Found by the held-out corpus: jsonpatch's `RemoveOperation.apply`, whose
`del subobj[part]` handler was taken for unreachable for any call.)"""
import json
import os

ARG = os.environ.get("Z785_INPUT")


def first_key(text):
    rec, key = json.loads(text)
    try:
        v = rec[key]
    except KeyError:
        v = "missing"  #@value v: exactly={'missing'}
    return v


def remove(text):
    subobj, part = json.loads(text)
    try:
        del subobj[part]
    except (KeyError, IndexError):
        msg = "can't remove"  #@value msg: exactly={"can't remove"}
        return msg
    return "removed"


def tag(a, b):
    c = a + b
    try:
        t = c.tag
    except AttributeError:
        t = "none"  #@value t: exactly={'none'}
    return t


def xs_of(text):
    found = []
    for name, rec in json.loads(text):
        try:
            found.append(rec["x"])
        except KeyError:
            seen = "no x"  #@value seen: exactly={'no x'}
            found.append(seen)
    return found


def remove_any(text):
    # (its run for any call reaches the handler too)
    subobj, part = json.loads(text)
    try:
        del subobj[part]
    except (KeyError, IndexError):
        gone = "gone"  #@general gone: exactly={'gone'}
        return gone
    return None


out = first_key(ARG or '[{"a": 1}, "b"]')  #@value out: includes={'missing'}
r = remove(ARG or "[[1, 2], 5]")  #@value r: includes={"can't remove"}
pair = json.loads(ARG or "[1, 2]")
t = tag(pair[0], pair[1])  #@value t: includes={'none'}
xs = xs_of(ARG or '[["a", {"y": 1}]]')
g = remove_any(ARG or '[{}, "k"]')
