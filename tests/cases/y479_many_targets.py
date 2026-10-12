"""An argument that is one of many objects (forty dicts here): its entry row is not the
join of theirs (a row of every key of every object, concretised at each access of the
callee's summary: rich's renderers took seconds per call), but unknown; the values the
callee reads stay sound."""
import sys

ROWS = [{"k": 0, "n0": 1}, {"k": 1, "n1": 1}, {"k": 2, "n2": 1}, {"k": 3, "n3": 1}, {"k": 4, "n4": 1}, {"k": 5, "n5": 1}, {"k": 6, "n6": 1}, {"k": 7, "n7": 1}, {"k": 8, "n8": 1}, {"k": 9, "n9": 1}, {"k": 10, "n10": 1}, {"k": 11, "n11": 1}, {"k": 12, "n12": 1}, {"k": 13, "n13": 1}, {"k": 14, "n14": 1}, {"k": 15, "n15": 1}, {"k": 16, "n16": 1}, {"k": 17, "n17": 1}, {"k": 18, "n18": 1}, {"k": 19, "n19": 1}, {"k": 20, "n20": 1}, {"k": 21, "n21": 1}, {"k": 22, "n22": 1}, {"k": 23, "n23": 1}, {"k": 24, "n24": 1}, {"k": 25, "n25": 1}, {"k": 26, "n26": 1}, {"k": 27, "n27": 1}, {"k": 28, "n28": 1}, {"k": 29, "n29": 1}, {"k": 30, "n30": 1}, {"k": 31, "n31": 1}, {"k": 32, "n32": 1}, {"k": 33, "n33": 1}, {"k": 34, "n34": 1}, {"k": 35, "n35": 1}, {"k": 36, "n36": 1}, {"k": 37, "n37": 1}, {"k": 38, "n38": 1}, {"k": 39, "n39": 1}]


def get(d):
    return d["k"]


x = ROWS[len(sys.argv)]
v = get(x)
#@value v: includes={0, 1, 39}
