# df.T / transpose(): the columns become the index labels; known when the index is literal
import pandas as pd


def flip(df):
    t = df.T
    #@row t: present={"r1","r2"} definite={"r1"} absent={x,y} closed
    t["r1"]                                      #@ ok "r1"
    back = t.T
    #@row back: present={x,y,z} definite={x} closed
    t["x"]                                       #@ missing x


frame = pd.DataFrame({"x": [1, 2], "y": [3, 4]}, index=["r1", "r2"])
ft = frame.T                                     #@row ft: present={"r1","r2"} definite={"r1","r2"} closed
ft["r2"]                                         #@ ok "r2"
tt = frame.transpose()                           #@row tt: present={"r1","r2"} definite={"r1","r2"} absent={x} closed
num = pd.DataFrame({"a": [1, 2]}).T              #@row num: present={0,1} definite={0,1} closed
num[0]                                           #@ ok 0
by_id = pd.DataFrame({"id": ["p", "q"], "v": [1, 2]}).set_index("id").T  #@row by_id: present={p,q} definite={p,q} closed

if __name__ == "__main__":
    caught = []
    for df in [frame, pd.DataFrame({"x": [9], "z": [8]}, index=["r1"])]:
        try:
            flip(df)
        except KeyError as e:
            caught.append(e)
