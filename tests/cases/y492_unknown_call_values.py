"""Code rowan cannot see, given a record, may overwrite its values, and change whatever it reaches
through it, at any depth. Unknown calls kept every slot's value (`v` read as exactly 1) and
changed the rows of records one level down only, at most 24 of them (`deep` stayed closed).
`operator.setitem` was taken as having no effect."""
import operator

d = {"k": 1, "box": [{"deep": 1}]}
operator.setitem(d, "k", 5)
v = d["k"]                                #@value v: unknown
#@todo value v: exactly={5}
inner = d["box"][0]                       #@row inner: open
