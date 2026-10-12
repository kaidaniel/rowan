"""What unknown code stores may be anything, so the items of such a value are anything too: their
rows are unknown, not empty. The element of an unknown value whose row had no source (only
"anything") got a closed, empty row, and every key read on it was definitely missing."""
import operator

box = {"p": [1]}
operator.setitem(box, "p", [{"pts": 1}])
for x in box["p"]:
    x["pts"]                              #@ sound pts
