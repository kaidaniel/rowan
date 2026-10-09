# "%(key)s" % record reads the keys (KeyError when absent); positional %-formatting of literals


def line(rec):
    return "%(name)s owes %(amount).2f" % rec    #@ ok name ; ok amount ; missing amount
#@schema param line.rec: always={name,amount} only={name,amount}


def header(meta):
    title = "%(title)s" % meta                   #@ ok title
    if "sub" in meta:
        title += " - %(sub)s" % meta             #@ ok sub
    return title


if __name__ == "__main__":
    s = line({"name": "ann", "amount": 3})       #@value s: exactly={"ann owes 3.00"}
    line({"name": "bob", "amount": 1.5, "id": 7})
    try:
        line({"name": "cy"})
    except KeyError:
        pass
    h1 = header({"title": "A"})                  #@value h1: exactly={"A"}
    h2 = header({"title": "B", "sub": "c"})      #@value h2: exactly={"B - c"}
    p = "%s-%03d" % ("id", 7)                    #@value p: exactly={"id-007"}
    q = "%x|%5.1f|%%" % (255, 2.25)              #@value q: exactly={"ff|  2.2|%"}
