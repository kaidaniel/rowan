# keyword partials: bound keywords, overriding them at the call, a partial of a partial, **opts bound at run time

from functools import partial


def make_event(kind, *, source="app", level="info", **extra):
    ev = {"kind": kind, "source": source, "level": level}
    ev.update(extra)
    return ev


def emit(opts):
    send = partial(make_event, **opts)
    ev = send("tick")
    #@row ev: present={kind,source,level,user} definite={kind,source,level} closed
    return ev


warn = partial(make_event, level="warn")
audit = partial(warn, source="audit", user="root")

e1 = warn("disk")                                  #@row e1: present={kind,source,level} definite={kind,source,level} closed
lv = e1["level"]                                   #@value lv: exactly={"warn"}
e2 = audit("login")                                #@row e2: present={kind,source,level,user} definite={kind,source,level,user} closed
src = e2["source"]                                 #@value src: exactly={"audit"}
e3 = audit("logout", level="error", ip="1.2.3.4")  #@row e3: present={kind,source,level,user,ip} definite={kind,source,level,user,ip} closed
lv3 = e3["level"]                                  #@value lv3: exactly={"error"}
kw = audit.keywords                                #@row kw: present={level,source,user} definite={level,source,user} closed
try:
    e1["user"]                                     #@ missing user
except KeyError:
    pass

if __name__ == "__main__":
    emit({"level": "debug"})
    x = emit({"source": "cron", "user": "bob"})    #@row x: present={kind,source,level,user} definite={kind,source,level,user} closed
    xs = x["source"]                               #@value xs: exactly={"cron"}
