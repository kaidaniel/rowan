# csv.DictReader: without fieldnames rows are open (keys from the data); with fieldnames= they are
# those names, plus restkey for long rows
import csv
import io


def total_qty(text):  # (every call passes a literal text: the rows are known; was input qty / definite={qty} open)
    total = 0
    for row in csv.DictReader(io.StringIO(text)):
        q = row["qty"]                           #@ ok qty
        _ = 0                                    #@row row: present={id,qty,note,sku} definite={qty} closed
        total += int(q or 0)
        note = row.get("note")                   #@ none note
    return total


def read_fixed(text):
    rows = list(csv.DictReader(io.StringIO(text), fieldnames=["id", "name"], restkey="rest"))
    for r in rows:
        _ = 0                                    #@row r: present={id,name,rest} definite={id,name} closed
        r["id"]                                  #@ ok id
    return rows


def read_plain(text):
    rows = list(csv.DictReader(io.StringIO(text), fieldnames=("a", "b")))
    for r in rows:
        _ = 0                                    #@row r: present={a,b,None} definite={a,b} closed
    return rows


if __name__ == "__main__":
    t = total_qty("id,qty\n1,3\n2,\n")           #@value t: exactly={3}
    total_qty("qty,note,sku\n5,x,a\n")
    total_qty("qty\n")
    rows = read_fixed("1,a\n2,b,x,y\n3\n")
    n = len(rows)                                #@value n: exactly={3}
    nm = rows[2]["name"]                         #@value nm: exactly={None}
    read_fixed("")
    p = read_plain("1,2,3\n4,5\n")
