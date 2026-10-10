"""Text and byte helpers on known values, inside functions too: bytes methods / bytes() /
str(b, enc), base64, shlex, html, string.capwords, fnmatch, urllib.parse (urlparse with
hostname / port, urlencode of a dict, parse_qs, urljoin, quote); `c in s` with s a name holding
a string tests substrings (both branches stay possible); hasattr on a dict answers by its type."""
import base64
import fnmatch
import html
import shlex
import string
from urllib.parse import parse_qs, quote, urlencode, urljoin, urlparse


def endpoint(url):
    u = urlparse(url)
    return u.hostname, u.port, u.path


def scheme_ok(text):
    allowed = "abcdefghijklmnopqrstuvwxyz"
    for c in text:
        if c not in allowed:
            return False
    return True


def as_pairs(q):
    if hasattr(q, "items"):
        return sorted(q.items())
    return list(q)


if __name__ == "__main__":
    host, port, path = endpoint("https://Api.Example.com:8443/v1/items?x=1")
    _ = 0                                        #@value host: exactly={"api.example.com"} ; port: exactly={8443} ; path: exactly={"/v1/items"}
    noport = endpoint("http://h/")[1]            #@value noport: exactly={None}
    q = urlencode({"q": "a b", "n": 2, "t": "x&y"})   #@value q: exactly={"q=a+b&n=2&t=x%26y"}
    p = parse_qs("a=1&a=2&b=3")                  #@row p: present={a,b} definite={a,b} closed
    a2 = p["a"][1]                               #@value a2: exactly={"2"}
    j = urljoin("http://h/a/b/c", "../d")        #@value j: exactly={"http://h/a/d"}
    qu = quote("a b/ü")                          #@value qu: exactly={"a%20b/%C3%BC"}
    ok1 = scheme_ok("https")                     #@value ok1: exactly={True}
    ok2 = scheme_ok("http2")                     #@value ok2: exactly={False}
    pairs = tuple(as_pairs({"b": 1, "a": 2}))    #@value pairs: exactly={(("a",2),("b",1))}
    b64 = base64.b64encode(b"hi").decode()       #@value b64: exactly={"aGk="}
    raw = base64.b64decode("aGk=")               #@value raw: exactly={b"hi"}
    txt = str(b"q", "ascii")                     #@value txt: exactly={"q"}
    hx = bytes.fromhex("6869").hex()             #@value hx: exactly={"6869"}
    parts = tuple(b"a,b".split(b","))            #@value parts: exactly={(b"a",b"b")}
    argv = tuple(shlex.split("cmd --x 'a b'"))   #@value argv: exactly={("cmd","--x","a b")}
    esc = html.escape("<a & 'b'>") == "&lt;a &amp; &#x27;b&#x27;&gt;"   #@value esc: exactly={True}
    cw = string.capwords("hello big world")      #@value cw: exactly={"Hello Big World"}
    fm = fnmatch.fnmatch("data_01.csv", "data_[0-9]?.csv")   #@value fm: exactly={True}
