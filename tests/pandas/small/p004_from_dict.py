# DataFrame.from_dict: orient="columns" (outer keys are columns) vs orient="index" (inner keys are)
import pandas as pd


def by_columns(d):
    df = pd.DataFrame.from_dict(d)
    #@row df: present={alice,bob} definite={alice} closed
    df["alice"]                                  #@ ok alice
    df["age"]                                    #@ missing age


def by_index(d):
    df = pd.DataFrame.from_dict(d, orient="index")
    #@row df: present={age,city} definite={age,city} absent={alice,bob} closed
    df["city"]                                   #@ ok city
    df["alice"]                                  #@ missing alice


def by_index_named(d):
    df = pd.DataFrame.from_dict(d, orient="index", columns=["years", "town"])
    #@row df: present={years,town} definite={years,town} absent={age,city} closed
    df["town"]                                   #@ ok town
    df["age"]                                    #@ missing age


people = {"alice": {"age": 30, "city": "Oslo"}, "bob": {"age": 25, "city": "Rome"}}
t = pd.DataFrame.from_dict(people, orient="index")
#@row t: present={age,city} definite={age,city} closed
t["age"]                                         #@ ok age
c = pd.DataFrame.from_dict(people, orient="columns")
#@row c: present={alice,bob} definite={alice,bob} closed
c["bob"]                                         #@ ok bob

if __name__ == "__main__":
    caught = []
    for d in [{"alice": {"age": 30, "city": "Oslo"}, "bob": {"age": 25, "city": "Rome"}}, {"alice": {"age": 1, "city": "X"}}]:
        try:
            by_columns(d)
        except KeyError as e:
            caught.append(e)
        try:
            by_index(d)
        except KeyError as e:
            caught.append(e)
    for d in [{"alice": [30, "Oslo"]}, {"alice": [30, "Oslo"], "bob": [25, "Rome"]}]:
        try:
            by_index_named(d)
        except KeyError as e:
            caught.append(e)
