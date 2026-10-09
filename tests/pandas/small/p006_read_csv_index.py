# read_csv with index_col= (the column leaves the frame), dtype=, skiprows=
import io

import pandas as pd

DATA = "id,zip,v\n1,0101,2.5\n2,0202,3.0\n"
JUNK = "# exported\nid,zip,v\n1,0101,2.5\n"


def indexed(text):
    df = pd.read_csv(io.StringIO(text), usecols=["key", "v"], index_col="key")
    #@row df: present={v} definite={v} absent={key} closed
    df["v"]                                      #@ ok v
    back = df.reset_index()
    #@row back: present={key,v} definite={key,v} closed
    df["key"]                                    #@ missing key


typed = pd.read_csv(io.StringIO(DATA), dtype={"zip": str})
#@row typed: open
typed["zip"]                                     #@ input zip
skipped = pd.read_csv(io.StringIO(JUNK), skiprows=1)
#@row skipped: open
skipped["v"]                                     #@ input v
ids = skipped["id"]                              #@ input id
by_id = skipped.set_index("id")
#@row by_id: definite={v} absent={id} open
by_id["v"]                                       #@ ok v
first = pd.read_csv(io.StringIO(DATA), index_col=0)
#@row first: open
first["v"]                                       #@ input v
#@schema read_csv@L19: always={zip} only={zip}
#@schema read_csv@L22: always={v,id} only={v,id}
#@schema read_csv@L29: always={v} only={v}
#@schema read_csv@L11: always={v} only={key,v}

if __name__ == "__main__":
    caught = []
    for text in ["key,v\n1,2.5\n", "v,key\n1.0,7\n", "key,v,w\n3,4,5\n6,7,8\n"]:
        try:
            indexed(text)
        except KeyError as e:
            caught.append(e)
