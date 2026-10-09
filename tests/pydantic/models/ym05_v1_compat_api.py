# a v1-style API layer still running on v2: parse_obj / parse_raw / dict() / json() / copy(update=) / __fields__ / construct, @validator and @root_validator, request dicts handled in functions
import json
import warnings
from typing import Generic, List, Optional, TypeVar

from pydantic import BaseModel, Field, ValidationError, root_validator, validator

warnings.simplefilter("ignore", DeprecationWarning)

T = TypeVar("T")


class Tag(BaseModel):
    name: str
    color: str = "grey"


class Note(BaseModel):
    id: int
    title: str
    body: str = ""
    tags: List[Tag] = []
    owner: Optional[str] = None
    archived: bool = False

    @validator("title")
    def no_blank(cls, v):
        if not v.strip():
            raise ValueError("blank title")
        return v.strip()

    @validator("tags", pre=True)
    def split_tags(cls, v):
        if isinstance(v, str):
            return [{"name": t} for t in v.split(",") if t]
        return v

    @root_validator(pre=True)
    def legacy(cls, values):
        if "text" in values and "body" not in values:
            values = {**values, "body": values["text"]}
            #@row values: present={id,title,text,owner,body} definite={id,title,text,owner,body} closed
        return values

    @root_validator(skip_on_failure=True)
    def owner_tag(cls, values):
        #@row values: present={id,title,body,tags,owner,archived} definite={id,title,body,tags,owner,archived} closed
        if values["owner"] and not values["tags"]:   #@ ok owner ; ok tags
            values["tags"] = [Tag(name=values["owner"])]
        return values


class Envelope(BaseModel, Generic[T]):
    status: int = 200
    data: Optional[T] = None
    error: Optional[str] = None


class NoteStore:
    def __init__(self):
        self.notes = {}

    def put(self, note):
        self.notes[note.id] = note.dict()
        return self.notes[note.id]

    def get(self, nid):
        return Note.parse_obj(self.notes[nid])


def create(store, request):
    try:
        note = Note.parse_obj(request["body"])   #@ possibly title
    except ValidationError as e:
        return Envelope[Note](status=422, error=str(e.errors()[0]["loc"])).dict()
    _ = 0  #@row note: present={id,title,body,tags,owner,archived} definite={id,title,body,tags,owner,archived} absent={text} closed
    row = store.put(note)
    #@row row: present={id,title,body,tags,owner,archived} definite={id,title,body,tags,owner,archived} closed
    return Envelope[Note](status=201, data=note).dict()


def create_raw(store, text):
    note = Note.parse_raw(text)
    store.put(note)
    return note.json(exclude={"body"})


def patch(store, nid, changes):
    note = store.get(nid)
    allowed = {k: v for k, v in changes.items() if k in Note.__fields__ and k != "id"}
    #@row allowed: present={title,owner,archived} absent={id,color,body} closed
    updated = note.copy(update=allowed)
    #@row updated: present={id,title,body,tags,owner,archived} definite={id,title,body,tags,owner,archived} closed
    store.put(updated)
    return updated.dict(exclude_unset=True)


def archive(store, nid):
    note = store.get(nid)
    store.put(note.copy(update={"archived": True}))
    return store.notes[nid]["archived"]          #@ ok archived


def listing(store, include_body):
    out = []
    for nid in sorted(store.notes):
        n = store.get(nid)
        fields = {"id", "title", "tags"} | ({"body"} if include_body else set())
        d = n.dict(include=fields)
        #@row d: present={id,title,tags,body} definite={id,title,tags} absent={owner,archived} closed
        for t in d["tags"]:
            #@row t: present={name,color} definite={name,color} closed
            c = t["color"]                       #@ ok color
        out.append(d)
    return out


def unsafe(nid):
    n = Note.construct(id=nid)
    #@row n: present={id,body,tags,owner,archived} definite={id,body,tags,owner,archived} absent={title} closed
    return n.title                               #@ missing title


store = NoteStore()
r1 = create(store, {"body": {"id": 1, "title": " Groceries ", "tags": "home,food"}})
st = r1["status"]
#@value st: exactly={201}
r1d = r1["data"]
#@row r1d: present={id,title,body,tags,owner,archived} definite={id,title,body,tags,owner,archived} closed
t1 = r1d["title"]
#@value t1: exactly={"Groceries"}
n_tags = len(r1d["tags"])
#@value n_tags: exactly={2}
r2 = create(store, {"body": {"id": 2, "title": "Call", "text": "ring bo", "owner": "ann"}})
b2 = store.notes[2]["body"]
#@value b2: exactly={"ring bo"}
owner_tag = store.notes[2]["tags"][0]["name"]
#@value owner_tag: exactly={"ann"}
r3 = create(store, {"body": {"id": 3, "title": "  "}})
st3 = r3["status"]
#@value st3: exactly={422}
#@row r3: present={status,data,error} definite={status,data,error} closed
raw_json = json.loads(create_raw(store, '{"id": 4, "title": "raw", "body": "x"}'))
#@row raw_json: present={id,title,tags,owner,archived} definite={id,title,tags,owner,archived} absent={body} closed
fields = Note.__fields__
#@row fields: present={id,title,body,tags,owner,archived} definite={id,title,body,tags,owner,archived} closed

if __name__ == "__main__":
    patch(store, 1, {"title": "Shopping", "color": "red"})
    patch(store, 2, {"id": 9, "archived": True, "owner": None})
    archive(store, 4)
    listing(store, True)
    listing(store, False)
    try:
        create(store, {"headers": {}})
    except KeyError:
        pass
    try:
        create(store, {"body": {"id": 5}})
    except ValidationError:
        pass
    try:
        unsafe(7)
    except AttributeError:
        pass

#@schema param create.request: always={body} only={body}
#@schema param create.request['body']: always={id,title} only={id,title}
