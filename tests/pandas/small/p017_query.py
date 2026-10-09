# df.query("..."): same columns as the frame; the names in the expression must be columns
import pandas as pd


def recent(df, year):
    r = df.query("year >= @year")
    #@row r: present={year,city,pop} definite={year,city} closed
    big = r.query("pop > 100 and city != 'x'")
    #@row big: present={year,city,pop} definite={year,city,pop} closed
    big["pop"]                                   #@ ok pop
    r["pop"]                                     #@ ok pop


def chained(df):
    out = df.query("year > 2000").sort_values("city").head(3)  #@ ok city
    #@row out: present={year,city,pop} definite={year,city} closed
    out["region"]                                #@ missing region


frame = pd.DataFrame({"year": [1999, 2024], "city": ["a", "b"], "pop": [5, 500]})
q = frame.query("`pop` > 10")                    #@row q: present={year,city,pop} definite={year,city,pop} closed
q["year"]                                        #@ ok year
q2 = frame.query("city in ['a', 'c']", inplace=False)  #@row q2: present={year,city,pop} definite={year,city,pop} closed

if __name__ == "__main__":
    caught = []
    for df in [frame, pd.DataFrame({"year": [2020], "city": ["z"], "pop": [1]}), pd.DataFrame({"year": [2001, 2002], "city": ["y", "x"]})]:
        try:
            recent(df, 2000)
        except Exception as e:
            caught.append(e)
        try:
            chained(df)
        except KeyError as e:
            caught.append(e)
