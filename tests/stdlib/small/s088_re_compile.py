# compiled patterns: finditer with named groups into a dict, lastgroup, groupindex, groups(), sub with backrefs

import re

KV = re.compile(r"(?P<key>\w+)\s*=\s*(?P<val>[^;]+)")
TOKEN = re.compile(r"(?P<num>\d+)|(?P<op>[+*])")
DATE = re.compile(r"(\d{4})-(\d{2})-(\d{2})")


def parse_kv(text):
    out = {}
    for m in KV.finditer(text):
        out[m["key"]] = m["val"].strip()
    return out


def valid_emails(users):
    pat = re.compile(r"[^@]+@[^@]+\.\w+")
    return [u["email"] for u in users if pat.fullmatch(u["email"])]
#@schema param valid_emails.users['*']: always={email} only={email}


cfg = parse_kv("host = db; port=5432;user = ann ")
#@row cfg: present={host,port,user} definite={host,port,user} closed
port = cfg["port"]                               #@value port: exactly={"5432"}
user = cfg["user"]                               #@value user: exactly={"ann"}
e = parse_kv("no pairs")                         #@row e: present={} closed
kinds = [m.lastgroup for m in TOKEN.finditer("1+22*3")]
k1 = kinds[1]                                    #@value k1: exactly={"op"}
nk = len(kinds)                                  #@value nk: exactly={5}
gi = TOKEN.groupindex                            #@row gi: present={num,op} definite={num,op} closed
g = DATE.search("on 2023-11-05 ok").groups()     #@value g: exactly={("2023", "11", "05")}
none = DATE.match("on 2023-11-05")               #@value none: exactly={None}
fm = DATE.fullmatch("2023-11-05").group(2)       #@value fm: exactly={"11"}
red = DATE.sub(r"\3/\2/\1", "2023-11-05")        #@value red: exactly={"05/11/2023"}
nv = len(valid_emails([{"email": "a@b.co"}, {"email": "bad", "id": 2}]))   #@value nv: exactly={1}
