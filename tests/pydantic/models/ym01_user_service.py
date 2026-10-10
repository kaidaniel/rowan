# user service: registration / profile update / listings over dict payloads; aliases, a legacy-key before validator, private state, dumps for storage and the API
import json
from typing import ClassVar, Literal

from pydantic import BaseModel, ConfigDict, Field, PrivateAttr, ValidationError, computed_field, field_validator, model_validator


class Address(BaseModel):
    street: str
    city: str
    postcode: str | None = None


class UserIn(BaseModel):
    model_config = ConfigDict(populate_by_name=True, extra="forbid")
    username: str = Field(min_length=3)
    email: str
    full_name: str | None = Field(None, alias="fullName")
    password: str
    address: Address | None = None

    @field_validator("email")
    @classmethod
    def norm_email(cls, v):
        if "@" not in v:
            raise ValueError("invalid email")
        return v.strip().lower()

    @model_validator(mode="before")
    @classmethod
    def legacy_login(cls, data):
        if isinstance(data, dict) and "login" in data:
            data = dict(data)
            data["username"] = data.pop("login")
            #@row data: present={username,email,password,fullName,address} definite={username,email,password,fullName,address} absent={login} closed
        return data


class User(BaseModel):
    ROLES: ClassVar[tuple] = ("member", "admin")
    id: int
    username: str
    email: str
    full_name: str | None = None
    role: Literal["member", "admin"] = "member"
    address: Address | None = None
    _password_hash: str = PrivateAttr(default="")

    @computed_field
    @property
    def display(self) -> str:
        return self.full_name or self.username


class UserPatch(BaseModel):
    email: str | None = None
    full_name: str | None = None
    address: Address | None = None


class Store:
    def __init__(self):
        self.rows = {}
        self.next_id = 1

    def save(self, user):
        row = user.model_dump(exclude={"display"})
        #@row row: present={id,username,email,full_name,role,address} definite={id,username,email,full_name,role,address} absent={display,_password_hash} closed
        self.rows[user.id] = row
        return row

    def load(self, uid):
        u = User.model_validate(self.rows[uid])
        #@row u: present={id,username,email,full_name,role,address} definite={id,username,email,full_name,role,address} closed
        return u


def hash_pw(pw):
    return "h:" + pw[::-1]


def register(store, payload):  # the schema had `login` too, but its read is guarded by `"login" in data` (w308)
    data = UserIn.model_validate(payload)        #@ possibly password ; possibly username
    #@row data: present={username,email,full_name,password,address} definite={username,email,full_name,password,address} absent={login,fullName} closed
    fields = data.model_dump(exclude={"password"})
    #@row fields: present={username,email,full_name,address} definite={username,email,full_name,address} absent={password} closed
    user = User(id=store.next_id, **fields)
    #@row user: present={id,username,email,full_name,role,address} definite={id,username,email,full_name,role,address} absent={password,display} closed
    user._password_hash = hash_pw(data.password)  #@ ok password
    store.next_id += 1
    store.save(user)
    return user


def public_view(user):
    out = user.model_dump(include={"id", "username", "display", "address"}, exclude_none=True)
    #@row out: present={id,username,display,address} definite={id,username,display} absent={email,role} closed
    return out


def api_response(user):
    body = json.loads(user.model_dump_json(exclude={"address": {"postcode"}}))
    #@row body: present={id,username,email,full_name,role,address,display} definite={id,username,email,full_name,role,address,display} closed
    if body["address"] is not None:              #@ ok address
        addr = body["address"]
        #@row addr: present={street,city} definite={street,city} absent={postcode} closed
    return body


def update_profile(store, uid, patch_payload):
    patch = UserPatch.model_validate(patch_payload)
    changes = patch.model_dump(exclude_unset=True)
    #@row changes: present={email,full_name,address} absent={username,role} closed
    user = store.load(uid)
    merged = {**user.model_dump(), **changes}
    #@row merged: present={id,username,email,full_name,role,address,display} definite={id,username,email,full_name,role,address,display} closed
    updated = User.model_validate(merged)
    #@row updated: present={id,username,email,full_name,role,address} definite={id,username,email,full_name,role,address} absent={display} closed
    store.save(updated)
    return updated


def promote(store, uid):
    user = store.load(uid)
    admin = user.model_copy(update={"role": "admin"})
    store.save(admin)
    return admin.role


def password_of(user):
    return user.password                         #@ missing password


def admin_listing(store):
    out = []
    for uid in sorted(store.rows):
        u = store.load(uid)
        entry = {"id": u.id, "name": u.display, "admin": u.role == "admin"}   #@ ok display ; ok role
        out.append(entry)
    _ = 0  #@row entry: present={id,name,admin} definite={id,name,admin} closed
    return out


def errors_for(payload):
    try:
        UserIn.model_validate(payload)
        return []
    except ValidationError as e:
        return [(".".join(str(p) for p in err["loc"]), err["type"]) for err in e.errors()]


GOOD = [
    {"username": "alice", "email": " Alice@Example.org ", "password": "pw1"},
    {"login": "bobby", "email": "bob@x.org", "password": "pw2", "fullName": "Bob B", "address": {"street": "s 1", "city": "Oslo"}},
    {"username": "carol", "email": "c@x.org", "password": "pw3", "full_name": "Carol", "address": {"street": "t 2", "city": "Rome", "postcode": "001"}},
]
BAD = [
    {"username": "dave", "email": "d@x.org"},
    {"email": "e@x.org", "password": "p"},
    {"username": "frank", "email": "f@x.org", "password": "p", "admin": True},
    {"username": "gary", "email": "g@x.org", "password": "p", "address": {"street": "u 3"}},
]

store = Store()
alice = register(store, GOOD[0])
em = alice.email
#@value em: exactly={"alice@example.org"}
disp = alice.display
#@value disp: exactly={"alice"}
role = alice.role
#@value role: exactly={"member"}
pv = public_view(alice)
#@row pv: present={id,username,display} definite={id,username,display} absent={address} closed
e1 = errors_for({"username": "ab", "email": "nope", "password": "p"})
first = e1[0][1]
#@value first: exactly={"string_too_short"}
second = e1[1][1]
#@value second: exactly={"value_error"}
extra_err = errors_for(BAD[2])[0][1]
#@value extra_err: exactly={"extra_forbidden"}

if __name__ == "__main__":
    for p in GOOD[1:]:
        u = register(store, p)
        public_view(u)
        api_response(u)
    api_response(alice)
    for p in BAD:
        try:
            register(store, p)
        except ValidationError:
            pass
    update_profile(store, 1, {"full_name": "Alice A"})
    update_profile(store, 2, {"email": "b2@x.org", "address": None})
    update_profile(store, 3, {})
    promoted = promote(store, 2)
    for uid in (1, 2):
        try:
            password_of(store.load(uid))
        except AttributeError:
            pass
    listing = admin_listing(store)

#@schema param register.payload: always={email,password} sometimes={username} only={email,password,username}
#@schema param register.payload['address']: always={street,city} only={street,city}
#@schema param update_profile.patch_payload: only={}
