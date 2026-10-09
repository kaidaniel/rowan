# deque(maxlen=) keeps the last records; a UserDict subclass normalising keys and answering misses
from collections import UserDict, deque


def recent(events, n):
    q = deque(maxlen=n)
    for e in events:
        q.append(e)
    return q


class Config(UserDict):
    def __setitem__(self, key, value):
        super().__setitem__(key.lower(), value)

    def __missing__(self, key):
        return None


def load(raw):
    c = Config(raw)
    port = c["port"]                             #@ ok port
    return c


if __name__ == "__main__":
    q = recent([{"id": 1}, {"id": 2}, {"id": 3, "x": 1}], 2)
    first = q[0]                                 #@row first: present={id} definite={id} closed
    last = q[-1]                                 #@row last: present={id,x} definite={id,x} closed
    n = len(q)                                   #@value n: exactly={2}
    q.appendleft({"id": 0})
    head = q[0]["id"]                            #@value head: exactly={0}
    m = len(q)                                   #@value m: exactly={2}
    gone = q.pop()                               #@row gone: present={id} definite={id} closed
    c = load({"Host": "h", "PORT": 1})           #@row c: present={host,port} definite={host,port} absent={Host,PORT} closed
    c2 = load({})                                #@row c2: present={} closed
    v = c2["anything"]                           #@value v: exactly={None}
    data = c.data                                #@row data: present={host,port} definite={host,port} closed
