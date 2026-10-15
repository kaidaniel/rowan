"""Hypothesis: unknown code called again and again on data nothing changes between the calls
(a function past its budget, called in a loop) walks that data once: each later call walks
only what changed since the last walk by the same code (here nothing), not everything its
globals reach. The work was calls x objects (about 30 x 81 here); it is now about the
objects once. What the code may change stays as unknown code may: after every call.

Expectations were written from the program text before running the engine."""
#@limits budget=0
#@havoc <= 600
import sys

reg0 = {"id": 0}
reg1 = {"id": 1}
reg2 = {"id": 2}
reg3 = {"id": 3}
reg4 = {"id": 4}
reg5 = {"id": 5}
reg6 = {"id": 6}
reg7 = {"id": 7}
reg8 = {"id": 8}
reg9 = {"id": 9}
reg10 = {"id": 10}
reg11 = {"id": 11}
reg12 = {"id": 12}
reg13 = {"id": 13}
reg14 = {"id": 14}
reg15 = {"id": 15}
reg16 = {"id": 16}
reg17 = {"id": 17}
reg18 = {"id": 18}
reg19 = {"id": 19}
reg20 = {"id": 20}
reg21 = {"id": 21}
reg22 = {"id": 22}
reg23 = {"id": 23}
reg24 = {"id": 24}
reg25 = {"id": 25}
reg26 = {"id": 26}
reg27 = {"id": 27}
reg28 = {"id": 28}
reg29 = {"id": 29}
reg30 = {"id": 30}
reg31 = {"id": 31}
reg32 = {"id": 32}
reg33 = {"id": 33}
reg34 = {"id": 34}
reg35 = {"id": 35}
reg36 = {"id": 36}
reg37 = {"id": 37}
reg38 = {"id": 38}
reg39 = {"id": 39}
reg40 = {"id": 40}
reg41 = {"id": 41}
reg42 = {"id": 42}
reg43 = {"id": 43}
reg44 = {"id": 44}
reg45 = {"id": 45}
reg46 = {"id": 46}
reg47 = {"id": 47}
reg48 = {"id": 48}
reg49 = {"id": 49}
reg50 = {"id": 50}
reg51 = {"id": 51}
reg52 = {"id": 52}
reg53 = {"id": 53}
reg54 = {"id": 54}
reg55 = {"id": 55}
reg56 = {"id": 56}
reg57 = {"id": 57}
reg58 = {"id": 58}
reg59 = {"id": 59}
reg60 = {"id": 60}
reg61 = {"id": 61}
reg62 = {"id": 62}
reg63 = {"id": 63}
reg64 = {"id": 64}
reg65 = {"id": 65}
reg66 = {"id": 66}
reg67 = {"id": 67}
reg68 = {"id": 68}
reg69 = {"id": 69}
reg70 = {"id": 70}
reg71 = {"id": 71}
reg72 = {"id": 72}
reg73 = {"id": 73}
reg74 = {"id": 74}
reg75 = {"id": 75}
reg76 = {"id": 76}
reg77 = {"id": 77}
reg78 = {"id": 78}
reg79 = {"id": 79}
ALL = [reg0, reg1, reg2, reg3, reg4, reg5, reg6, reg7, reg8, reg9, reg10, reg11, reg12, reg13, reg14, reg15, reg16, reg17, reg18, reg19, reg20, reg21, reg22, reg23, reg24, reg25, reg26, reg27, reg28, reg29, reg30, reg31, reg32, reg33, reg34, reg35, reg36, reg37, reg38, reg39, reg40, reg41, reg42, reg43, reg44, reg45, reg46, reg47, reg48, reg49, reg50, reg51, reg52, reg53, reg54, reg55, reg56, reg57, reg58, reg59, reg60, reg61, reg62, reg63, reg64, reg65, reg66, reg67, reg68, reg69, reg70, reg71, reg72, reg73, reg74, reg75, reg76, reg77, reg78, reg79]


def find(k):
    for r in ALL:
        if r["id"] == k:
            return r
    return None


for i in range(len(sys.argv)):
    f = find(i)

for x in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]:
    g = find(x)

first = reg0["id"]                       #@ likely id
n = len(ALL)  # (was `unknown`: find returns one of ALL's items, ALL itself does not change)
#@value n: exactly={80}
