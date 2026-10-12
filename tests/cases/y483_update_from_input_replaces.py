"""`d.update(input)` may replace the value under any key d has: the input's value there joins
in (it was the old value only, so a read below it was `missing` where the input decides)."""
import json
import sys

DEFAULTS = {"debug": False, "db": {"host": "localhost"}}
cfg = dict(DEFAULTS)
cfg.update(json.loads(sys.argv[1]))
cfg["db"]["host"]                        #@ input host
cfg["db"]["user"]                        #@ possibly user
cfg["debug"]                             #@ ok debug
