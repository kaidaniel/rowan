# csv.DictWriter with fieldnames: missing keys get restval, extra keys raise ValueError
# (extrasaction="raise") or are dropped ("ignore"); csv.reader / csv.writer on lists
import csv
import io


def write(rows, fields, extras="raise"):
    buf = io.StringIO()
    w = csv.DictWriter(buf, fieldnames=fields, extrasaction=extras, delimiter="|")
    w.writeheader()
    for r in rows:
        w.writerow(r)
    return buf.getvalue()


def safe_write(rows, fields):
    try:
        return write(rows, fields)
    except ValueError:
        return "rejected"


if __name__ == "__main__":
    a = safe_write([{"a": 1, "b": 2}], ["a", "b"])  #@value a: exactly={"a|b\r\n1|2\r\n"}
    b = safe_write([{"a": 1}], ["a", "b"])       #@value b: exactly={"a|b\r\n1|\r\n"}
    c = safe_write([{"a": 1, "c": 3}], ["a"])    #@value c: exactly={"rejected"}
    d = write([{"a": 1, "c": 3}], ["a"], "ignore")  #@value d: exactly={"a\r\n1\r\n"}
    buf = io.StringIO()
    cw = csv.writer(buf, delimiter="|")
    cw.writerow(["x", 1])
    out = buf.getvalue()                         #@value out: exactly={"x|1\r\n"}
    rows = list(csv.reader(io.StringIO("p|q\n1|2\n"), delimiter="|"))
    n = len(rows)                                #@value n: exactly={2}
    hd = tuple(rows[0])                          #@value hd: exactly={("p","q")}
    back = list(csv.DictReader(io.StringIO(a), delimiter="|"))
    first = back[0]
    first["a"]                                   #@ input a
