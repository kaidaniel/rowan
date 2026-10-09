# df.filter(items= / like= / regex=): items not present are skipped silently, no KeyError
import pandas as pd


def by_items(df):
    sub = df.filter(items=["id", "amt_net", "zz"])
    #@row sub: present={id,amt_net} definite={id} absent={zz,amt_gross} closed
    sub["id"]                                    #@ ok id
    sub["zz"]                                    #@ missing zz


def by_like(df):
    amts = df.filter(like="amt")
    #@row amts: present={amt_net,amt_gross} absent={id,qty} closed
    q = df.filter(regex="^q")
    #@row q: present={qty} absent={id,amt_net} closed
    amts["id"]                                   #@ missing id


frame = pd.DataFrame({"id": [1], "amt_net": [2.0], "amt_gross": [2.5], "qty": [3]})
nets = frame.filter(items=["amt_net", "qty"])    #@row nets: present={amt_net,qty} definite={amt_net,qty} closed
liked = frame.filter(like="amt")                 #@row liked: present={amt_net,amt_gross} definite={amt_net,amt_gross} closed
liked["amt_gross"]                               #@ ok amt_gross
rx = frame.filter(regex="_(net|gross)$")         #@row rx: present={amt_net,amt_gross} definite={amt_net,amt_gross} closed
by_rows = frame.filter(items=[0], axis=0)        #@row by_rows: present={id,amt_net,amt_gross,qty} definite={id,amt_net,amt_gross,qty} closed

if __name__ == "__main__":
    caught = []
    for df in [frame, pd.DataFrame({"id": [1, 2], "qty": [0, 1]}), pd.DataFrame({"amt_net": [1.0], "id": [9], "extra": [0]})]:
        try:
            by_items(df)
        except KeyError as e:
            caught.append(e)
        try:
            by_like(df)
        except KeyError as e:
            caught.append(e)
