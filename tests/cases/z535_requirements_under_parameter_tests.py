# What a function reads of one parameter only under a test of another: `if admin: row["role"]`
# reads `role` only on calls where `admin` holds (the `#@only` directive: key, then `if` and the
# test, or `none`). The test is Python text over the parameters as passed; it qualifies only
# while it says the same as at the call (a parameter the function never rebinds, a key nothing
# stored or deleted since).


def make_user(row, admin):
    user = {"id": row["id"]}
    if admin:
        user["role"] = row["role"]
    if "email" in row:
        user["email"] = row["email"]
    else:
        user["phone"] = row["phone"]
    return user
#@schema param make_user.row: always={id} sometimes={role,phone} only={id,role,phone,email}
#@only param make_user.row: role if admin
#@only param make_user.row: phone if "email" not in row
#@only param make_user.row: id none


def by_mode(cfg, mode):
    if mode == "fast":
        return cfg["speed"]
    elif mode == "safe":
        return cfg["limit"]
    return None  # (limit's test: `mode == "safe"` implies `mode != "fast"`, so it is said alone)
#@schema param by_mode.cfg: sometimes={speed,limit} only={speed,limit}
#@only param by_mode.cfg: speed if mode == "fast"
#@only param by_mode.cfg: limit if mode == "safe"


def nested(cfg, a, b):
    if a:
        if b is None:
            return cfg["x"]
    return 0
#@schema param nested.cfg: sometimes={x} only={x}
#@only param nested.cfg: x if a and b is None


def rebound(cfg, flag):
    # the test reads `flag` after the function changed it: not the flag the caller passed
    flag = not flag
    if flag:
        return cfg["k"]
    return 0
#@schema param rebound.cfg: sometimes={k} only={k}
#@only param rebound.cfg: k none


def stored_first(cfg, fill):
    # "k" in cfg may hold because the function stored it, not because the caller passed it
    if fill:
        cfg["k"] = 0
    if "k" in cfg:
        return cfg["v"]
    return 0
#@schema param stored_first.cfg: sometimes={v} only={v}
#@only param stored_first.cfg: v none


def early_return(cfg, on):
    if not on:
        return None
    return cfg["v"]
#@schema param early_return.cfg: sometimes={v} only={v}
#@only param early_return.cfg: v if on


def both_sides(cfg, on):
    if on:
        return cfg["v"] + 1
    return cfg["v"]
#@schema param both_sides.cfg: always={v} only={v}
#@only param both_sides.cfg: v none


def not_on_every_path(cfg, on):
    # under `on`, `v` is read only when cfg has a truthy "w": on some of those calls only
    if on:
        if cfg.get("w"):
            return cfg["v"]
    return 0
#@schema param not_on_every_path.cfg: sometimes={v} only={v}
#@only param not_on_every_path.cfg: v if on


def either_test(cfg, a, b):
    # read under `a`, or under `b` when not `a`: no one test holds on both
    if a:
        return cfg["v"]
    if b:
        return cfg["v"] * 2
    return 0
#@schema param either_test.cfg: sometimes={v} only={v}
#@only param either_test.cfg: v none


def walk(node, deep):
    # the inner call's `deep` is not the outer call's
    if deep:
        return walk(node["child"], False)
    return node["leaf"]
#@schema param walk.node: sometimes={child,leaf}
#@only param walk.node: child if deep
#@only param walk.node: leaf if not deep


def contract(row, admin):
    # nothing calls it: what any caller must pass (the editor shows it on `row`)
    if admin:
        return row["role"]
    return row["id"]
#@only param contract.row: role if admin
#@only param contract.row: id if not admin


rows = [{"id": 1, "role": "x", "email": "e"}, {"id": 2, "phone": "p"}, {"id": 3, "role": "y", "phone": "q"}]
for i, r in enumerate(rows):
    try:
        print(make_user(dict(r), i != 1))
    except KeyError as e:
        print("missing", e)
for m in ["fast", "safe", "other"]:
    try:
        print(by_mode({"speed": 1, "limit": 2}, m))
    except KeyError as e:
        print("missing", e)
for a in (False, True):
    for b in (None, 1):
        try:
            print(nested({"x": 1}, a, b))
            print(either_test({"v": 1}, a, b))
        except KeyError as e:
            print("missing", e)
for on in (False, True):
    for fn in (rebound, stored_first, early_return, both_sides, not_on_every_path):
        try:
            print(fn({"k": 1, "v": 2, "w": on}, on))
        except KeyError as e:
            print("missing", e)
    try:
        print(walk({"child": {"leaf": 1}, "leaf": 2}, on))
    except KeyError as e:
        print("missing", e)
