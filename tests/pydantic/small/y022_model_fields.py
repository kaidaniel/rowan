# Model.model_fields (FieldInfo per field, inherited too), is_required / default / alias, and model_fields_set per instance
from pydantic import BaseModel, Field


class Base(BaseModel):
    id: int


class Doc(Base):
    title: str = Field(alias="Title")
    pages: int = 1
    owner: str | None = None


fields = Doc.model_fields
#@row fields: present={id,title,pages,owner} definite={id,title,pages,owner} absent={Title} closed
req = sorted(name for name, f in fields.items() if f.is_required())
nreq = len(req)
#@value nreq: exactly={2}
pages_default = fields["pages"].default         #@ ok pages
#@value pages_default: exactly={1}
al = fields["title"].alias                      #@ ok title
#@value al: exactly={"Title"}
base_fields = Base.model_fields
#@row base_fields: present={id} definite={id} closed


def describe(cls):
    info = {name: f.is_required() for name, f in cls.model_fields.items()}
    #@row info: present={id,title,pages,owner} definite={id} closed
    return info


def touched(d):
    doc = Doc.model_validate(d)
    given = doc.model_fields_set
    has_pages = "pages" in given
    return has_pages


t = touched({"id": 1, "Title": "x", "pages": 3})
#@value t: exactly={True}
f = touched({"id": 2, "Title": "y"})
#@value f: exactly={False}

if __name__ == "__main__":
    describe(Base)
    describe(Doc)
