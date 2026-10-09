# merging dumps with dicts: {**defaults, **m.model_dump(exclude_none=True)}, dump | patch revalidated, a diff over two dumps
from pydantic import BaseModel

DEFAULTS = {"theme": "light", "size": 12, "beta": False}


class Prefs(BaseModel):
    theme: str | None = None
    size: int | None = None
    lang: str = "en"


def effective(p):
    merged = {**DEFAULTS, **p.model_dump(exclude_none=True)}
    #@row merged: present={theme,size,beta,lang} definite={theme,size,beta,lang} closed
    return merged


def apply_patch(p, patch):
    data = p.model_dump() | patch
    #@row data: present={theme,size,lang,beta} definite={theme,size,lang} closed
    q = Prefs.model_validate(data)
    #@row q: present={theme,size,lang} definite={theme,size,lang} absent={beta} closed
    return q


def diff(a, b):
    old = a.model_dump()
    changed = {k: v for k, v in b.model_dump().items() if old[k] != v}
    #@row changed: present={theme,size,lang} closed
    return changed


e = effective(Prefs(size=14))
sz = e["size"]
#@value sz: exactly={14}
th = e["theme"]
#@value th: exactly={"light"}
d = diff(Prefs(), Prefs(lang="de"))
#@row d: present={lang} definite={lang} closed

if __name__ == "__main__":
    for p in (Prefs(), Prefs(theme="dark", lang="fr")):
        effective(p)
        apply_patch(p, {})
        apply_patch(p, {"size": 10, "beta": True})
        diff(p, Prefs(size=1))
