# Two reads that may fail where rowan can only guess: a key computed from data on a closed
# dict, and an attribute nothing in the program sets but code rowan cannot see was given the
# object. Both still raise into their handlers (the values below), but neither is a warning:
# the report and the editor show only `missing` and `possibly` (vscode-rowan/test.js checks
# that these lines have no diagnostic).
import sys
import lib

PRICES = {"tea": 2, "cake": 3}
item = sys.argv[1]
try:
    price = PRICES[item]
except KeyError:
    price = 0
cost = price                         #@value cost: exactly={0,2,3}


class Box:
    def __init__(self):
        self.size = 1


box = Box()
lib.fill(box)
try:
    label = box.label                #@ likely-missing label
except AttributeError:
    label = None
shown = label                        #@value shown: includes={None}
size = box.size                      #@ likely size
# (the same reads unguarded: verdicts in the document, still no diagnostic)
fee = PRICES[item]
tag = box.label                      #@ likely-missing label
