# to_dict("records") after drop / rename / new column: each record has exactly the edited frame's columns
import pandas as pd


def cleaned(df):
    out = df.drop(columns=["tmp"]).rename(columns={"qty": "n"})
    out["total"] = out["n"] * 2                          #@ ok n
    recs = out.to_dict("records")
    for r in recs:
        t = r["total"]                                   #@ ok total
        #@row r: present={n,total,note} definite={n,total} absent={qty,tmp} closed
        r.get("note")
    return recs


def first_qty(recs):
    return recs[0]["qty"]                                #@ missing qty


def first_note(recs):
    return recs[0]["note"]                               #@ ok note ; missing note


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    a = pd.DataFrame({"qty": [1, 2], "tmp": [0, 0], "note": ["x", "y"]})
    b = pd.DataFrame({"qty": [3], "tmp": [1]})
    ra = cleaned(a)
    rb = cleaned(b)
    for recs in (ra, rb):
        try:
            first_qty(recs)
        except KeyError as e:
            caught.append(e)
    first_note(ra)
    try:
        first_note(rb)
    except KeyError as e:
        caught.append(e)
