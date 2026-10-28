"""On an instance of a class with `__slots__`, `del o.name` of a name that is not a slot (a
class attribute, or no attribute at all) raises AttributeError; of a slot that was never set
too; of a set slot it removes the attribute. (Before, no `del` on such an instance raised.)

Expectations were written from the program text before running the engine."""
import sys


class Point:
    __slots__ = ("x", "y")
    unit = "m"

    def __init__(self):
        self.x = 1


p = Point()
try:
    del p.unit
    r = p.unit
except AttributeError:
    r = "attr"
r_seen = r
#@value r_seen: exactly={"attr"}

try:
    del p.y
    s = "deleted"
except AttributeError:
    s = "attr"
s_seen = s
#@value s_seen: exactly={"attr"}

try:
    del p.x
    t = "deleted"
except AttributeError:
    t = "attr"
t_seen = t
#@value t_seen: exactly={"deleted"}

try:
    u = p.x
    #@ missing x
except AttributeError:
    u = "attr"
u_seen = u
#@value u_seen: exactly={"attr"}

if len(sys.argv) > 2:
    p.y = 3
try:
    del p.y
    w = "deleted"
except AttributeError:
    w = "attr"
w_seen = w
#@value w_seen: exactly={"deleted", "attr"}
