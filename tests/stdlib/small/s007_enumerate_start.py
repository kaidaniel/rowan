# enumerate(start=) numbering records; keys built from positions are known literals


def number(items, start):
    out = {}
    for i, it in enumerate(items, start=start):
        out[f"item{i}"] = it
    return out


def label_rows(rows):
    labelled = [{"pos": i, **r} for i, r in enumerate(rows, 1)]
    first = labelled[0]
    _ = 0                                        #@row first: present={pos,id,name} definite={pos,id} closed
    first["pos"]                                 #@ ok pos
    p = first["pos"]                             #@value p: exactly={1}
    return labelled


if __name__ == "__main__":
    a = number(["x", "y"], 1)                    #@row a: present={item1,item2} definite={item1,item2} closed
    b = number(["z"], 0)                         #@row b: present={item0} definite={item0} closed
    try:
        b["item1"]                               #@ missing item1
    except KeyError:
        pass
    label_rows([{"id": 1}, {"id": 2, "name": "n"}])
    label_rows([{"id": 3, "name": "m"}])
    idx = tuple([i for i, _ in enumerate("abc", start=10)])  #@value idx: exactly={(10,11,12)}
