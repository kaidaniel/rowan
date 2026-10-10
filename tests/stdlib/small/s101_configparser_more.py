# configparser: continuation lines, %(name)s interpolation, getboolean, options order, set / add_section, read_dict; a file read at run time is an open input
import configparser

TEXT = """
[paths]
root = /srv
data = %(root)s/data
motd = first line
    second line

[flags]
debug = yes
verbose = Off
"""


def flags(cp):
    d = cp.getboolean("flags", "debug")
    v = cp["flags"].getboolean("verbose")
    q = cp.getboolean("flags", "quiet", fallback=False)
    return d, v, q


def from_file(path):
    cp = configparser.ConfigParser()
    cp.read(path)
    url = cp["db"]["url"]                        #@ input url
    return url
#@schema configparser.ConfigParser@L25: always={db} only={db}
#@schema configparser.ConfigParser@L25['db']: always={url} only={url}


if __name__ == "__main__":
    cp = configparser.ConfigParser()
    cp.read_string(TEXT)
    data = cp.get("paths", "data")               #@value data: exactly={"/srv/data"}
    raw = cp.get("paths", "data", raw=True)      #@value raw: exactly={"%(root)s/data"}
    motd = cp["paths"]["motd"]                   #@value motd: exactly={"first line\nsecond line"}
    d, v, q = flags(cp)
    #@value d: exactly={True} ; v: exactly={False} ; q: exactly={False}
    opts = tuple(cp.options("paths"))            #@value opts: exactly={("root","data","motd")}
    has = cp.has_option("flags", "DEBUG")        #@value has: exactly={True}
    cp.add_section("extra")
    cp.set("extra", "k", "v")
    ex = cp["extra"]                             #@row ex: present={k} definite={k} closed
    two = configparser.ConfigParser()
    two.read_dict({"a": {"X": 1}, "b": {}})
    secs = tuple(two.sections())                 #@value secs: exactly={("a","b")}
    x = two["a"]["x"]                            #@value x: exactly={"1"}
    try:
        two.get("c", "x")
    except configparser.NoSectionError:
        pass
    raw_cp = configparser.RawConfigParser()
    raw_cp.read_string("[s]\nv = %(a)s\n")
    rv = raw_cp.get("s", "v")                    #@value rv: exactly={"%(a)s"}
    try:
        u = from_file("app.ini")
    except KeyError:
        pass
