# slices of a parameter's list are lists (not elements), with known positions where the call site knows them
import sys


def head(rows, k):
    return rows[:k]


def window(rows, n):
    return rows[slice(0, n)]


def tail(rows):
    return rows[1:]


a = head([{"k": 1}, {"k": 2}], len(sys.argv))
first = a[0]                                         #@ sound 0
#@row first: present={k} definite={k} closed
w = window([{"k": 1}, {"k": 2}, {"j": 3}], 2)
n = len(w)                                           #@value n: exactly={2}
last = w[-1]
#@row last: present={k} definite={k} closed
t = tail([{"a": 1}, {"b": 2}, {"c": 3}])
m = len(t)                                           #@value m: exactly={2}
t0 = t[0]
#@row t0: present={b} definite={b} closed
