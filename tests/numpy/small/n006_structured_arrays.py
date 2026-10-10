# numpy structured arrays: fields of a record dtype are keys (field reads are proven, a missing
# field is a ValueError), dtype.names / shape / size; genfromtxt(names=True) reads the header,
# so its field reads are requirements on the file
import io

import numpy as np

ROWS = [(1, 2.0), (3, 4.0)]


def table():
    rec = np.array(ROWS, dtype=[("x", "i4"), ("y", "f8")])
    _ = 0                                        #@row rec: present={x,y} definite={x,y} closed
    x = rec["x"]                                 #@shape x: [2]
    names = rec.dtype.names                      #@value names: exactly={("x","y")}
    n = rec.size                                 #@value n: exactly={2}
    try:
        rec["zz"]                                #@ missing zz
        caught = False
    except ValueError:
        caught = True
    return caught


def prices(text):
    data = np.genfromtxt(io.StringIO(text), delimiter=",", names=True)
    return data["price"]                         #@ input price


if __name__ == "__main__":
    caught = table()                             #@value caught: exactly={True}
    z = np.zeros(5, dtype=[("a", "f8"), ("b", "i8")])
    zb = z["b"]                                  #@shape zb: [5]
    p = prices("price,qty\n1,2\n3,4\n")
    size = np.zeros((3, 4)).size                 #@value size: exactly={12}
