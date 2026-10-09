# str.extract: columns = named groups (else 0, 1, …); expand=False with one group gives a Series; extractall the same columns
import pandas as pd

PATTERN = r"(?P<user>[a-z]+)@(?P<host>[a-z.]+)"


def parts(df):
    ex = df["email"].str.extract(PATTERN)                #@row ex: present={user,host} definite={user,host} closed
    pos = df["email"].str.extract(r"([a-z]+)@([a-z.]+)")   #@row pos: present={0,1} definite={0,1} closed
    one = df["email"].str.extract(r"(?P<user>[a-z]+)@")   #@row one: present={user} definite={user} closed
    every = df["email"].str.extractall(PATTERN)          #@row every: present={user,host} definite={user,host} closed
    joined = df.join(ex)                                 #@row joined: present={email,age,user,host} definite={email,user,host} closed
    ex["host"]                                           #@ ok host
    return ex["domain"]                                  #@ missing domain


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"email": ["ada@math.org", "alan@kings.uk"]})
    f2 = pd.DataFrame({"email": ["grace@navy.mil", "bad address"], "age": [85, 0]})
    for frame in (f1, f2):
        try:
            parts(frame)
        except KeyError as e:
            caught.append(e)
