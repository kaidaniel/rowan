# A value that is a literal on some paths and not known on others keeps the literal: the hover
# says `8080 | ?`, not `?`, and what the code computes from it keeps it too (`8081 | ?`).
# `shows={…}`: exactly these literals are named as possible (beside an unknown or not).
import json
import os
import sys


def load(text):
    cfg = {"port": 8080, "debug": False}
    if text:
        cfg.update(json.loads(text))
    return cfg


cfg = load(sys.argv[1])
port = cfg["port"]                                  #@value port: shows={8080}
next_port = port + 1                                #@value next_port: shows={8081}
timeout = cfg.get("timeout", 30)                    #@value timeout: shows={30}
ms = timeout * 1000                                 #@value ms: shows={30000}
name = cfg.get("name") or "anon"                    #@value name: shows={"anon"}

level = None
if len(sys.argv) > 2:
    level = int(sys.argv[2])
given = level                                       #@value given: shows={None}
if level is None:
    lv = level                                      #@value lv: exactly={None}
else:
    lv2 = level                                     #@value lv2: shows={}
    double = level * 2                              #@value double: shows={}

mode = "fast"
if len(sys.argv) > 3:
    mode = sys.argv[3]
picked = mode                                       #@value picked: shows={"fast"}
if mode == "fast":
    chosen = mode                                   #@value chosen: exactly={"fast"}
else:
    other = mode                                    #@value other: shows={}
upper = mode.upper()                                #@value upper: shows={"FAST"}
label = f"mode={mode}"                              #@value label: shows={"mode=fast"}
text = str(port)                                    #@value text: shows={"8080"}

token = os.environ.get("TOKEN")
seen = token                                        #@value seen: shows={None}
if token:
    t1 = token                                      #@value t1: shows={}
else:
    t2 = token                                      #@value t2: shows={None}
retries = int(os.environ.get("RETRIES", "3"))       #@value retries: shows={3}
debug = os.environ.get("DEBUG", "") == "1"
verbose = debug or cfg.get("verbose", False)        #@value verbose: shows={False, True}

# a value that keeps changing in a loop: an int, any of them
count = 0
for arg in sys.argv:
    if arg:
        count += 1
    else:
        count = len(arg) * 7
total = count                                       #@value total: types={int}; total: includes={0,1,2,14}
