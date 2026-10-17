"""What a function needs of its parameter is the same at its `def` and in its body: the keys the
function reads of it, not what the whole program reads of the input a call passed (`summarise`'s
`cfg` showed `cfg["verbose"]`, read only by the module code). An `__init__` whose class has a
subclass has its contract too (its run for any call took an instance made by running `__init__`
itself on the same argument object, whose key it had read already: no contract).
`vscode-rowan/test.js` checks the hovers.

Expectations were written from the program text before running the engine."""
import json
import os


class Base:
    def __init__(self, cfg):
        self.name = cfg["name"]                     #@ input name


class Child(Base):
    pass


def summarise(cfg):
    return cfg["min_amount"] * 2                    #@ input min_amount


cfg = json.loads(os.environ.get("CFG_JSON", '{"name": "n", "min_amount": 3, "verbose": true}'))
b = Base(cfg)
total = summarise(cfg)
cfg["verbose"]                                      #@ input verbose
#@schema json.loads@L26: always={name,min_amount,verbose} only={name,min_amount,verbose}
#@schema param Base.__init__.cfg: always={name} only={name}
#@schema param summarise.cfg: always={min_amount} only={min_amount}
