# takewhile / dropwhile with a predicate reading a key: the split happens at the first failing record

from itertools import dropwhile, takewhile


def is_header(line):
    return line["kind"] == "header"
#@schema param is_header.line: always={kind} only={kind}


def split_doc(lines):
    head = list(takewhile(is_header, lines))
    body = list(dropwhile(is_header, lines))
    if body:
        first = body[0]
        #@row first: present={kind,text,num} definite={kind,text} closed
        first["text"]                             #@ ok text
    return len(head), len(body)


a = split_doc([{"kind": "header", "text": "t"}, {"kind": "row", "text": "x", "num": 1}, {"kind": "header", "text": "u"}])
#@value a: exactly={(1, 2)}
b = split_doc([{"kind": "row", "text": "y"}])       #@value b: exactly={(0, 1)}
c = split_doc([])                                   #@value c: exactly={(0, 0)}
small = list(takewhile(lambda x: x < 3, [1, 2, 5, 1]))
ns = len(small)                                     #@value ns: exactly={2}
rest = list(dropwhile(lambda x: x < 3, [1, 2, 5, 1]))
r0 = rest[0]                                        #@value r0: exactly={5}
