# os.path on literal strings: join, basename, dirname, splitext, normpath, split, relpath — values follow

import os.path


def output_path(job):
    base, ext = os.path.splitext(os.path.basename(job["input"]))
    return os.path.join(job.get("outdir", "out"), base + ".json")
#@schema param output_path.job: always={input} only={input}


def layout(cfg):
    root = cfg["root"]
    return {"logs": os.path.join(root, "logs"), "data": os.path.join(root, cfg.get("data", "data"))}
#@schema param layout.cfg: always={root} only={root}


p1 = output_path({"input": "/data/in/report.csv"})              #@value p1: exactly={"out/report.json"}
p2 = output_path({"input": "x.tar.gz", "outdir": "/tmp/res"})   #@value p2: exactly={"/tmp/res/x.tar.json"}
d = os.path.dirname("/a/b/c.txt")                               #@value d: exactly={"/a/b"}
n = os.path.normpath("a//b/../c/")                              #@value n: exactly={"a/c"}
j = os.path.join("a", "/abs", "f")                              #@value j: exactly={"/abs/f"}
sp = os.path.split("dir/file.py")                               #@value sp: exactly={("dir", "file.py")}
ia = os.path.isabs("rel/x")                                     #@value ia: exactly={False}
ext = os.path.splitext("archive.zip")[1]                        #@value ext: exactly={".zip"}
rel = os.path.relpath("/a/b/c", "/a")                           #@value rel: exactly={"b/c"}
lay = layout({"root": "/srv"})                                  #@row lay: present={logs,data} definite={logs,data} closed
ld = lay["data"]                                                #@value ld: exactly={"/srv/data"}
lay2 = layout({"root": "r", "data": "d2", "x": 1})
ld2 = lay2["data"]                                              #@value ld2: exactly={"r/d2"}
