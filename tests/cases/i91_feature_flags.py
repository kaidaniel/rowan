"""Feature flags with literal booleans and numbers decide which keys a page gets; one flag
comes from the command line; `.get` of an absent flag is falsy.

Expectations were written from the program text before running the engine."""
import sys

FLAGS = {"beta": True, "legacy": False, "limit": 0}


def render(user, flags):
    page = {"user": user["name"]}
    if flags["beta"]:
        page["beta_banner"] = True
    if flags["legacy"]:
        page["legacy_nav"] = True
    if flags["limit"]:
        page["limited"] = True
    if flags.get("theme"):
        page["theme"] = flags["theme"]
    if not flags["legacy"]:
        page["modern"] = True
    return page


p = render({"name": "a"}, FLAGS)          #@row p: present={user,beta_banner,modern} definite={user,beta_banner,modern} closed
dyn = dict(FLAGS, legacy=len(sys.argv) > 3)
p2 = render({"name": "b"}, dyn)           #@row p2: present={user,beta_banner,legacy_nav,modern} definite={user,beta_banner} closed
themed = render({"name": "c"}, {**FLAGS, "theme": "dark", "limit": 5})  #@row themed: present={user,beta_banner,limited,theme,modern} definite={user,beta_banner,limited,theme,modern} closed
p["legacy_nav"]                           #@ missing legacy_nav
p["theme"]                                #@ missing theme
p2["legacy_nav"]                          #@ possibly legacy_nav
themed["limited"]                         #@ ok limited
