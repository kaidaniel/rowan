# operator.add / mul / or_ with reduce: sums, products, string joins and dict unions (|)

import operator
from functools import reduce


def combine(layers):
    cfg = reduce(operator.or_, layers, {})
    return cfg


def total_of(orders):
    amounts = map(operator.itemgetter("amount"), orders)
    return reduce(operator.add, amounts, 0)
#@schema param total_of.orders['*']: always={amount} only={amount}


c = combine([{"a": 1}, {"b": 2}, {"a": 3}])       #@row c: present={a,b} definite={a,b} closed
ca = c["a"]                                        #@value ca: exactly={3}
e = combine([])                                    #@row e: present={} closed
try:
    e["a"]                                         #@ missing a
except KeyError:
    pass
p = reduce(operator.mul, [2, 3, 7], 1)             #@value p: exactly={42}
s = reduce(operator.add, ["a", "b", "c"])          #@value s: exactly={"abc"}
nt = len(reduce(operator.add, [[1], [2, 3]]))      #@value nt: exactly={3}
g = operator.getitem({"k": 5}, "k")                #@value g: exactly={5}
tt = total_of([{"amount": 4, "id": 1}, {"amount": 6}])   #@value tt: exactly={10}
t0 = total_of([])                                  #@value t0: exactly={0}
