# iterrows over a frame that may have no rows: what the loop body read or stored is not known after the loop
import io

import pandas as pd

ROWS = "id,qty,note\n1,2,a\n2,3,b\n"
NONE = "id,qty\n"


def scan(text):
    df = pd.read_csv(io.StringIO(text))
    total = 0
    for _, row in df.iterrows():
        total += row["note"].count("a") + row["qty"]   #@ input note ; input qty
        #@row row: definite={note,qty} open
    q = df["qty"]                                # the loop may not have run: still the file's
    #@row df: definite={qty} open
    return total


#@schema read_csv@L11: always={qty} sometimes={note} only={qty,note}


def mark(df):
    for i, row in df.iterrows():
        df.loc[i, "seen"] = 1                    # adds the column only when a row exists
    #@row df: present={id,seen} definite={id} closed
    return df


def seen(df):
    return df["seen"]                            #@ possibly seen


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    scan(ROWS)
    scan(NONE)
    for f in (pd.DataFrame({"id": [1, 2]}), pd.DataFrame({"id": []})):
        try:
            seen(mark(f))
        except KeyError as e:
            kept.append(e)
