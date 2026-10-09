"""A plugin registry filled from the package's submodules when the package is imported;
the entry picks plugins by name and by option.

Expectations were written from the program text before running the engine."""
from plugins import REGISTRY


def pick(name):
    entry = REGISTRY[name]
    return entry["cls"]()


registry = REGISTRY                       #@row registry: present={csv,json} definite={csv,json} closed
csv_entry = registry["csv"]               #@row csv_entry: present={cls,opts} definite={cls,opts} closed
csv_opts = csv_entry["opts"]              #@row csv_opts: present={sep} definite={sep} closed
json_opts = registry["json"]["opts"]      #@row json_opts: present={strict} definite={strict} closed
out = pick("csv").read("1,2\n3,4")        #@row out: present={rows,format} definite={rows,format} closed
row0 = out["rows"][0]                     #@row row0: present={a,b} definite={a,b} closed
jout = pick("json").read("{}")            #@row jout: present={rows,format,strict} definite={rows,format,strict} closed
out["strict"]                             #@ missing strict
registry["xml"]                           #@ missing xml
