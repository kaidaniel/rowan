# slices of literal sequences decide which keys a record built from them has

KEYS = ("id", "a", "b", "c")


def tail_record(values):
    rec = dict(zip(KEYS[1:], values))
    return rec


def head_record(values):
    rec = dict(zip(KEYS[:-1], values))
    _ = 0                                        #@row rec: present={id,a,b} definite={id,a,b} absent={c} closed
    return rec


def window(rows, n):
    return rows[slice(0, n)]


if __name__ == "__main__":
    t = tail_record([1, 2, 3, 4])                #@row t: present={a,b,c} definite={a,b,c} absent={id} closed
    head_record([1, 2, 3])
    head_record("xyzw")
    w = window([{"k": 1}, {"k": 2}, {"j": 3}], 2)
    n = len(w)                                   #@value n: exactly={2}
    last = w[-1]                                 #@row last: present={k} definite={k} closed
    s = "abcdef"[1:4]                            #@value s: exactly={"bcd"}
    ev = "abcdef"[::2]                           #@value ev: exactly={"ace"}
    rv = "abc"[::-1]                             #@value rv: exactly={"cba"}
    ix = slice(1, None, 2).indices(6)            #@value ix: exactly={(1,6,2)}
    mid = KEYS[1:3]                              #@value mid: exactly={("a","b")}
