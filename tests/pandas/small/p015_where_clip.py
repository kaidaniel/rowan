# element-wise frame methods keep the columns: where/mask/clip/round/abs and frame arithmetic
import pandas as pd


def tidy(df):
    pos = df.where(df > 0, 0)
    #@row pos: present={x,y,z} definite={x,y} closed
    neg = df.mask(df > 0)
    #@row neg: present={x,y,z} definite={x,y} closed
    c = df.clip(lower=-1, upper=1)
    #@row c: present={x,y,z} definite={x,y} closed
    r = df.round(1).abs()
    #@row r: present={x,y,z} definite={x,y} closed
    r["y"]                                       #@ ok y
    c["z"]                                       #@ ok z ; missing z


def scaled(df):
    half = df / 2
    #@row half: present={x,y,z} definite={x,y} closed
    sub = df[["x"]].clip(upper=df["y"], axis=0)  #@ ok y
    #@row sub: present={x} definite={x} closed
    sub["y"]                                     #@ missing y


frame = pd.DataFrame({"x": [-2.5, 3.25], "y": [1.0, -1.0]})
w = frame["x"].where(frame["y"] > 0)             # a Series: no columns
rounded = frame.round({"x": 0, "zz": 2})         #@row rounded: present={x,y} definite={x,y} absent={zz} closed
rounded["x"]                                     #@ ok x

if __name__ == "__main__":
    caught = []
    for df in [frame, pd.DataFrame({"x": [1.0], "y": [2.0], "z": [-3.0]}), pd.DataFrame({"y": [0.5], "x": [9.0]})]:
        try:
            tidy(df)
        except KeyError as e:
            caught.append(e)
        try:
            scaled(df)
        except KeyError as e:
            caught.append(e)
