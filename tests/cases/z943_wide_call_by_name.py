"""A method called by a name read from data (`getattr(self, name)(…)`) may be any of the
class's two dozen methods, each calling back into the dispatcher. Running every one of them at
every such call, each again through the dispatcher, ran each function to its budget: a hundred
thousand statements for this file. More methods of one receiver than a wide read's bound at one
call are one call that does not run, for all of them (what each may change, what each may
return), beside the other callees.

Expectations were written from the program text before running the engine."""
import json
import sys

#@work <= 1500


class Pipeline:
    steps = json.loads(sys.stdin.read() or "{}")

    def run(self, tag, data):
        for name in self.steps.get(tag, []):
            data = getattr(self, name)(data, tag=tag)
        return data

    def step0(self, data, tag=None):
        if data:
            data = self.run("t1", data[1:])
        return {"n": 0, "data": data}

    def step1(self, data, tag=None):
        if data:
            data = self.run("t2", data[1:])
        return {"n": 1, "data": data}

    def step2(self, data, tag=None):
        if data:
            data = self.run("t3", data[1:])
        return {"n": 2, "data": data}

    def step3(self, data, tag=None):
        if data:
            data = self.run("t4", data[1:])
        return {"n": 3, "data": data}

    def step4(self, data, tag=None):
        if data:
            data = self.run("t5", data[1:])
        return {"n": 4, "data": data}

    def step5(self, data, tag=None):
        if data:
            data = self.run("t6", data[1:])
        return {"n": 5, "data": data}

    def step6(self, data, tag=None):
        if data:
            data = self.run("t7", data[1:])
        return {"n": 6, "data": data}

    def step7(self, data, tag=None):
        if data:
            data = self.run("t8", data[1:])
        return {"n": 7, "data": data}

    def step8(self, data, tag=None):
        if data:
            data = self.run("t9", data[1:])
        return {"n": 8, "data": data}

    def step9(self, data, tag=None):
        if data:
            data = self.run("t10", data[1:])
        return {"n": 9, "data": data}

    def step10(self, data, tag=None):
        if data:
            data = self.run("t11", data[1:])
        return {"n": 10, "data": data}

    def step11(self, data, tag=None):
        if data:
            data = self.run("t12", data[1:])
        return {"n": 11, "data": data}

    def step12(self, data, tag=None):
        if data:
            data = self.run("t13", data[1:])
        return {"n": 12, "data": data}

    def step13(self, data, tag=None):
        if data:
            data = self.run("t14", data[1:])
        return {"n": 13, "data": data}

    def step14(self, data, tag=None):
        if data:
            data = self.run("t15", data[1:])
        return {"n": 14, "data": data}

    def step15(self, data, tag=None):
        if data:
            data = self.run("t16", data[1:])
        return {"n": 15, "data": data}

    def step16(self, data, tag=None):
        if data:
            data = self.run("t17", data[1:])
        return {"n": 16, "data": data}

    def step17(self, data, tag=None):
        if data:
            data = self.run("t18", data[1:])
        return {"n": 17, "data": data}

    def step18(self, data, tag=None):
        if data:
            data = self.run("t19", data[1:])
        return {"n": 18, "data": data}

    def step19(self, data, tag=None):
        if data:
            data = self.run("t20", data[1:])
        return {"n": 19, "data": data}

    def step20(self, data, tag=None):
        if data:
            data = self.run("t21", data[1:])
        return {"n": 20, "data": data}

    def step21(self, data, tag=None):
        if data:
            data = self.run("t22", data[1:])
        return {"n": 21, "data": data}

    def step22(self, data, tag=None):
        if data:
            data = self.run("t23", data[1:])
        return {"n": 22, "data": data}

    def step23(self, data, tag=None):
        if data:
            data = self.run("t0", data[1:])
        return {"n": 23, "data": data}


r = Pipeline().run(sys.argv[0], sys.argv)
n = r["n"] if isinstance(r, dict) else -1
#@value n: includes={-1,0,23}
