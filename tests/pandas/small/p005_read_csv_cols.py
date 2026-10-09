# read_csv: open without arguments; usecols= / names= close it; header=None numbers the columns
import io

import pandas as pd

SALES = "region,amount,rep,year\nnorth,10,ann,2024\nsouth,5,bob,2025\n"
RAW = "north,10,ann\nsouth,5,bob\n"


def used(text):
    df = pd.read_csv(io.StringIO(text), usecols=["region", "amount"])
    #@row df: present={region,amount} definite={region,amount} absent={rep,year} closed
    df["amount"]                                 #@ ok amount
    df["rep"]                                    #@ missing rep


def numbered(text):
    df = pd.read_csv(io.StringIO(text), header=None)
    #@row df: open
    df[0]                                        #@ input 0
    df["zone"]                                   #@ input zone


full = pd.read_csv(io.StringIO(SALES))
#@row full: open
full["rep"]                                      #@ input rep
named = pd.read_csv(io.StringIO(RAW), names=["region", "amount", "rep"])
#@row named: present={region,amount,rep} definite={region,amount,rep} closed
named["amount"]                                  #@ ok amount
renamed = pd.read_csv(io.StringIO(SALES), header=0, names=["r", "a", "p", "y"])
#@row renamed: present={r,a,p,y} definite={r,a,p,y} absent={region} closed
renamed["y"]                                     #@ ok y
# (written as L23 at first: the read of `full` is on line 24; the oracle did not check yet
# that a schema line holds a read)
#@schema read_csv@L24: always={rep} only={rep}
#@schema read_csv@L11: always={amount} only={amount,region}

if __name__ == "__main__":
    caught = []
    for text in [SALES, "region,amount\nwest,1\n", "amount,region,extra\n3,east,z\n"]:
        try:
            used(text)
        except KeyError as e:
            caught.append(e)
    for text in [RAW, "1,2\n3,4\n"]:
        try:
            numbered(text)
        except KeyError as e:
            caught.append(e)
