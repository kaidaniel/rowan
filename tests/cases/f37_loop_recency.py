#@engine rust
"""Recency abstraction (Rust engine): the object allocated by the current loop iteration is
strong — attributes set on it after construction are proven on it, and keys every iteration
sets are proven on the per-site summary object the collection holds afterwards."""


class Node:
    def __init__(self, v):
        self.v = v


nodes = []
for i in range(n):
    nd = Node(i)
    nd.extra = i * 2
    nd.extra                               #@ ok extra
    if i % 2:
        nd.flag = True
    nd.flag                                #@ possibly flag
    rec = {"id": i}
    rec["seen"] = True
    rec["seen"]                            #@ ok seen
    nodes.append((nd, rec))

for nd, rec in nodes:
    nd.v                                   #@ ok v
    nd.extra                               #@ ok extra
    nd.flag                                #@ possibly flag
    rec["seen"]                            #@ ok seen
    rec["nope"]                            #@ missing nope
