"""Hypothesis: a call that does not run (here every call: `#@limits budget=0`) applies what
its code may change, the closure of its code, at every call, reading every slot of it again
though nothing the closure reads or changed has changed since: `work` reaches 24 globals and
is called 40 times. A call whose closure was applied before, with none of what it read or
changed changed since, changes nothing again. What the calls may change is still changed:
each record may have gained "seen" and its items may be anything.

Expectations were written from the program text before running the engine."""
#@limits budget=0
#@visits <= 700
REG0 = {"id": 0, "items": [0]}
REG1 = {"id": 1, "items": [1]}
REG2 = {"id": 2, "items": [2]}
REG3 = {"id": 3, "items": [3]}
REG4 = {"id": 4, "items": [4]}
REG5 = {"id": 5, "items": [5]}
REG6 = {"id": 6, "items": [6]}
REG7 = {"id": 7, "items": [7]}
REG8 = {"id": 8, "items": [8]}
REG9 = {"id": 9, "items": [9]}
REG10 = {"id": 10, "items": [10]}
REG11 = {"id": 11, "items": [11]}
REG12 = {"id": 12, "items": [12]}
REG13 = {"id": 13, "items": [13]}
REG14 = {"id": 14, "items": [14]}
REG15 = {"id": 15, "items": [15]}
REG16 = {"id": 16, "items": [16]}
REG17 = {"id": 17, "items": [17]}
REG18 = {"id": 18, "items": [18]}
REG19 = {"id": 19, "items": [19]}
REG20 = {"id": 20, "items": [20]}
REG21 = {"id": 21, "items": [21]}
REG22 = {"id": 22, "items": [22]}
REG23 = {"id": 23, "items": [23]}


def touch0():
    REG0["seen"] = True
    REG0["items"].append(1)


def touch1():
    REG1["seen"] = True
    REG1["items"].append(1)


def touch2():
    REG2["seen"] = True
    REG2["items"].append(1)


def touch3():
    REG3["seen"] = True
    REG3["items"].append(1)


def touch4():
    REG4["seen"] = True
    REG4["items"].append(1)


def touch5():
    REG5["seen"] = True
    REG5["items"].append(1)


def touch6():
    REG6["seen"] = True
    REG6["items"].append(1)


def touch7():
    REG7["seen"] = True
    REG7["items"].append(1)


def touch8():
    REG8["seen"] = True
    REG8["items"].append(1)


def touch9():
    REG9["seen"] = True
    REG9["items"].append(1)


def touch10():
    REG10["seen"] = True
    REG10["items"].append(1)


def touch11():
    REG11["seen"] = True
    REG11["items"].append(1)


def touch12():
    REG12["seen"] = True
    REG12["items"].append(1)


def touch13():
    REG13["seen"] = True
    REG13["items"].append(1)


def touch14():
    REG14["seen"] = True
    REG14["items"].append(1)


def touch15():
    REG15["seen"] = True
    REG15["items"].append(1)


def touch16():
    REG16["seen"] = True
    REG16["items"].append(1)


def touch17():
    REG17["seen"] = True
    REG17["items"].append(1)


def touch18():
    REG18["seen"] = True
    REG18["items"].append(1)


def touch19():
    REG19["seen"] = True
    REG19["items"].append(1)


def touch20():
    REG20["seen"] = True
    REG20["items"].append(1)


def touch21():
    REG21["seen"] = True
    REG21["items"].append(1)


def touch22():
    REG22["seen"] = True
    REG22["items"].append(1)


def touch23():
    REG23["seen"] = True
    REG23["items"].append(1)


def work(x):
    touch0()
    touch1()
    touch2()
    touch3()
    touch4()
    touch5()
    touch6()
    touch7()
    touch8()
    touch9()
    touch10()
    touch11()
    touch12()
    touch13()
    touch14()
    touch15()
    touch16()
    touch17()
    touch18()
    touch19()
    touch20()
    touch21()
    touch22()
    touch23()
    return x


out = []
out.append(work(0))
out.append(work(1))
out.append(work(2))
out.append(work(3))
out.append(work(4))
out.append(work(5))
out.append(work(6))
out.append(work(7))
out.append(work(8))
out.append(work(9))
out.append(work(10))
out.append(work(11))
out.append(work(12))
out.append(work(13))
out.append(work(14))
out.append(work(15))
out.append(work(16))
out.append(work(17))
out.append(work(18))
out.append(work(19))
out.append(work(20))
out.append(work(21))
out.append(work(22))
out.append(work(23))
out.append(work(24))
out.append(work(25))
out.append(work(26))
out.append(work(27))
out.append(work(28))
out.append(work(29))
out.append(work(30))
out.append(work(31))
out.append(work(32))
out.append(work(33))
out.append(work(34))
out.append(work(35))
out.append(work(36))
out.append(work(37))
out.append(work(38))
out.append(work(39))

first = REG0["id"]                           #@value first: includes={0}
REG0["id"]                                   #@ sound id
REG23["seen"]                                #@ sound seen
items = REG23["items"]                       #@value items: unknown
