# str methods on literal strings: values follow (strip, title, zfill, upper, replace, split, partition, format…)


def label(rec):
    name = rec["name"].strip().title()
    code = str(rec["code"]).zfill(4)
    return f"{code}-{name}"
#@schema param label.rec: always={name,code} only={name,code}


l1 = label({"name": "  ada lovelace ", "code": 7})   #@value l1: exactly={"0007-Ada Lovelace"}
l2 = label({"name": "bob", "code": 12345, "x": 1})   #@value l2: exactly={"12345-Bob"}
up = "abc".upper()                                    #@value up: exactly={"ABC"}
rp = "a-b-c".replace("-", "+", 1)                     #@value rp: exactly={"a+b-c"}
p1 = "k=v=w".split("=", 1)[1]                         #@value p1: exactly={"v=w"}
head, sep, tail = "user@example.com".partition("@")   #@value head: exactly={"user"} ; tail: exactly={"example.com"}
rh, _, rt = "a.b.c".rpartition(".")                   #@value rh: exactly={"a.b"} ; rt: exactly={"c"}
fmt = "{0}:{name}".format(1, name="x")                #@value fmt: exactly={"1:x"}
c = "hi".center(6, "*")                               #@value c: exactly={"**hi**"}
sw = "report.csv".endswith((".csv", ".tsv"))          #@value sw: exactly={True}
cnt = "banana".count("an")                            #@value cnt: exactly={2}
pc = "%s has %d" % ("x", 3)                           #@value pc: exactly={"x has 3"}
fmap = "{a}-{b}".format_map({"a": 1, "b": 2})         #@value fmap: exactly={"1-2"}
try:
    "{a}-{c}".format_map({"a": 1})                    #@ missing c
except KeyError:
    pass
