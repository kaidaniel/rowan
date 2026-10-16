# A text that grows in a loop of unknown count (`replace("", "ab")` triples it each time):
# rowan stops computing it once it is long, and the loop's fixpoint stays small.
import sys

n = len(sys.argv)
s = "ab"
for _ in range(n):
    s = s.replace("", "ab")
size = len(s)                                #@value size: types={int}
small = "ab".replace("", "-")                #@value small: exactly={"-a-b-"}
#@work <= 100
