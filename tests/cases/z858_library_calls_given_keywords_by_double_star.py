"""Keywords given through `**kw` reach a library function: a dict of known keys is spread into
its keywords, and one whose keys are not known makes the call of a standard library model
unknown. `csv.writer(f, **kw)` with `quoting=csv.QUOTE_ALL` quotes every field. (Before, the
model never saw the keywords given by `**`: it wrote the rows as with the defaults.) A csv
reader of text it does not model (`strict=True`) may raise csv.Error.

Expectations were written from the program text before running the engine."""
import csv
import io
import json


def write(row, **kw):
    f = io.StringIO()
    csv.writer(f, **kw).writerow(row)
    return f.getvalue()


quoted = write(["a", 1], quoting=csv.QUOTE_ALL)
#@value quoted: includes={"\"a\",\"1\"\r\n"}
semi = write(["a", 1], delimiter="|")
#@value semi: exactly={"a|1\r\n"}
plain = write(["a", 1])
#@value plain: exactly={"a,1\r\n"}

opts = {"sort_keys": True}
dumped = json.dumps({"b": 1, "a": 2}, **opts)
sorted_keys = dumped == '{"a": 2, "b": 1}'
#@value sorted_keys: exactly={True}

try:
    rows = list(csv.reader(io.StringIO('"abc\n'), strict=True))
    got = "rows"
except csv.Error:
    got = "error"
got_seen = got
#@value got_seen: includes={"error"}
