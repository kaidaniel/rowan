# f-strings reading record keys and formatting values; f-string keys of a built record


def label(rec):
    return f"{rec['name']} ({rec.get('role', 'user')})"  #@ ok name ; missing name


def columns(n):
    cols = {f"col_{i}": i for i in range(n)}
    return cols


def money(rec):
    amt = rec["amount"]                          #@ ok amount
    return f"{rec['cur']}{amt:.2f}"             #@ ok cur


if __name__ == "__main__":
    a = label({"name": "a", "role": "admin"})    #@value a: exactly={"a (admin)"}
    b = label({"name": "b"})                     #@value b: exactly={"b (user)"}
    try:
        label({"role": "x"})
    except KeyError:
        pass
    c2 = columns(2)                              #@row c2: present={col_0,col_1} definite={col_0,col_1} closed
    c0 = columns(0)                              #@row c0: present={} closed
    m = money({"amount": 1234.5, "cur": "$"})    #@value m: exactly={"$1234.50"}
    n = 7
    z = f"{n:03d}|{n!r}|{'x'*2}"                 #@value z: exactly={"007|7|xx"}
