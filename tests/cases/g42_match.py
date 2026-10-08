#@engine rust
"""`match` statements: cases are joined (no pattern narrowing yet, spec §16)."""


def classify(evt):
    out = {"kind": "unknown"}
    match evt["type"]:                   #@ input type
        case "click":
            out["x"] = evt["x"]          #@ input x
        case "key":
            out["key"] = evt["key"]      #@ input key
        case _:
            out["other"] = True
    return out


r = classify(e)
r["kind"]                                #@ ok kind
r["x"]                                   #@ possibly x
r["other"]                               #@ possibly other
r["nope"]                                #@ missing nope
#@schema name e: always={type} sometimes={x,key}


def with_capture(cmd):
    match cmd:
        case {"action": "go", "to": dest}:
            return {"dest": dest}
        case [first, *_]:
            return {"first": first}
        case Point(x=px):
            return {"px": px}
        case _:
            return {}


w = with_capture(c)
w["dest"]                                #@ possibly dest
w["first"]                               #@ possibly first
w["none"]                                #@ missing none
