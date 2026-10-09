# Field aliases: the body uses the alias, attributes use the field name; a response_model serialises by alias by default
from fastapi import FastAPI
from fastapi.testclient import TestClient
from pydantic import BaseModel, ConfigDict, Field

app = FastAPI()


class Signup(BaseModel):
    user_name: str = Field(alias="userName")
    email_address: str = Field(alias="emailAddress")
    newsletter: bool = False


class Account(BaseModel):
    model_config = ConfigDict(populate_by_name=True)
    account_id: int = Field(alias="accountId")
    user_name: str = Field(alias="userName")


def to_record(s):
    _ = 0  #@row s: present={user_name,email_address,newsletter} definite={user_name,email_address,newsletter} closed
    rec = s.model_dump()
    #@row rec: present={user_name,email_address,newsletter} definite={user_name,email_address,newsletter} absent={userName} closed
    wire = s.model_dump(by_alias=True)
    #@row wire: present={userName,emailAddress,newsletter} definite={userName,emailAddress,newsletter} absent={user_name} closed
    return rec


@app.post("/signup", response_model=Account)
def signup(s: Signup):
    rec = to_record(s)
    acc = Account(account_id=len(rec["user_name"]), user_name=rec["user_name"])  #@ ok user_name
    return acc


@app.post("/signup-plain")
def signup_plain(s: Signup):
    return to_record(s)


if __name__ == "__main__":
    client = TestClient(app)
    for body in [{"userName": "ann", "emailAddress": "a@x"}, {"userName": "bobby", "emailAddress": "b@x", "newsletter": True}]:
        out = client.post("/signup", json=body).json()
        #@row out: present={accountId,userName} definite={accountId,userName} absent={account_id,user_name} closed
        plain = client.post("/signup-plain", json=body).json()
        #@row plain: present={user_name,email_address,newsletter} definite={user_name,email_address,newsletter} closed
    assert client.post("/signup", json={"user_name": "ann", "email_address": "a@x"}).status_code == 422
