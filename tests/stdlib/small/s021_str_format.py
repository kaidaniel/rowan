# str.format(**rec) / "{r[k]}".format(r=rec) / format_map read keys of the record (KeyError when absent)

TEMPLATE = "{name} <{email}>"


class Lenient(dict):
    def __missing__(self, key):
        return "?"


def render(rec):
    return TEMPLATE.format(**rec)                #@ ok email ; ok name ; missing name
#@schema param render.rec: always={name,email} only={name,email}


def tagline(rec):
    return "{r[id]}-{r[kind]}".format(r=rec)     #@ ok id ; ok kind ; missing kind


def soft(rec):  # author was `none`, but a present author is a proven read (a missing one goes to __missing__)
    return "{title} by {author}".format_map(Lenient(rec))  #@ sound author


if __name__ == "__main__":
    s = render({"name": "a", "email": "e"})      #@value s: exactly={"a <e>"}
    render({"name": "b", "email": "f", "extra": 1})
    try:
        render({"email": "g"})
    except KeyError:
        pass
    t = tagline({"id": 1, "kind": "x"})          #@value t: exactly={"1-x"}
    try:
        tagline({"id": 2})
    except KeyError:
        pass
    a = soft({"title": "T"})                     #@value a: exactly={"T by ?"}
    b = soft({"title": "U", "author": "V"})      #@value b: exactly={"U by V"}
    p = "{0}/{1}".format("x", 2)                 #@value p: exactly={"x/2"}
