"""Hypothesis: a key read from what unknown code returned is needed of an input only when the
input was given to it whole (or one of its elements): the code may give it back, or a copy,
and the requirement holds unless that code added the key (the golden `always` is the raw one,
as in a02; the report says sometimes). A value read under a key of an input is taken as a
value, not as a mapping: a key read from the result needs nothing more of that value.

Expectations were written from the program text before running the engine."""


def plan(order, stock):
    picks = []
    for line in order["lines"]:
        result = stock.consume(line["sku"], line["qty"])
        picks.extend(result["picks"])
        if result["short"]:
            picks.append(None)
    return picks
#@schema param plan.order: always={lines} only={}
#@schema param plan.order['lines']['*']: always={sku,qty} only={}
#@schema param plan.order['lines']['*']['sku']: only={}
#@schema param plan.order['lines']['*']['qty']: only={}


def scaled(df, lib):
    sub = lib.clip(df[["x"]], upper=df["y"])
    return sub["y"]
#@schema param scaled.df: always={x,y} only={}
#@schema param scaled.df['y']: only={}


def merged(cfg, lib):
    full = lib.apply_defaults(cfg)
    return full["timeout"]
#@schema param merged.cfg: always={timeout} only={}


def each(rows, lib):
    return [lib.normalize(r)["id"] for r in rows]
#@schema param each.rows['*']: always={id} only={}


def build(model_cls, pretrained, **kwargs):
    # (`model_cls(**kwargs)` is given kwargs' values, not kwargs, and is not given itself)
    model = model_cls(**kwargs)
    if pretrained:
        model["weights"]
    return model
#@schema param build.kwargs: only={}
#@schema param build.model_cls: only={}
