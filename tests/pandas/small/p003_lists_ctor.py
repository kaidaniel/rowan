# DataFrame from a list of lists / tuples: columns= names them, without it the columns are 0..n-1
import pandas as pd


def named(rows):
    df = pd.DataFrame(rows, columns=["day", "temp", "rain"])
    #@row df: present={day,temp,rain} definite={day,temp,rain} closed
    df["temp"]                                   #@ ok temp
    df["wind"]                                   #@ missing wind


def numbered(rows):
    df = pd.DataFrame(rows)
    #@row df: present={0,1} definite={0,1} closed
    df[1]                                        #@ ok 1
    df["day"]                                    #@ missing day


def zipped(days, temps):
    df = pd.DataFrame(list(zip(days, temps)), columns=["day", "temp"])
    #@row df: present={day,temp} definite={day,temp} closed
    n = len(df.columns)                          #@value n: exactly={2}
    df["day"]                                    #@ ok day


grid = pd.DataFrame([[1, 2, 3], [4, 5, 6]], columns=list("xyz"))
#@row grid: present={x,y,z} definite={x,y,z} closed
grid["z"]                                        #@ ok z
last = list(grid.columns)[-1]                     #@value last: exactly={"z"}

if __name__ == "__main__":
    caught = []
    for rows in [[["mon", 20, 0.0]], [("tue", 18, 1.5), ("wed", 22, 0.2)], []]:
        try:
            named(rows)
        except KeyError as e:
            caught.append(e)
    for rows in [[[1, 2]], [("a", "b"), ("c", "d")]]:
        try:
            numbered(rows)
        except KeyError as e:
            caught.append(e)
    zipped(["mon"], [20])
    zipped([], [])
    zipped(["a", "b", "c"], [1, 2, 3])
