"""Hypothesis: `match` on a known literal subject runs exactly one case (literal, or-pattern,
wildcard); mapping patterns on a known dict pick the first matching case; an unknown subject
keeps every case possible.

Expectations were written from the program text before running the engine."""


def handle(cmd):
    out = {"cmd": cmd}
    match cmd:
        case "add":
            out["added"] = True
        case "del" | "rm":
            out["deleted"] = True
        case _:
            out["unknown"] = True
    return out


def kind(msg):
    match msg:
        case {"type": "point", "x": _}:
            return {"point": True}
        case {"type": _}:
            return {"typed": True}
    return {"other": True}


a = handle("add")                            #@row a: present={cmd,added} definite={cmd,added} closed
r = handle("rm")                             #@row r: present={cmd,deleted} definite={cmd,deleted} closed
z = handle("zz")                             #@row z: present={cmd,unknown} definite={cmd,unknown} closed
a["deleted"]                                 #@ missing deleted
u = handle(cmd_in)                           #@row u: present={cmd,added,deleted,unknown} definite={cmd} closed
p = kind({"type": "point", "x": 1})          #@row p: present={point} definite={point} closed
t = kind({"type": "line"})                   #@row t: present={typed} definite={typed} closed
o = kind({})                                 #@row o: present={other} definite={other} closed
