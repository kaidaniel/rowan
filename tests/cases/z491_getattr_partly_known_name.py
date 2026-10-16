"""getattr by a name known only in part (`f"run_{mode}"` with mode from the environment or
"0"): any attribute whose name fits `run_*`, each call runs the 30 methods (bound 400 -> 1000):
`r` is not run_0's 0 only. (An f-string of such a name was exactly its literal's text,
unsound; made fully unknown, every attribute read for it cost sktime 70 s -> past 4 minutes.)
Expectations and the cost bound were written from the program text.
"""
import os

mode = os.environ.get("Z491_MODE", "0")


class Job:
    def __init__(self):
        self.n = 0

    def run_0(self):
        self.n += 0
        return self.n

    def run_1(self):
        self.n += 1
        return self.n

    def run_2(self):
        self.n += 2
        return self.n

    def run_3(self):
        self.n += 3
        return self.n

    def run_4(self):
        self.n += 4
        return self.n

    def run_5(self):
        self.n += 5
        return self.n

    def run_6(self):
        self.n += 6
        return self.n

    def run_7(self):
        self.n += 7
        return self.n

    def run_8(self):
        self.n += 8
        return self.n

    def run_9(self):
        self.n += 9
        return self.n

    def run_10(self):
        self.n += 10
        return self.n

    def run_11(self):
        self.n += 11
        return self.n

    def run_12(self):
        self.n += 12
        return self.n

    def run_13(self):
        self.n += 13
        return self.n

    def run_14(self):
        self.n += 14
        return self.n

    def run_15(self):
        self.n += 15
        return self.n

    def run_16(self):
        self.n += 16
        return self.n

    def run_17(self):
        self.n += 17
        return self.n

    def run_18(self):
        self.n += 18
        return self.n

    def run_19(self):
        self.n += 19
        return self.n

    def run_20(self):
        self.n += 20
        return self.n

    def run_21(self):
        self.n += 21
        return self.n

    def run_22(self):
        self.n += 22
        return self.n

    def run_23(self):
        self.n += 23
        return self.n

    def run_24(self):
        self.n += 24
        return self.n

    def run_25(self):
        self.n += 25
        return self.n

    def run_26(self):
        self.n += 26
        return self.n

    def run_27(self):
        self.n += 27
        return self.n

    def run_28(self):
        self.n += 28
        return self.n

    def run_29(self):
        self.n += 29
        return self.n

    def go(self):
        return getattr(self, f"run_{mode}")()


j = Job()
for _ in range(10):
    r = j.go()
#@value r: unknown
#@work <= 1000
