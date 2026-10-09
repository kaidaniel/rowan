"""Installed packages: `tinylib` is on the allowlist (`--follow tinylib,pandas` with the fake
site-packages directory `site/`), so its source is analysed; `pandas` is on the allowlist too
but has a model, and the model wins over the (fake) source; `otherlib` is installed but not
followed, so its calls are unknown.

Expectations were written from the program text before running the engine."""
#@argv --follow tinylib,pandas --site-packages site
import otherlib
import pandas as pd
from tinylib import Record, load, VERSION

r = load("x")                             #@row r: present={name,fields,meta} definite={name,fields,meta} closed
fields = r.fields                         #@row fields: present={id,loaded} definite={id,loaded} closed
rd = r.to_dict()                          #@row rd: present={name,fields,meta} definite={name,fields,meta} closed
ver = VERSION                             #@row ver: present={major,minor} definite={major,minor} closed
r2 = Record("y", {"k": 1})                #@row r2: present={name,fields,meta} definite={name,fields,meta} closed
df = pd.DataFrame({"a": [1, 2]})
df["b"] = df["a"] * 2
df["a"]                                   #@ ok a
df["b"]                                   #@ ok b
df["c"]                                   #@ missing c
m = otherlib.make()                       #@row m: present={} definite={} open
r.fields["loaded"]                        #@ ok loaded
r.fields["nope"]                          #@ missing nope
