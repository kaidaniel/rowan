# re.sub with a function replacement: the callback receives each match; findall / split / finditer on literals

import re


def expand(template, values):
    def repl(m):
        return str(values[m.group(1)])
    return re.sub(r"\{(\w+)\}", repl, template)


def tokens(text):
    out = []

    def collect(m):
        out.append({"word": m.group(0), "at": m.start()})
        return m.group(0).upper()
    up = re.sub(r"\w+", collect, text)
    return up, out


e1 = expand("hi {name}", {"name": "bo"})                   #@value e1: exactly={"hi bo"}
e2 = expand("{name} is {age}", {"name": "al", "age": 3})   #@value e2: exactly={"al is 3"}
try:
    expand("{zip}", {"name": "x"})                         #@ missing zip
except KeyError:
    pass
up, toks = tokens("hi there")                              #@value up: exactly={"HI THERE"}
t1 = toks[1]                                               #@row t1: present={word,at} definite={word,at} closed
at = t1["at"]                                              #@value at: exactly={3}
nw = len(re.findall(r"[a-z]+", "ab 12 cd ef"))             #@value nw: exactly={3}
p1 = re.findall(r"(\w)=(\d)", "a=1 b=2")[1]                #@value p1: exactly={("b", "2")}
parts = re.split(r"[,;]\s*", "a, b;c")
np_ = len(parts)                                           #@value np_: exactly={3}
s0 = [m.start() for m in re.finditer(r"o", "foo boo")][0]  #@value s0: exactly={1}
dbl = re.sub(r"(\d+)", lambda m: str(int(m.group(1)) * 2), "a1b22")   #@value dbl: exactly={"a2b44"}
cnt = re.subn(r"a", "x", "banana")[1]                      #@value cnt: exactly={3}
