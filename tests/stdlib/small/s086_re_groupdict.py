# re named groups: groupdict() keys are the group names (an unmatched optional group maps to None)

import re

LINE = re.compile(r"(?P<level>[A-Z]+) (?P<code>\d+)(?: (?P<msg>.*))?")


def parse(line):
    m = LINE.match(line)
    if m is None:
        return {}
    d = m.groupdict()
    #@row d: present={level,code,msg} definite={level,code,msg} closed
    d["code"] = int(d["code"])                   #@ ok code
    return d


a = parse("ERROR 42 disk full")                   #@row a: present={level,code,msg} definite={level,code,msg} closed
lv = a["level"]                                   #@value lv: exactly={"ERROR"}
c = a["code"]                                     #@value c: exactly={42}
b = parse("INFO 7")
bm = b["msg"]                                     #@value bm: exactly={None}
z = parse("nothing here")                         #@row z: present={} closed
try:
    z["level"]                                    #@ missing level
except KeyError:
    pass
m = re.search(r"(?P<user>\w+)@(?P<host>[\w.]+)", "mail bob@example.org now")
u = m.group("user")                               #@value u: exactly={"bob"}
gd = m.groupdict()                                #@row gd: present={user,host} definite={user,host} closed
h = gd["host"]                                    #@value h: exactly={"example.org"}
fm = re.fullmatch(r"(?P<y>\d{4})-(?P<mo>\d\d)", "2024-05")
y = fm["y"]                                       #@value y: exactly={"2024"}
nofm = re.fullmatch(r"\d+", "12a")                #@value nofm: exactly={None}
gd2 = re.match(r"(?P<a>x)(?P<b>y)?", "x").groupdict("")
bv = gd2["b"]                                     #@value bv: exactly={""}
