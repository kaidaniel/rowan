"""Hypothesis: keys produced by string methods on literals (`strip`, `lower`, `upper`,
`replace`, `capitalize`, `removeprefix`, `removesuffix`) are known.

Expectations were written from the program text before running the engine."""

RAW = ["  Name ", "AGE", "e-mail"]
clean = {}
for r in RAW:
    clean[r.strip().lower().replace("-", "_")] = True
_ = 0                                        #@row clean: present={name,age,e_mail} definite={name,age,e_mail} closed
clean["Name"]                                #@ missing Name
cap = {s.upper(): 1 for s in ("a", "b")}     #@row cap: present={A,B} definite={A,B} closed
pre = {"x_" + s.capitalize(): 0 for s in ["foo"]}  #@row pre: present={x_Foo} definite={x_Foo} closed
short = {"db_host".removeprefix("db_"): 1, "port.cfg".removesuffix(".cfg"): 2}  #@row short: present={host,port} definite={host,port} closed
