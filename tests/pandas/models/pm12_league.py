# Sports league table: match results -> one row per team and match by concatenating a home
# view and an away view (projection + rename built from f-strings over the side), per-team
# sums with named aggregations, points and goal difference, sort_values on several keys,
# rank, home/away split via unstack + reindex + add_suffix, form strings from a groupby
# apply, a comparison with last season (merge suffixes); the two seasons differ in optional
# columns (`attendance`, `round`) and in teams.
import io

import pandas as pd

SEASON_A = """date,home,away,home_goals,away_goals,attendance
2024-08-10,Ajax,Bree,2,1,30000
2024-08-11,Cove,Dale,0,0,12000
2024-08-17,Bree,Cove,1,3,15000
2024-08-18,Dale,Ajax,2,2,9000
2024-08-24,Ajax,Cove,1,0,31000
2024-08-25,Bree,Dale,0,1,14000
2024-08-31,Cove,Ajax,,,
"""

SEASON_B = """date,round,home,away,home_goals,away_goals,referee
2025-08-09,1,Erin,Fife,3,0,Moss
2025-08-09,1,Gael,Ajax,1,1,Oliver
2025-08-16,2,Fife,Gael,2,2,Moss
2025-08-16,2,Ajax,Erin,0,2,Taylor
2025-08-23,3,Erin,Gael,1,0,Oliver
2025-08-23,3,Fife,Ajax,1,4,Taylor
"""

LAST_SEASON = pd.DataFrame({"team": ["Ajax", "Bree", "Cove", "Erin"], "points": [10, 4, 7, 9], "gd": [5, -3, 1, 2]})
#@row LAST_SEASON: present={team,points,gd} definite={team,points,gd} closed


def load_matches(text):
    m = pd.read_csv(io.StringIO(text))
    m["date"] = pd.to_datetime(m["date"])                       #@ input date
    for col in ("home", "away"):
        m[col] = m[col].str.strip()                             #@ input home ; input away
    m = m[m["home_goals"].notna() & m["away_goals"].notna()]    #@ input home_goals ; input away_goals
    m["home_goals"] = m["home_goals"].astype(int)               #@ ok home_goals
    m["away_goals"] = m["away_goals"].astype(int)               #@ ok away_goals
    return m                                                    #@row m: present={date,home,away,home_goals,away_goals} definite={date,home,away,home_goals,away_goals} open


def team_view(m, side):
    other = "away" if side == "home" else "home"
    view = m[["date", side, other, f"{side}_goals", f"{other}_goals"]]
    view = view.rename(columns={side: "team", other: "opponent", f"{side}_goals": "gf", f"{other}_goals": "ga"})
    view["venue"] = side
    #@row view: present={date,team,opponent,gf,ga,venue} definite={date,team,opponent,gf,ga,venue} absent={home,away,home_goals,away_goals} closed
    return view


def long_results(m):
    both = pd.concat([team_view(m, "home"), team_view(m, "away")], ignore_index=True)
    both["win"] = (both["gf"] > both["ga"]).astype(int)        #@ ok gf ; ok ga
    both["draw"] = (both["gf"] == both["ga"]).astype(int)
    both["loss"] = (both["gf"] < both["ga"]).astype(int)
    both["points"] = 3 * both["win"] + both["draw"]
    #@row both: present={date,team,opponent,gf,ga,venue,win,draw,loss,points} definite={date,team,opponent,gf,ga,venue,win,draw,loss,points} closed
    return both


def league_table(both):
    table = both.groupby("team").agg(played=("points", "size"), won=("win", "sum"), drawn=("draw", "sum"), lost=("loss", "sum"), gf=("gf", "sum"), ga=("ga", "sum"), points=("points", "sum"))
    #@row table: present={played,won,drawn,lost,gf,ga,points} definite={played,won,drawn,lost,gf,ga,points} absent={team,win,venue} closed
    table["gd"] = table["gf"] - table["ga"]                     #@ ok gf ; ok ga
    table = table.reset_index()                                 #@row table: present={team,played,won,drawn,lost,gf,ga,points,gd} definite={team,played,won,drawn,lost,gf,ga,points,gd} closed
    table = table.sort_values(["points", "gd", "gf", "team"], ascending=[False, False, False, True])
    table["position"] = range(1, len(table) + 1)
    table["pts_rank"] = table["points"].rank(method="min", ascending=False).astype(int)   #@ ok points
    return table.reset_index(drop=True)


def goal_diff_of(table):
    return table["goal_diff"]                                   #@ missing goal_diff


def result_letter(row):
    if row["win"]:                                              #@ ok win
        return "W"
    return "D" if row["draw"] else "L"                          #@ ok draw


def recent_form(both, n):
    both = both.sort_values(["team", "date"])                   #@row both: present={date,team,opponent,gf,ga,venue,win,draw,loss,points} definite={date,team,opponent,gf,ga,venue,win,draw,loss,points} closed
    both["result"] = both.apply(result_letter, axis=1)
    form = both.groupby("team")["result"].apply(lambda s: "".join(s.tail(n))).reset_index(name="form")
    #@row form: present={team,form} definite={team,form} closed
    form["result"]                                              #@ missing result
    return form


def recent_form_safe(both, n):
    try:
        return recent_form(both, n)
    except KeyError:
        both = both.assign(result=both.apply(result_letter, axis=1))
        return both.groupby("team", as_index=False).agg(form=("result", "sum"))


def home_away_split(both):
    split = both.groupby(["team", "venue"])["points"].sum().unstack(fill_value=0)
    split = split.reindex(columns=["home", "away"], fill_value=0)
    #@row split: present={home,away} definite={home,away} closed
    split = split.add_suffix("_pts")                            #@row split: present={home_pts,away_pts} definite={home_pts,away_pts} absent={home,away} closed
    split = split.reset_index()                                 #@row split: present={team,home_pts,away_pts} definite={team,home_pts,away_pts} closed
    split["home_share"] = split["home_pts"] / (split["home_pts"] + split["away_pts"]).where(lambda s: s > 0)
    return split


def crowds(m, table):
    if "attendance" in m.columns:
        att = m.groupby("home", as_index=False)["attendance"].mean()   #@ ok attendance
        #@row att: present={home,attendance} definite={home,attendance} closed
        att = att.rename(columns={"home": "team"})
    else:
        att = table[["team"]].assign(attendance=float("nan"))
    return att                                                  #@row att: present={team,attendance} definite={team,attendance} absent={home} closed


def versus_last(table):
    cmp = table[["team", "points", "gd"]].merge(LAST_SEASON, on="team", how="left", suffixes=("_now", "_prev"))
    #@row cmp: present={team,points_now,points_prev,gd_now,gd_prev} definite={team,points_now,points_prev,gd_now,gd_prev} absent={points,gd} closed
    cmp["delta"] = cmp["points_now"] - cmp["points_prev"]       #@ ok points_now ; ok points_prev
    return cmp


def last_points(cmp):
    return cmp["points"]                                        #@ missing points


def standings(text):
    m = load_matches(text)
    both = long_results(m)
    table = league_table(both)
    try:
        goal_diff_of(table)
    except KeyError:
        pass
    form = recent_form_safe(both, 3)                            #@row form: present={team,form} definite={team,form} closed
    split = home_away_split(both)
    att = crowds(m, table)
    final = table.merge(form, on="team").merge(split, on="team").merge(att, on="team", how="left")
    #@row final: present={team,played,won,drawn,lost,gf,ga,points,gd,position,pts_rank,form,home_pts,away_pts,home_share,attendance} definite={team,played,won,drawn,lost,gf,ga,points,gd,position,pts_rank,form,home_pts,away_pts,home_share,attendance} closed
    cmp = versus_last(table)
    try:
        last_points(cmp)
    except KeyError:
        pass
    if "round" in m.columns:
        m = m.assign(matchday=m["round"])                       #@ ok round
    rounds = m["matchday"].nunique() if "matchday" in m.columns else 0   #@ ok matchday
    leader = final.iloc[0].to_dict()                            #@row leader: present={team,played,won,drawn,lost,gf,ga,points,gd,position,pts_rank,form,home_pts,away_pts,home_share,attendance} definite={team,played,won,drawn,lost,gf,ga,points,gd,position,pts_rank,form,home_pts,away_pts,home_share,attendance} closed
    return final, leader, rounds


def matchdays(text):
    m = load_matches(text)
    if "round" in m.columns:
        m["matchday"] = m["round"]                              #@ ok round
    # (first written `possibly`: m is an open input, which may hold `matchday` itself)
    return m["matchday"].max()                                  #@ input matchday


if __name__ == "__main__":
    tables = {}
    for name, text in [("A", SEASON_A), ("B", SEASON_B)]:
        final, leader, rounds = standings(text)
        tables[name] = final
        top = leader["team"]                                    #@ ok team
        try:
            matchdays(text)
        except KeyError:
            pass
    both_seasons = pd.concat(tables, names=["season", "row"]).reset_index()
    #@row both_seasons: present={season,row,team,played,won,drawn,lost,gf,ga,points,gd,position,pts_rank,form,home_pts,away_pts,home_share,attendance} definite={season,row,team,played,won,drawn,lost,gf,ga,points,gd,position,pts_rank,form,home_pts,away_pts,home_share,attendance} closed
    best = both_seasons.sort_values(["points", "gd"], ascending=False).head(1)
    best["season"]                                              #@ ok season

# (first written sometimes={round,attendance}: their reads are behind `in` tests; matchday is
# needed where round is absent)
#@schema read_csv@L35: always={home,away,home_goals,away_goals,date} sometimes={matchday} only={home,away,home_goals,away_goals,date,matchday}
