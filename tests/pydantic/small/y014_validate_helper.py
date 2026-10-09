# one validating helper called from several entry points that build or forward the payload
from pydantic import BaseModel, ValidationError


class Signup(BaseModel):
    email: str
    password: str
    referrer: str | None = None


def validate_signup(data):
    s = Signup.model_validate(data)              #@ possibly password
    #@row s: present={email,password,referrer} definite={email,password,referrer} closed
    return s


def from_web(form):
    data = {"email": form["email"], "password": form["pw"]}   #@ possibly pw
    if "ref" in form:
        data["referrer"] = form["ref"]           #@ ok ref
    #@row data: present={email,password,referrer} definite={email,password} closed
    return validate_signup(data)


def from_api(body):
    s = validate_signup(body)
    out = s.model_dump(exclude={"password"})
    #@row out: present={email,referrer} definite={email,referrer} absent={password} closed
    return out


if __name__ == "__main__":
    from_web({"email": "a@x", "pw": "1"})
    from_web({"email": "b@x", "pw": "2", "ref": "friend", "csrf": "t"})
    try:
        from_web({"email": "c@x", "password": "3"})
    except KeyError:
        pass
    from_api({"email": "d@x", "password": "4"})
    try:
        from_api({"email": "e@x"})
    except ValidationError:
        pass

#@schema param from_web.form: always={email,pw} only={email,pw}
#@schema param from_api.body: always={email,password} only={email,password}
#@schema param validate_signup.data: always={email,password} only={email,password}
