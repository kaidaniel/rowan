# ConfigDict(populate_by_name=True): an aliased field accepts its alias or its name as input key
from pydantic import BaseModel, ConfigDict, Field, ValidationError


class Account(BaseModel):
    model_config = ConfigDict(populate_by_name=True)
    account_id: int = Field(alias="accountId")
    owner: str = Field(alias="ownerName")
    plan: str = "free"


def load(d):
    a = Account.model_validate(d)                #@ possibly accountId
    #@row a: present={account_id,owner,plan} definite={account_id,owner,plan} absent={accountId,ownerName} closed
    out = a.model_dump(by_alias=True)
    #@row out: present={accountId,ownerName,plan} definite={accountId,ownerName,plan} closed
    return out


x = Account(account_id=1, owner="ann")
#@row x: present={account_id,owner,plan} definite={account_id,owner,plan} closed
y = Account(accountId=2, ownerName="bo", plan="pro")
pl = y.plan
#@value pl: exactly={"pro"}
both = x.model_dump()
#@row both: present={account_id,owner,plan} definite={account_id,owner,plan} closed

if __name__ == "__main__":
    load({"accountId": 1, "ownerName": "a"})
    load({"account_id": 2, "owner": "b", "plan": "team"})
    load({"accountId": 3, "owner": "c", "x": 0})
    try:
        load({"owner": "d"})
    except ValidationError:
        pass

# each required field may come under its alias or its name
#@schema param load.d: sometimes={accountId,account_id,ownerName,owner} only={accountId,account_id,ownerName,owner}
