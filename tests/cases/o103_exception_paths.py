"""Exception paths around record access: a lookup wrapped in try/except with a known key,
a validator that raises for one input and not another, finally blocks, and a result that
depends on which path ran.

Expectations were written from the program text before running the engine."""

CONFIG = {"host": "h", "port": 1}


def lookup(key):
    try:
        return {"ok": True, "value": CONFIG[key]}
    except KeyError:
        return {"ok": False, "error": key}


def validate(rec):
    if "id" not in rec:
        raise ValueError("no id")
    return {"id": rec["id"], "valid": True}


def safe_validate(rec):
    out = {"tried": True}
    try:
        out["result"] = validate(rec)
    except ValueError as e:
        out["error"] = str(e)
    finally:
        out["done"] = True
    return out


hit = lookup("host")                      #@row hit: present={ok,value} definite={ok,value} closed
miss = lookup("nope")                     #@row miss: present={ok,error} definite={ok,error} closed
good = safe_validate({"id": 1})           #@row good: present={tried,result,done} definite={tried,result,done} closed
bad = safe_validate({"name": "x"})        #@row bad: present={tried,error,done} definite={tried,error,done} closed
res = good["result"]                      #@row res: present={id,valid} definite={id,valid} closed
hit["error"]                              #@ missing error
miss["value"]                             #@ missing value
good["error"]                             #@ missing error
bad["result"]                             #@ missing result
