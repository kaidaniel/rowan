"""Hypothesis: str.format_map looks up only the fields the template names: a plain dict
missing one raises KeyError carrying the field name (extra keys are ignored), a dict subclass
with __missing__ fills the field without inserting the key, a defaultdict inserts the missing
key on lookup; `{p[x]}` indexes the nested dict with the string "x". Exact text and key sets;
an environment record must have `name` and `age`."""
from collections import defaultdict
import json
import os


class Blank(dict):
    def __missing__(self, key):
        return "<" + key + ">"


def render(tpl, data):
    return tpl.format_map(data)


def safe_render(tpl, data):
    try:
        return render(tpl, data)
    except KeyError as e:
        return "missing:" + e.args[0]


def with_default(tpl, data):
    dd = defaultdict(lambda: "?", data)
    text = render(tpl, dd)
    return text, dd


if __name__ == "__main__":
    s1 = safe_render("{a}-{b}", {"a": 1, "c": 2})    #@value s1: exactly={"missing:b"}
    s2 = safe_render("{a}/{a}", {"a": "x", "z": 0})  #@value s2: exactly={"x/x"}
    bl = Blank({"a": 1})
    s3 = render("{a}{q}", bl)                    #@value s3: exactly={"1<q>"}
    #@row bl: present={a} definite={a} absent={q} closed
    s4, dd = with_default("{u}:{v}", {"u": "U"})
    #@value s4: exactly={"U:?"}
    #@row dd: present={u,v} definite={u,v} closed
    s5 = render("{p[x]}!", {"p": {"x": 7}})      #@value s5: exactly={"7!"}
    data = json.loads(os.environ.get("S125_D", '{"name": "kim", "age": 3, "pet": "cat"}'))
    #@schema json.loads@L43: always={name,age} only={name,age}
    s6 = render("{name} ({age})", data)
    t6 = s6.endswith(")")                        #@value t6: includes={True}
    #@todo value t6: exactly={True}  (the template's literal suffix after the last field)
