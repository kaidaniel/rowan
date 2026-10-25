"""Iterating bytes gives ints, one per byte (`for c in b"\\x01"`), as email.quoprimime's tables
need: a list indexed by them takes the stores. (Before, the items were the bytes themselves,
and once a list store by a bytes key raised TypeError the module's code stopped there.)

Expectations were written from the program text before running the engine."""

TABLE = ["-"] * 8
for c in b"\x01\x02" + "\x05".encode("ascii"):
    TABLE[c] = chr(c + 64)
a = TABLE[1]
#@value a: exactly={"A"}
e = TABLE[5]
#@value e: exactly={"E"}
z = TABLE[0]
#@value z: exactly={"-"}
first = [x for x in b"\x07\x08"]
f0 = first[0]
#@value f0: exactly={7}
