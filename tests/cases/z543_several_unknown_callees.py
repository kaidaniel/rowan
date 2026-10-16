"""Hypothesis: a call of a value that is one of several values rowan knows nothing of (the
hook a parameter's attribute picks) is one unknown call, not one per value: any of them is any
code, each run on its own copy of the state walked all it was given again. The work was
callees x records (12 x 81 here); it is now the records once per run of the function. What
the call is given may change as before: the records' keys are there unless unknown code
removed them, and ALL's length is unknown.

Expectations were written from the program text before running the engine."""
#@havoc <= 400

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


def run(hooks, mode):
    if mode == 0:
        h = hooks.a
    elif mode == 1:
        h = hooks.b
    elif mode == 2:
        h = hooks.c
    elif mode == 3:
        h = hooks.d
    elif mode == 4:
        h = hooks.e
    elif mode == 5:
        h = hooks.f
    elif mode == 6:
        h = hooks.g
    elif mode == 7:
        h = hooks.h
    elif mode == 8:
        h = hooks.i
    elif mode == 9:
        h = hooks.j
    elif mode == 10:
        h = hooks.k
    else:
        h = hooks.l
    h(ALL)
    first = reg0["id"]                   #@ likely id
    return len(ALL)
