"""A branch taken for one caller only: the key is possible at the join of the calls.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def describe(rec):
    out = {"id": rec["id"]}                  #@ ok id
    if "name" in rec:
        out["name"] = rec["name"]            #@ ok name
    _ = 0                                    #@row out: present={id,name} definite={id} closed
    return out


describe({"id": 1, "name": "n"})
describe({"id": 2})
