def tree(depth):
    if depth == 0:
        return {"leaf": True}
    return {"l": tree(depth - 1), "r": tree(depth - 1)}


t = tree(3)                          #@row t: present={l,r} definite={l,r} closed
t["l"]["r"]["leaf"]                  #@ ok l ; ok r ; missing leaf
t["x"]                               #@ missing x
s = tree(n)                          #@row s: present={leaf,l,r} definite={} closed
s["leaf"]                            #@ possibly leaf
s["l"]["x"]                          #@ missing x
