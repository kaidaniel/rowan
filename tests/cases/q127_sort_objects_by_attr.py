"""Hypothesis: sorting instances by an attribute with known values gives a known order, so
the first instance's optional attribute (set only on some) is decided.

Expectations were written from the program text before running the engine."""


class Job:
    def __init__(self, name, pri, urgent=False):
        self.name = name
        self.pri = pri
        if urgent:
            self.flag = "urgent"


jobs = [Job("a", 3), Job("b", 1, urgent=True), Job("c", 2)]
ordered = sorted(jobs, key=lambda j: j.pri)
first = ordered[0]                        #@row first: present={name,pri,flag} definite={name,pri,flag} closed
last = ordered[-1]                        #@row last: present={name,pri} definite={name,pri} closed
last.flag                                 #@ missing flag
first.flag                                #@ ok flag
