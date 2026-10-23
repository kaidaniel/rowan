"""A generator made at one place for each of many calls, its first item taken there and the
rest later (a YAML-style constructor: `next(gen)` gives the empty record, the loop at the end
fills it). Each run is watched for what the code between its items changes; every consumer
state where an item is asked for was joined into each run's watch, one join per run per item:
work in runs x items. Joined once per item for all of them, the answers are the same."""
import json
import os


class Constructor:
    def __init__(self, deep=False):
        self.pending = []
        self.deep = deep

    def construct_map(self, node):
        data = {}
        yield data
        for k in node:
            data[k] = self.construct_object(node[k])

    def construct_seq(self, node):
        data = []
        yield data
        for x in node:
            data.append(self.construct_object(x))

    def construct_object(self, node):
        if isinstance(node, dict):
            gen = self.construct_map(node)
        elif isinstance(node, list):
            gen = self.construct_seq(node)
        else:
            return node
        data = next(gen)
        if self.deep:
            for _ in gen:
                pass
        else:
            self.pending.append(gen)
        return data

    def construct_document(self, nodes):
        out = [self.construct_object(n) for n in nodes]
        while self.pending:
            gens = self.pending
            self.pending = []
            for g in gens:
                for _ in g:
                    pass
        return out


c = Constructor()
made = c.construct_object({"x": 1})
#@row made: present={} closed
rest = list(c.pending[0])
#@row made: present={x} definite={x} closed
nodes = json.loads(os.environ.get("Z794_NODES", '[{"a": 1}, {"b": 2}, {"a": 3, "c": 4}]'))
docs = Constructor().construct_document(nodes)
first = docs[0]  #@ sound 0
n = len(docs)
#@value n: types={int}
# (each record a run of construct_map / construct_seq, of the same generators again and again)
deep = Constructor(deep=True).construct_object(nodes)
# (bound: each consumer state where an item is asked for joined into every run's watch was
# about 46,000 row joins here; once per generator, about 27,000)
#@joins <= 32000
