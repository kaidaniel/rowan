# unstack moves an index level into the columns (its values become column names); stack moves the columns into the index; reset_index names the levels
import pandas as pd


def unstacked(df):
    s = df.set_index(["day", "kind"])["amount"]
    w = s.unstack()
    #@row w: present={in,out} definite={in,out} absent={day,kind,amount} closed
    w["in"]                                        #@ ok in
    flat = w.reset_index()
    #@row flat: present={day,in,out} definite={day,in,out} closed
    w2 = s.unstack(level=0)
    #@row w2: present={mon,tue} definite={mon} absent={in,out,day} closed
    w2["mon"]                                      #@ ok mon
    w2["in"]                                       #@ missing in


def stacked(df):
    s = df.set_index("id").stack()
    long = s.reset_index(name="val")
    #@row long: present={id,level_1,val} definite={id,level_1,val} absent={a,b} closed
    named = s.rename_axis(["id", "col"]).reset_index(name="val")
    #@row named: present={id,col,val} definite={id,col,val} absent={level_1} closed
    return named


def round_trip(df):
    back = df.set_index(["day", "kind"])["amount"].unstack().stack().reset_index(name="amount")
    #@row back: present={day,kind,amount} definite={day,kind,amount} closed
    return back


if __name__ == "__main__":
    a = pd.DataFrame({"day": ["mon", "mon", "tue", "tue"], "kind": ["in", "out", "in", "out"], "amount": [1, 2, 3, 4]})
    b = pd.DataFrame({"day": ["mon", "mon"], "kind": ["in", "out"], "amount": [5, 6]})
    for f in (a, b):
        try:
            unstacked(f)
        except KeyError:
            pass
        round_trip(f)
    stacked(pd.DataFrame({"id": [1, 2], "a": [3, 4], "b": [5, 6]}))
    stacked(pd.DataFrame({"id": [7], "a": [8]}))
