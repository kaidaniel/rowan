# Web log analysis: raw lines parsed with str.extract named groups, bot filtering on an optional column, sessionisation with diff/shift/cumsum, per-user and per-session aggregates, top pages via value_counts, a funnel reindexed to fixed pages
import io

import pandas as pd

PATTERN = r"(?P<user>\w+) (?P<method>GET|POST) (?P<path>\S+) (?P<status>\d{3}) (?P<bytes>\d+)"
FUNNEL = ["/home", "/cart", "/checkout"]

LOGS_A = """ts,line,agent
2024-05-01 10:00:00,alice GET /home 200 512,Mozilla
2024-05-01 10:05:00,alice GET /cart 200 128,Mozilla
2024-05-01 10:50:00,alice POST /checkout 500 0,Mozilla
2024-05-01 10:01:00,bob GET /home 200 512,Mozilla
2024-05-01 10:02:00,crawler GET /home 200 512,Googlebot
2024-05-01 10:03:00,garbage line,Mozilla
2024-05-01 11:30:00,bob GET /cart 404 64,Mozilla
"""

# no agent column; a referrer column
LOGS_B = """ts,line,referrer
2024-06-02 09:00:00,carol GET /home 200 400,https://search.example/q
2024-06-02 09:10:00,carol GET /cart 200 100,
2024-06-02 09:20:00,carol POST /checkout 200 50,
2024-06-02 12:00:00,carol GET /home 304 0,https://news.example/
"""

# an extra host column, pages outside the funnel
LOGS_C = """ts,host,line,agent
2024-07-01 08:00:00,web1,dave GET /about 200 300,curl/8.0
2024-07-01 08:10:00,web2,dave GET /about 200 300,curl/8.0
2024-07-01 08:11:00,web1,erin GET /home 200 100,Safari
2024-07-01 08:12:00,web1,BingBot GET /home 200 100,bingbot/2.0
"""


def load_logs(text):
    logs = pd.read_csv(io.StringIO(text))
    logs["ts"] = pd.to_datetime(logs["ts"])                                       #@ input ts
    if "agent" in logs.columns:
        logs = logs[~logs["agent"].str.contains("bot", case=False, na=False)]     #@ ok agent
    return logs  #@row logs: definite={ts} open


def parse_lines(logs):
    parts = logs["line"].str.extract(PATTERN)                                     #@ input line
    #@row parts: present={user,method,path,status,bytes} definite={user,method,path,status,bytes} closed
    n_fields = parts.shape[1]
    #@value n_fields: exactly={5}
    df = logs.drop(columns=["line"]).join(parts)
    #@row df: definite={ts,user,method,path,status,bytes} absent={line} open
    df = df.dropna(subset=["user"])
    df["status"] = df["status"].astype(int)                                       #@ ok status
    df["bytes"] = df["bytes"].astype(int)                                         #@ ok bytes
    df["is_error"] = df["status"] >= 400
    return df


def raw_line(df):
    return df["line"]                                                             #@ missing line


def sessionise(df, gap_minutes=30):
    df = df.sort_values(["user", "ts"]).reset_index(drop=True)
    df["gap"] = df.groupby("user")["ts"].diff()                                   #@ ok ts
    df["new_session"] = df["gap"].isna() | (df["gap"] > pd.Timedelta(minutes=gap_minutes))
    df["session_id"] = df["new_session"].cumsum()                                 #@ ok new_session
    df["prev_path"] = df.groupby("user")["path"].shift(1)                         #@ ok path
    df["step"] = df.groupby("session_id").cumcount() + 1
    # (first written absent={line,index}: the frame is open, the input may hold an `index` column)
    return df  #@row df: definite={ts,user,method,path,status,bytes,is_error,gap,new_session,session_id,prev_path,step} absent={line} open


def sessions_table(df):
    s = df.groupby("session_id").agg(user=("user", "first"), start=("ts", "min"), end=("ts", "max"), pages=("path", "count"), errors=("is_error", "sum"))
    #@row s: present={user,start,end,pages,errors} definite={user,start,end,pages,errors} absent={session_id,ts,path} closed
    s["duration_min"] = (s["end"] - s["start"]).dt.total_seconds() / 60           #@ ok end ; ok start
    s = s.reset_index()
    bounced = s[s["pages"] == 1]                                                  #@ ok pages
    #@row bounced: present={session_id,user,start,end,pages,errors,duration_min} definite={session_id,user,start,end,pages,errors,duration_min} closed
    return s


def user_stats(df, sessions):
    u = df.groupby("user").agg(hits=("path", "size"), bytes=("bytes", "sum"), last_seen=("ts", "max"))
    per_user = sessions.groupby("user")["session_id"].nunique().rename("sessions")
    u = u.join(per_user)
    #@row u: present={hits,bytes,last_seen,sessions} definite={hits,bytes,last_seen,sessions} absent={user,session_id} closed
    u = u.reset_index()
    u["bytes_per_hit"] = u["bytes"] / u["hits"]                                   #@ ok bytes ; ok hits
    u["user"]                                                                     #@ ok user
    return u  #@row u: present={user,hits,bytes,last_seen,sessions,bytes_per_hit} definite={user,hits,bytes,last_seen,sessions,bytes_per_hit} closed


def top_pages(df, n=3):
    top = df["path"].value_counts().head(n).reset_index()
    #@row top: present={path,count} definite={path,count} closed
    top = top.rename(columns={"count": "hits"})
    top["share"] = top["hits"] / len(df)                                          #@ ok hits
    labels = []
    for _, row in top.iterrows():
        labels.append(f"{row['path']} {row['share']:.0%}")                      #@ ok path ; ok share
        #@row row: present={path,hits,share} definite={path,hits,share} absent={count} closed
    return top


def old_count(top):
    return top["count"]                                                           #@ missing count


def funnel(df):
    counts = df.pivot_table(index="user", columns="path", values="ts", aggfunc="count", fill_value=0)
    #@row counts: open
    f = counts.reindex(columns=FUNNEL, fill_value=0)
    #@row f: present={"/home","/cart","/checkout"} definite={"/home","/cart","/checkout"} closed
    f["converted"] = (f["/cart"] > 0) & (f["/checkout"] > 0)                     #@ ok "/cart" ; ok "/checkout"
    width = len(f.columns)
    #@value width: exactly={4}
    return f["converted"].mean()


def about_hits(f):
    return f["/about"]                                                            #@ missing "/about"


def external_share(df):
    # (first written `possibly`: df comes from an open read_csv frame, so this depends on the input)
    return df["referrer"].notna().mean()                                          #@ input referrer


def landing_sources(df):
    if "referrer" not in df.columns:
        return pd.DataFrame(columns=["source", "visits"])
    src = df.dropna(subset=["referrer"])
    src = src.assign(source=src["referrer"].str.extract(r"https?://([^/]+)", expand=False))  #@ ok referrer
    return src.groupby("source").size().reset_index(name="visits")


def run(text):
    logs = load_logs(text)
    df = parse_lines(logs)
    try:
        raw_line(df)
    except KeyError:
        pass
    df = sessionise(df)
    sessions = sessions_table(df)
    u = user_stats(df, sessions)
    top = top_pages(df)
    try:
        old_count(top)
    except KeyError:
        pass
    funnel(df)
    try:
        about_hits(df.pivot_table(index="user", columns="path", values="ts", aggfunc="count").reindex(columns=FUNNEL))
    except KeyError:
        pass
    try:
        external_share(df)
    except KeyError:
        pass
    ls = landing_sources(df)
    #@row ls: present={source,visits} definite={source,visits} closed
    return u


if __name__ == "__main__":
    for text in (LOGS_A, LOGS_B, LOGS_C):
        run(text)

# (first written sometimes={referrer}: every read of referrer is behind an `in` test or a try)
#@schema read_csv@L37: always={ts,line} only={ts,line}
