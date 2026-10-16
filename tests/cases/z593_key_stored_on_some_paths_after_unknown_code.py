# A dict code rowan cannot see was given may have any key with any value; a key the program
# stores on some paths only holds, on the others, whatever that code put there.
import sys
import xml.dom.minidom as somelib

n = len(sys.argv)
f = {}
somelib.something(f)
if n > 2:
    f["x"] = 1
c = f.get("x", -1)                           #@value c: unknown
