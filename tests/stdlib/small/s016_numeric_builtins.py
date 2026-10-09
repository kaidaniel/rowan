# divmod / pow / abs / round and chr / ord / hex / bin / oct on literals; results used as keys


def split_minutes(total):
    h, m = divmod(total, 60)
    return {"h": h, "m": m}


def encode(c):
    code = ord(c)
    return {"char": c, "code": code, "hex": hex(code)}


if __name__ == "__main__":
    r = split_minutes(135)
    h = r["h"]                                   #@value h: exactly={2}
    m = r["m"]                                   #@value m: exactly={15}
    p = pow(2, 10)                               #@value p: exactly={1024}
    pm = pow(3, 2, 5)                            #@value pm: exactly={4}
    a = abs(-3.5)                                #@value a: exactly={3.5}
    r1 = round(7.5)                              #@value r1: exactly={8}
    r2 = round(6.5)                              #@value r2: exactly={6}
    r3 = round(1234, -2)                         #@value r3: exactly={1200}
    ch = chr(65)                                 #@value ch: exactly={"A"}
    hx = hex(255) + bin(5) + oct(8)              #@value hx: exactly={"0xff0b1010o10"}
    letters = {chr(ord("a") + i): i for i in range(3)}  #@row letters: present={a,b,c} definite={a,b,c} closed
    codes = {hex(n): n for n in (10, 16)}        #@row codes: present={"0xa","0x10"} definite={"0xa","0x10"} closed
    e = encode("a")
    e2 = encode("z")
    c = e2["code"]                               #@value c: exactly={122}
    x = e["hex"]                                 #@value x: exactly={"0x61"}
