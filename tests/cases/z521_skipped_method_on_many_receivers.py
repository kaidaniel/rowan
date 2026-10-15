# A method past its budget called on a value that holds many receivers (a theme of styles read
# by an unknown name) is one call past the budget for all of them: what the receivers hold
# under the names the method reads is walked as unknown code once, not once per receiver; each
# receiver keeps the attributes the method does not store.
#@limits budget=100
#@havoc <= 300

import sys

CONFIG = {"k%d" % i: {"v": i} for i in range(20)}


class Style:
    def __init__(self, n):
        self.n = n
        self.tag = "t"
        self.cfg = CONFIG

    def bump(self):
        for j in range(40):
            a = j
        self.n = self.n + 1
        return self.cfg.get("k1")


THEME = {
    "s0": Style(0),
    "s1": Style(1),
    "s2": Style(2),
    "s3": Style(3),
    "s4": Style(4),
    "s5": Style(5),
    "s6": Style(6),
    "s7": Style(7),
    "s8": Style(8),
    "s9": Style(9),
    "s10": Style(10),
    "s11": Style(11),
    "s12": Style(12),
    "s13": Style(13),
    "s14": Style(14),
    "s15": Style(15),
    "s16": Style(16),
    "s17": Style(17),
    "s18": Style(18),
    "s19": Style(19),
    "s20": Style(20),
    "s21": Style(21),
    "s22": Style(22),
    "s23": Style(23),
    "s24": Style(24),
    "s25": Style(25),
    "s26": Style(26),
    "s27": Style(27),
    "s28": Style(28),
    "s29": Style(29),
}

name = sys.argv[0]
for i in range(10):
    s = THEME.get(name, THEME["s0"])
    s.bump()
n0 = THEME["s0"].n  #@value n0: includes={0,10}
t1 = THEME["s1"].tag  #@ ok tag
#@value t1: exactly={"t"}
v = CONFIG["k2"]  #@ sound k2
