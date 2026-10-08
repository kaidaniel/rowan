"""Plugins registered by a decorator and instantiated by name; a visitor walks a tree of dict
nodes, dispatching on a key loaded from the node and recursing.

Expectations were written from the program text before running the engine."""
import sys

REGISTRY = {}


def plugin(name):
    def deco(cls):
        REGISTRY[name] = cls
        return cls
    return deco


class Plugin:
    def __init__(self, doc):
        self.doc = doc
        self.result = {}

    def run(self):
        raise NotImplementedError


@plugin("wordcount")
class WordCount(Plugin):
    def run(self):
        self.result["words"] = len(self.doc["body"].split())
        return self.result


@plugin("meta")
class Meta(Plugin):
    def run(self):
        self.result["title"] = self.doc["title"]
        if "author" in self.doc:
            self.result["author"] = self.doc["author"]
        return self.result


@plugin("links")
class Links(Plugin):
    def run(self):
        self.result["links"] = [l for l in self.doc.get("links", [])]
        self.result["count"] = len(self.result["links"])
        return self.result


def run_plugin(name, doc):
    inst = REGISTRY[name](doc)
    return inst.run()


def run_all(doc):
    results = {}
    for name in ["wordcount", "meta", "links"]:
        results[name] = run_plugin(name, doc)
    return results


reg = REGISTRY                            #@row reg: present={wordcount,meta,links} definite={wordcount,meta,links} closed
doc = {"title": "T", "body": "a b c", "author": "me"}
wc = run_plugin("wordcount", doc)         #@row wc: present={words} definite={words} closed
meta = run_plugin("meta", doc)            #@row meta: present={title,author} definite={title,author} closed
links = run_plugin("links", {"title": "x", "body": ""})  #@row links: present={links,count} definite={links,count} closed
anon = run_plugin("meta", {"title": "n", "body": ""})    #@row anon: present={title} definite={title} closed
all_r = run_all(doc)                      #@row all_r: present={wordcount,meta,links} definite={wordcount,meta,links} closed
m2 = all_r["meta"]                        #@row m2: present={title,author} definite={title,author} closed
chosen = run_plugin(sys.argv[1], doc)     #@row chosen: present={words,title,author,links,count} definite={} closed
chosen["title"]                           #@ possibly title


class Visitor:
    def __init__(self):
        self.seen = {"num": 0, "add": 0}
        self.depth_max = 0

    def visit(self, node, depth=0):
        if depth > self.depth_max:
            self.depth_max = depth
        handler = {"num": self.visit_num, "add": self.visit_add}[node["type"]]
        return handler(node, depth)

    def visit_num(self, node, depth):
        self.seen["num"] += 1
        return {"value": node["value"], "depth": depth}

    def visit_add(self, node, depth):
        self.seen["add"] += 1
        l = self.visit(node["left"], depth + 1)
        r = self.visit(node["right"], depth + 1)
        return {"value": l["value"] + r["value"], "depth": depth, "parts": [l, r]}


tree = {"type": "add",
        "left": {"type": "num", "value": 1},
        "right": {"type": "add", "left": {"type": "num", "value": 2}, "right": {"type": "num", "value": 3}}}
v = Visitor()
res = v.visit(tree)                       #@row res: present={value,depth,parts} definite={value,depth,parts} closed
part = res["parts"][0]                    #@row part: present={value,depth} definite={value,depth} closed
leaf = v.visit({"type": "num", "value": 9})  #@row leaf: present={value,depth} definite={value,depth} closed
seen = v.seen                             #@row seen: present={num,add} definite={num,add} closed
_ = 0                                     #@row v: present={seen,depth_max} definite={seen,depth_max} closed
