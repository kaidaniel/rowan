# row subsets keep the columns: head/tail/sample/sort_values/nlargest/nsmallest (by= is required)
import pandas as pd


def top(df, n):
    h = df.head(n)
    #@row h: present={name,score,team} definite={name,score} closed
    t = df.tail(1)
    #@row t: present={name,score,team} definite={name,score} closed
    s = df.sample(frac=1.0, random_state=0)
    #@row s: present={name,score,team} definite={name,score} closed
    best = df.sort_values("score", ascending=False)  #@ ok score
    #@row best: present={name,score,team} definite={name,score} closed
    big = df.nlargest(2, "score")                #@ ok score
    #@row big: present={name,score,team} definite={name,score} closed
    small = df.nsmallest(1, "team")              #@ ok team ; missing team


def by_two(df):
    out = df.sort_values(["team", "score"])      #@ ok team ; ok score ; missing team
    #@row out: present={name,score,team} definite={name,score,team} closed
    out["team"]                                  #@ ok team


frame = pd.DataFrame({"name": ["a", "b", "c"], "score": [3, 9, 1]})
ranked = frame.sort_values(by="score").head(2)   #@row ranked: present={name,score} definite={name,score} closed
ranked["name"]                                   #@ ok name

if __name__ == "__main__":
    caught = []
    for df, n in [(frame, 2), (pd.DataFrame({"name": ["x"], "score": [5], "team": [7]}), 0), (pd.DataFrame({"name": [], "score": [], "team": []}), 5)]:
        try:
            top(df, n)
        except KeyError as e:
            caught.append(e)
        try:
            by_two(df)
        except KeyError as e:
            caught.append(e)
