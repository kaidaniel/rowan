# reindex(columns=[...]): exactly those columns, missing ones NaN-filled; reindex(index) keeps columns
import pandas as pd


def conform(df):
    out = df.reindex(columns=["id", "amt", "fee"])
    #@row out: present={id,amt,fee} definite={id,amt,fee} absent={extra} closed
    out["fee"]                                   #@ ok fee
    rows = df.reindex([0, 1, 2])
    #@row rows: present={id,amt,extra} definite={id} closed
    filled = df.reindex(columns=["id", "zz"], fill_value=0)
    #@row filled: present={id,zz} definite={id,zz} closed
    out["extra"]                                 #@ missing extra


def like(df, template):
    out = df.reindex_like(template)
    #@row out: present={id,amt} definite={id,amt} closed
    out["amt"]                                   #@ ok amt


frame = pd.DataFrame({"id": [1], "amt": [2.0]})
wide = frame.reindex(columns=["amt", "id", "new"])  #@row wide: present={amt,id,new} definite={amt,id,new} closed
wide["new"]                                      #@ ok new
ax = frame.reindex(["id"], axis=1)               #@row ax: present={id} definite={id} absent={amt} closed

if __name__ == "__main__":
    caught = []
    for df in [frame, pd.DataFrame({"id": [7, 8], "extra": ["x", "y"]}), pd.DataFrame({"id": [], "amt": [], "extra": []})]:
        try:
            conform(df)
        except KeyError as e:
            caught.append(e)
        like(df, frame)
