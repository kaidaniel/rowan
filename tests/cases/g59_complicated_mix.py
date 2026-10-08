#@engine rust
"""A larger program mixing registries, closures, classes, loops and configs."""
import json

STEPS = {}


def step(name, requires=()):
    def deco(fn):
        STEPS[name] = {"fn": fn, "requires": list(requires)}
        return fn
    return deco


@step("load")
def load_step(ctx):
    ctx["raw"] = json.load(ctx["fh"])    #@ ok fh
    return ctx


@step("parse", requires=("load",))
def parse_step(ctx):
    raw = ctx["raw"]                     #@ ok raw
    ctx["records"] = [{"id": r["id"], "v": r.get("v", 0)} for r in raw["rows"]]   #@ input rows
    return ctx


@step("summarise", requires=("parse",))
def summarise_step(ctx):
    total = 0
    for rec in ctx["records"]:           #@ ok records
        total += rec["v"]                #@ ok v
        rec["seen"] = True
    ctx["summary"] = {"total": total, "n": len(ctx["records"])}
    return ctx


class Runner:
    def __init__(self, order):
        self.order = order
        self.done = {}

    def run(self, ctx):
        for name in self.order:
            spec = STEPS[name]
            for dep in spec["requires"]:         #@ ok requires
                if dep not in self.done:
                    raise RuntimeError(dep)
            ctx = spec["fn"](ctx)                #@ ok fn
            self.done[name] = True
        return ctx


runner = Runner(["load", "parse", "summarise"])
out = runner.run({"fh": fh})
out["summary"]["total"]                  #@ ok summary ; ok total
out["summary"]["avg"]                    #@ missing avg
out["records"]                           #@ ok records
out["raw"]                               #@ ok raw
out["report"]                            #@ missing report
for rec in out["records"]:
    rec["seen"]                          #@ ok seen
    rec["id"]                            #@ ok id
    rec["name"]                          #@ missing name
STEPS["load"]["requires"]                #@ ok load ; ok requires
STEPS["parse"]["after"]                  #@ missing after
runner.done["load"]                      #@ ok load
runner.done["nope"]                      #@ missing nope
#@schema json.load@L17['rows']['*']: always={id}
