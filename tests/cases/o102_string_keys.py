"""Keys built from strings: partition, split, formatting with widths, join, prefixes
through a parameter, and a table keyed by the result.

Expectations were written from the program text before running the engine."""


def parse_ref(ref):
    kind, _, ident = ref.partition(":")
    return {"kind": kind, "ident": ident}


def make_key(kind, n):
    return "%s-%03d" % (kind, n)


def index(refs):
    out = {}
    for ref in refs:
        p = parse_ref(ref)
        out[p["kind"]] = out.get(p["kind"], 0) + 1
    return out


p = parse_ref("lot:L001")                 #@row p: present={kind,ident} definite={kind,ident} closed
table = {make_key("lot", 1): {"n": 1}, make_key("box", 12): {"n": 12}}  #@row table: present={lot-001,box-012} definite={lot-001,box-012} closed
counts = index(["lot:1", "lot:2", "box:3"])  #@row counts: present={lot,box} definite={lot,box} closed
parts = "a.b.c".split(".")
joined = {"/".join(parts): True}          #@row joined: present={a/b/c} definite={a/b/c} closed
padded = {"%-4s|" % "ab": 1, "%05.1f|" % 3.14159: 2}  #@row padded: present={ab  |,003.1|} definite={ab  |,003.1|} closed
table["lot-001"]                          #@ ok lot-001
table["lot-1"]                            #@ missing lot-1
counts["crate"]                           #@ missing crate
