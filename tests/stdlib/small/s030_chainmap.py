# ChainMap lookups through layered dicts, new_child, maps; writes go to the first map only
from collections import ChainMap

DEFAULTS = {"color": "red", "user": "guest"}


def settings(cli, env):
    cm = ChainMap(cli, env, DEFAULTS)
    cm["color"]                                  #@ ok color
    u = cm["user"]                               #@value u: exactly={guest,kai}
    _ = 0                                        #@row cm: present={color,user,theme,verbose} definite={color,user} closed
    child = cm.new_child({"debug": True})
    child["mode"] = "x"
    first = child.maps[0]                        #@row first: present={debug,mode} definite={debug,mode} closed
    n = len(child.maps)                          #@value n: exactly={4}
    cm["seen"] = 1
    _ = 0                                        #@row cli: present={verbose,color,seen} definite={seen} closed
    return cm


def theme_of(cm):
    return cm["theme"]                           #@ ok theme ; missing theme


if __name__ == "__main__":
    a = settings({"verbose": True}, {"user": "kai"})
    b = settings({"color": "blue"}, {"theme": "dark"})
    for cm in (a, b):
        try:
            theme_of(cm)
        except KeyError:
            pass
    d = DEFAULTS                                 #@row d: present={color,user} definite={color,user} closed
    c = b["color"]                               #@value c: exactly={blue}
