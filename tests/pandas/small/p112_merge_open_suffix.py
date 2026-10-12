# merge of an open read_csv frame with a closed lookup sharing a non-key column: the plain name exists only if the file lacks it
import io

import pandas as pd

LOOKUP = pd.DataFrame({"cust": [10, 11], "name": ["a", "b"], "region": ["e", "w"]})
WITH = "cust,amount,name\n10,5.0,x\n11,6.0,y\n"
WITHOUT = "cust,amount\n10,5.0\n11,1.0\n"


def join(text):
    orders = pd.read_csv(io.StringIO(text))
    m = orders.merge(LOOKUP, on="cust")          #@ input cust
    # (first written definite={cust,region} / ok region: the file is open, and a region column
    # of its own would make region_x / region_y; no input here has one, so `sound`)
    #@row m: definite={cust} open
    m["region"]                                  #@ sound region
    m["amount"]                                  #@ input amount
    return m


#@schema read_csv@L12: always={cust,amount} only={cust,amount,name_y}


def plain(m):
    return m["name"]                             #@ possibly name


def suffixed(m):
    # (first written `possibly`: the open file may hold a name_y column itself)
    return m["name_y"]                           #@ input name_y


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for text in (WITH, WITHOUT):
        m = join(text)
        for fn in (plain, suffixed):
            try:
                fn(m)
            except KeyError as e:
                kept.append(e)
# (L22: `name_y` was in sometimes=; the read in suffixed() is behind its caller's try, a probe as in
#  pm05's loop of probes: required of no input; the oracle skips keys raised without dropping)
