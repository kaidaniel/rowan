# pathlib.PurePosixPath on literal strings: name, stem, suffix, suffixes, parent, parts, with_suffix, / joins

from pathlib import PurePosixPath


def describe(path_text):
    p = PurePosixPath(path_text)
    info = {"name": p.name, "stem": p.stem, "suffix": p.suffix, "parent": str(p.parent)}
    return info


def target(job):
    src = PurePosixPath(job["src"])
    return str(PurePosixPath(job.get("dest", "build")) / src.with_suffix(".o").name)
#@schema param target.job: always={src} only={src}


a = describe("/srv/data/report.final.csv")
#@row a: present={name,stem,suffix,parent} definite={name,stem,suffix,parent} closed
nm = a["name"]                                    #@value nm: exactly={"report.final.csv"}
st = a["stem"]                                    #@value st: exactly={"report.final"}
sf = a["suffix"]                                  #@value sf: exactly={".csv"}
pa = a["parent"]                                  #@value pa: exactly={"/srv/data"}
bs = describe("README")["suffix"]                 #@value bs: exactly={""}
p = PurePosixPath("docs") / "api" / "index.md"
ps = str(p)                                       #@value ps: exactly={"docs/api/index.md"}
ws = str(p.with_suffix(".html"))                  #@value ws: exactly={"docs/api/index.html"}
wn = str(p.with_name("toc.md"))                   #@value wn: exactly={"docs/api/toc.md"}
parts = p.parts                                   #@value parts: exactly={("docs", "api", "index.md")}
nsx = len(PurePosixPath("a.tar.gz").suffixes)     #@value nsx: exactly={2}
rel = str(PurePosixPath("/a/b/c").relative_to("/a"))   #@value rel: exactly={"b/c"}
t1 = target({"src": "lib/a.c"})                   #@value t1: exactly={"build/a.o"}
t2 = target({"src": "b.c", "dest": "/out", "cc": "gcc"})   #@value t2: exactly={"/out/b.o"}
