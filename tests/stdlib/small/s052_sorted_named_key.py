# a named key function given to sorted / list.sort: its parameter is each element; .get reads are no requirements


def rank_key(rec):
    return (-rec["points"], rec.get("name", ""))
#@schema param rank_key.rec: always={points} only={points}


def standings(teams):
    table = sorted(teams, key=rank_key)
    leader = table[0]
    #@row leader: present={points,city} definite={points} absent={name} closed
    leader["points"]                              #@ ok points
    names = [t.get("name", "?") for t in table]
    return names


def sort_in_place(entries):
    entries.sort(key=rank_key, reverse=True)
    first = entries[0]
    #@row first: present={points,name,city} definite={points,name} closed
    return first["name"]                          #@ ok name


if __name__ == "__main__":
    standings([{"points": 3, "name": "ann"}, {"points": 5, "city": "rome"}])
    standings([{"points": 1}])
    r = sort_in_place([{"points": 1, "name": "x"}, {"points": 2, "name": "y"}])   #@value r: exactly={"x"}
    r2 = sort_in_place([{"points": 4, "name": "z", "city": "oslo"}])             #@value r2: exactly={"z"}
