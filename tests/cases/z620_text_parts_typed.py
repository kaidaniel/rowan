"""Hypothesis: the parts of a text rowan does not know are texts too: `partition` and
`rpartition` give three str, `split`, `rsplit` and `splitlines` lists of str; of bytes, bytes.

Expectations were written from the program text before running the engine."""


def parse(line: str, raw: bytes):
    head, sep, rest = line.partition(":")
    #@general head: types={str}
    #@general rest: types={str}
    left, _, right = line.rpartition("=")
    #@general right: types={str}
    for word in rest.split(","):
        w = word
        #@general w: types={str}
    first = line.splitlines()[0]
    #@general first: types={str}
    for chunk in raw.split(b" "):
        c = chunk
        #@general c: types={bytes}
    return head, sep, left, right, first


if __name__ == "__main__":
    parse("a:b,c=d\ne", b"x y")
