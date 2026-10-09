# operator.attrgetter (dotted paths, several names) and methodcaller on user objects and dicts

from operator import attrgetter, methodcaller
from types import SimpleNamespace


class Address:
    def __init__(self, city):
        self.city = city


class User:
    def __init__(self, name, city):
        self.name = name
        self.address = Address(city)


def cities(people):
    return list(map(attrgetter("address.city"), people))


users = [User("bo", "rome"), User("al", "oslo")]
u = users[0]                                      #@row u: present={name,address} definite={name,address} closed
by_city = sorted(users, key=attrgetter("address.city"))
fc = by_city[0].name                              #@value fc: exactly={"al"}
pair = attrgetter("name", "address.city")(users[0])   #@value pair: exactly={("bo", "rome")}
try:
    attrgetter("email")(users[0])                 #@ missing email
except AttributeError:
    pass
cs = cities(users)
c1 = cs[1]                                        #@value c1: exactly={"oslo"}
ns = SimpleNamespace(a=1)
av = attrgetter("a")(ns)                          #@value av: exactly={1}
ks = sorted(methodcaller("keys")({"y": 1, "x": 2}))
k0 = ks[0]                                        #@value k0: exactly={"x"}
getter = methodcaller("get", "port", 80)
p1 = getter({"port": 8080})                       #@value p1: exactly={8080}
p2 = getter({})                                   #@value p2: exactly={80}
up = methodcaller("upper")("abc")                 #@value up: exactly={"ABC"}
