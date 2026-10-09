# exclude_unset (fields not given at construction), exclude_defaults (fields equal to their default), exclude_none
from pydantic import BaseModel


class Prefs(BaseModel):
    user: str
    theme: str = "light"
    size: int = 12
    lang: str | None = None


def patch_from(d):
    p = Prefs(**d)
    s = p.model_dump(exclude_unset=True)
    #@row s: present={user,theme,size,lang} definite={user} closed
    n = p.model_dump(exclude_none=True)
    #@row n: present={user,theme,size,lang} definite={user,theme,size} closed
    return s


p = Prefs(user="u", size=12)
unset = p.model_dump(exclude_unset=True)
#@row unset: present={user,size} definite={user,size} absent={theme,lang} closed
defaults = p.model_dump(exclude_defaults=True)
#@row defaults: present={user} definite={user} absent={size,theme,lang} closed
nonone = p.model_dump(exclude_none=True)
#@row nonone: present={user,theme,size} definite={user,theme,size} absent={lang} closed
nset = len(p.model_fields_set)
#@value nset: exactly={2}
q = Prefs(user="v", theme="dark", lang="de")
qd = q.model_dump(exclude_defaults=True)
#@row qd: present={user,theme,lang} definite={user,theme,lang} absent={size} closed

if __name__ == "__main__":
    patch_from({"user": "a"})
    patch_from({"user": "b", "theme": "dark", "lang": None})
    patch_from({"user": "c", "size": 9, "lang": "en", "other": 1})

#@schema param patch_from.d: always={user} only={user}
