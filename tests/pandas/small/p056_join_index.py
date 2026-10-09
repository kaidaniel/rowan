# DataFrame.join: aligns on the index (or on=column against the other's index); lsuffix / rsuffix on overlap
import pandas as pd

PEOPLE = pd.DataFrame({"name": ["a", "b"], "age": [30, 40]}, index=[1, 2])
CITY = pd.DataFrame({"city": ["x", "y"]}, index=[1, 2])
CITY_AGE = pd.DataFrame({"city": ["x"], "age": [99]}, index=[2])


def attach(p, extra):
    j = p.join(extra, lsuffix="_p", rsuffix="_e")
    #@row j: present={name,age,city,age_p,age_e} definite={name,city} closed
    j["city"]                                            #@ ok city
    return j


def age(j):
    return j["age"]                                      #@ possibly age


def by_key(p, extra):
    q = p.reset_index().rename(columns={"index": "pid"})
    #@row q: present={pid,name,age} definite={pid,name,age} closed
    j = q.join(extra[["city"]], on="pid")                #@ ok city
    #@row j: present={pid,name,age,city} definite={pid,name,age,city} closed
    return j


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    age(attach(PEOPLE, CITY))
    try:
        age(attach(PEOPLE, CITY_AGE))
    except KeyError as e:
        kept.append(e)
    by_key(PEOPLE, CITY)
    by_key(PEOPLE, CITY_AGE)
