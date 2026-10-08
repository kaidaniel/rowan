"""Keys computed from literals: concatenation, %-formatting, f-strings, str methods, a prefix
helper, and a loop over literal suffixes; one key from the command line opens the record.

Expectations were written from the program text before running the engine."""
import sys

PREFIX = "user_"


def key_for(name):
    return PREFIX + name


def col(name, i):
    return "%s_%d" % (name, i)


d = {}
d[key_for("ann")] = 1
d["x".upper()] = 2
d[col("c", 3)] = 3
d["a" + "b"] = 4
d[f"f{'lag'}"] = 5
for n in ["p", "q"]:
    d["s_" + n] = 6
_ = 0                                     #@row d: present={user_ann,X,c_3,ab,flag,s_p,s_q} definite={user_ann,X,c_3,ab,flag,s_p,s_q} closed
name = "bob"
d[name.lower()] = 7
_ = 0                                     #@row d: present={user_ann,X,c_3,ab,flag,s_p,s_q,bob} definite={user_ann,X,c_3,ab,flag,s_p,s_q,bob} closed
d["user_ann"]                             #@ ok user_ann
d["c_3"]                                  #@ ok c_3
d["s_r"]                                  #@ missing s_r
e = dict(d)
e[sys.argv[1]] = 8                        #@row e: present={user_ann,X,c_3,ab,flag,s_p,s_q,bob} definite={user_ann,X,c_3,ab,flag,s_p,s_q,bob} open
e["bob"]                                  #@ ok bob
