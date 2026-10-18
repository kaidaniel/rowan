# A catch-all handler in a calling function (a CLI runner's `except Exception:`, a worker's) is
# not about a read deep in the code it runs: that read's failure is still the program's
# finding, and the input still needs the key. A handler naming the error (`except KeyError:`,
# also in a caller) or a catch-all around the read in its own function guards it.
# python z674.py → every read finds its key; Z674_TEXT='{}' python z674.py → each is None.
import json
import os

TEXT = os.environ.get("Z674_TEXT", '{"name": "a", "owner": "o", "team": "t"}')


def load(text):
    cfg = json.loads(text)
    return cfg["name"]
    #@ reported name


def run(text):
    try:
        return load(text)
    except Exception:
        return None


def guarded_here(text):
    cfg = json.loads(text)
    try:
        return cfg["owner"]
        #@ none owner
    except Exception:
        return None


def pick(cfg):
    return cfg["team"]
    #@ none team


def named(text):
    try:
        return pick(json.loads(text))
    except KeyError:
        return None


r1 = run(TEXT)
#@value r1: includes={"a", None}
r2 = guarded_here(TEXT)
#@value r2: includes={"o", None}
r3 = named(TEXT)
#@value r3: includes={"t", None}
#@schema json.loads@L13: sometimes={name}
# (sometimes: where json.loads raises, run's handler takes over before the read)
