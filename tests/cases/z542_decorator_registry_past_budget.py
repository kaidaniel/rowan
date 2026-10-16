"""Hypothesis: a decorator past its budget (a registry filled at import: one skipped call
per decorated `def`, each at its own line) walks what its code may change once, not once per
`def`: the walks of unknown code are remembered by what they walk, not by the line of the call,
so each later call walks only what changed since (here nothing). The work was defs x records
(about 30 x 81 here); it is now about the records once. What the code may change is still
changed after every call: the records' keys are there unless unknown code removed them.

Expectations were written from the program text before running the engine."""
#@limits budget=0
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
REGISTRY = {}


def register(fn):
    for r in ALL:
        r["seen"] = fn.__name__
    REGISTRY[fn.__name__] = fn
    return fn


@register
def handler0(x):
    return x + 0

@register
def handler1(x):
    return x + 1

@register
def handler2(x):
    return x + 2

@register
def handler3(x):
    return x + 3

@register
def handler4(x):
    return x + 4

@register
def handler5(x):
    return x + 5

@register
def handler6(x):
    return x + 6

@register
def handler7(x):
    return x + 7

@register
def handler8(x):
    return x + 8

@register
def handler9(x):
    return x + 9

@register
def handler10(x):
    return x + 10

@register
def handler11(x):
    return x + 11

@register
def handler12(x):
    return x + 12

@register
def handler13(x):
    return x + 13

@register
def handler14(x):
    return x + 14

@register
def handler15(x):
    return x + 15

@register
def handler16(x):
    return x + 16

@register
def handler17(x):
    return x + 17

@register
def handler18(x):
    return x + 18

@register
def handler19(x):
    return x + 19

@register
def handler20(x):
    return x + 20

@register
def handler21(x):
    return x + 21

@register
def handler22(x):
    return x + 22

@register
def handler23(x):
    return x + 23

@register
def handler24(x):
    return x + 24

@register
def handler25(x):
    return x + 25

@register
def handler26(x):
    return x + 26

@register
def handler27(x):
    return x + 27

@register
def handler28(x):
    return x + 28

@register
def handler29(x):
    return x + 29


first = reg0["id"]                       #@ likely id
n = len(ALL)  # (register iterates ALL and changes its items, not ALL)
#@value n: exactly={80}
