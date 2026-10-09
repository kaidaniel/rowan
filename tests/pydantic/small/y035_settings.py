# pydantic_settings.BaseSettings: fields from prefixed environment variables (set by the program), init kwargs win, nested via a delimiter
import os

from pydantic import BaseModel, ValidationError
from pydantic_settings import BaseSettings, SettingsConfigDict


class Db(BaseModel):
    url: str = "sqlite://"
    pool: int = 5


class AppSettings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="ROWANY_", env_nested_delimiter="__")
    host: str = "localhost"
    port: int = 8000
    debug: bool = False
    api_key: str
    db: Db = Db()


def load(**overrides):
    s = AppSettings(**overrides)                 #@ possibly api_key
    #@row s: present={host,port,debug,api_key,db} definite={host,port,debug,api_key,db} closed
    db = s.db
    #@row db: present={url,pool} definite={url,pool} closed
    d = s.model_dump()
    #@row d: present={host,port,debug,api_key,db} definite={host,port,debug,api_key,db} closed
    return s


for k in [k for k in os.environ if k.upper().startswith("ROWANY_")]:
    del os.environ[k]
os.environ["ROWANY_API_KEY"] = "k1"
os.environ["ROWANY_PORT"] = "9000"
os.environ["ROWANY_DB__POOL"] = "7"
s = AppSettings()
port = s.port
#@value port: exactly={9000}
key = s.api_key
#@value key: exactly={"k1"}
pool = s.db.pool
#@value pool: exactly={7}
s2 = AppSettings(port=1)
p2 = s2.port
#@value p2: exactly={1}

if __name__ == "__main__":
    load()
    load(debug=True, host="h")
    del os.environ["ROWANY_API_KEY"]
    load(api_key="k2")
    try:
        load()
    except ValidationError:
        pass
