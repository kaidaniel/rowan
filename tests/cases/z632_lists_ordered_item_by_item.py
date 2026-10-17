"""Lists of known items order item by item, as tuples do (`[1, 2] < [3]`, a prefix is smaller,
equal lists are not less); a list against a tuple is a TypeError for `<` and unequal for `==`.
Lists built by appends, slicing, `sort` and `reverse` keep their items.

Expectations were written from the program text before running the engine."""
import os

a = [1, 2]
b = [3]
r1 = a < b
print(r1)  #@value r1: exactly={True}
r2 = [1, 2] < [1, 2, 0]
print(r2)  #@value r2: exactly={True}
r3 = [1, 2] <= [1, 2]
print(r3)  #@value r3: exactly={True}
r4 = ["b"] > ["a", "z"]
print(r4)  #@value r4: exactly={True}
c = []
c.append(5)
c.append(1)
r5 = c >= [5, 2]
print(r5)  #@value r5: exactly={False}
c.sort()
r6 = c < [2]
print(r6)  #@value r6: exactly={True}
c.reverse()
r7 = c == [5, 1]
print(r7)  #@value r7: exactly={True}
d = [4, 3, 9][1:]
r8 = d > [3, 8]
print(r8)  #@value r8: exactly={True}
try:
    r9 = [1] < (2,)
except TypeError:
    r9 = "te"
print(r9)  #@value r9: exactly={"te"}
r10 = [1] == (1,)
print(r10)  #@value r10: exactly={False}
e = [1, 2] if os.environ.get("Z632_E") else [0]
r11 = e < [1, 3]
print(r11)  #@value r11: exactly={True}
