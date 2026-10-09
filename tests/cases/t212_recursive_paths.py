"""Hypothesis: a recursive walk over a known nested dict produces known dotted paths.

Expectations were written from the program text before running the engine."""


def paths(d, prefix=""):
    out = []
    for k, v in d.items():
        p = prefix + k
        if isinstance(v, dict):
            out.extend(paths(v, p + "."))
        else:
            out.append(p)
    return out


CFG = {"db": {"host": "h", "port": 1}, "debug": True}
flat = dict.fromkeys(paths(CFG))             #@row flat: present={db.host,db.port,debug} definite={db.host,db.port,debug} closed
