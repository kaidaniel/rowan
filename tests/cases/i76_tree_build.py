"""A tree of dict nodes built from a flat literal list of (parent, name) pairs; children are
looked up by name in an index and linked; a path lookup walks the tree.

Expectations were written from the program text before running the engine."""

ROWS = [(None, "root"), ("root", "etc"), ("root", "usr"), ("usr", "bin"), ("etc", "hosts")]


def build(rows):
    index = {}
    root = None
    for parent, name in rows:
        node = {"name": name, "children": {}}
        index[name] = node
        if parent is None:
            root = node
            node["is_root"] = True
        else:
            index[parent]["children"][name] = node
            node["parent"] = parent
    return root, index


def path_of(index, name):
    out = [name]
    node = index[name]
    while "parent" in node:
        out.append(node["parent"])
        node = index[node["parent"]]
    return out


root, index = build(ROWS)
_ = 0                                     #@row index: present={root,etc,usr,bin,hosts} definite={root,etc,usr,bin,hosts} closed
_ = 0                                     #@row root: present={name,children,is_root} definite={name,children,is_root} closed
kids = root["children"]                   #@row kids: present={etc,usr} definite={etc,usr} closed
usr = index["usr"]                        #@row usr: present={name,children,parent} definite={name,children,parent} closed
usr_kids = usr["children"]                #@row usr_kids: present={bin} definite={bin} closed
hosts = index["hosts"]                    #@row hosts: present={name,children,parent} definite={name,children,parent} closed
empty = hosts["children"]                 #@row empty: present={} definite={} closed
root["parent"]                            #@ missing parent
usr["is_root"]                            #@ missing is_root
p = path_of(index, "bin")
# A node reached through the while loop may be any node: "parent" is a loop condition there.
