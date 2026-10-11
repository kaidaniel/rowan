"""A generator's `while True:` is unrolled (its yields get positions, a consumer taking a few
items sees exactly those) only outside other loops and unrolled iterations: unrolling a
`while True:` nested in another multiplied the iterations per level (16 per level; email's
`FeedParser._parsegen` nests three). Here three nested levels made 10k objects and 53k
statements; with the outer loop alone unrolled, a few hundred objects. The record keys are the
same either way: every kind of record may come, `text` always does (argv[0] is a path)."""
#@objects <= 600
import sys


def sections(lines):
    it = iter(lines)
    while True:
        line = next(it, None)
        if line is None:
            return
        if line.startswith("["):
            while True:
                inner = next(it, None)
                if inner is None or inner == "[end]":
                    break
                if inner.startswith("<"):
                    while True:
                        deep = next(it, None)
                        if deep is None or deep == ">":
                            break
                        yield {"deep": deep}
                yield {"inner": inner}
        yield {"text": line}


seen = {}
for rec in sections(sys.argv):
    seen.update(rec)
print(sorted(seen))  #@row seen: present={text,inner,deep} definite_within={text} closed
