# vars() / __dict__ as a record of an instance's attributes: reads, copies and writes through it


class Point:
    def __init__(self, x, y):
        self.x = x
        self.y = y


def as_dict(obj):
    d = vars(obj).copy()
    d["type"] = type(obj).__name__
    _ = 0                                        #@row d: present={x,y,label,type} definite={x,y,type} closed
    return d


def bump(obj):
    vars(obj)["z"] = 0
    obj.__dict__.update({"w": 1})
    _ = 0                                        #@row obj: present={x,y,z,w,label} definite={x,y,z,w} closed
    obj.z                                        #@ ok z
    return obj


if __name__ == "__main__":
    p = Point(1, 2)
    q = Point(3, 4)
    q.label = "q"
    a = as_dict(p)                               #@row a: present={x,y,type} definite={x,y,type} closed
    as_dict(q)
    bump(p)
    bump(q)
    keys = tuple(sorted(vars(p)))                #@value keys: exactly={("w","x","y","z")}
    try:
        p.label                                  #@ missing label
    except AttributeError:
        pass
