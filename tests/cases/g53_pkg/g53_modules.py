"""Imports: functions, module-level dicts, classes, aliases; module attribute writes."""
import helpers2
import helpers2 as h2
from helpers2 import build, DEFAULTS, Config

c = build({"user": "u"})
c["timeout"]                             #@ ok timeout
c["user"]                                #@ ok user
c["built"]                               #@ ok built
c["nope"]                                #@ missing nope
DEFAULTS["retries"]                      #@ ok retries
h2.DEFAULTS["timeout"]                   #@ ok timeout
helpers2.DEFAULTS["zzz"]                 #@ missing zzz
helpers2.register("k", 1)
helpers2.registry["k"]                   #@ ok k
helpers2.registry["j"]                   #@ missing j
helpers2.extra = {"e": 1}
helpers2.extra["e"]                      #@ ok e
helpers2.missing_attr                    #@ missing missing_attr
cfg = Config({"a": 1})
cfg.get("a")                             #@ ok a
cfg.get("b")                             #@ missing b
cfg.data["a"]                            #@ ok a
