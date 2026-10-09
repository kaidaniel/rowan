"""Hypothesis: starred assignment targets and slices of a known list keep positions: the
head, tail and middle elements are the right objects.

Expectations were written from the program text before running the engine."""

ROWS = [{"h": 1}, {"a": 1}, {"b": 2}, {"t": 9}]
head, *mid, tail = ROWS                      #@row head: present={h} definite={h} closed
_ = 0                                        #@row tail: present={t} definite={t} closed
m0 = mid[0]                                  #@row m0: present={a} definite={a} closed
m1 = mid[-1]                                 #@row m1: present={b} definite={b} closed
first, second = ROWS[1:3]                    #@row second: present={b} definite={b} closed
*init, end = ROWS[:2]                        #@row end: present={a} definite={a} closed
tail["h"]                                    #@ missing h
