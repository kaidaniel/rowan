"""Scenario 8: request validation on marshmallow, a typical application dependency (schemas
whose fields and hooks are classes and decorators; httpx and requests are modelled, so never
followed)."""

from marshmallow import Schema, ValidationError, fields, post_load, validate


class Address:
    def __init__(self, city: str, zip_code: str):
        self.city = city
        self.zip_code = zip_code


class AddressSchema(Schema):
    city = fields.Str(required=True)
    zip_code = fields.Str(required=True, validate=validate.Length(equal=5))

    @post_load
    def make(self, data: dict, **kwargs) -> Address:
        return Address(**data)


class UserSchema(Schema):
    name = fields.Str(required=True)
    age = fields.Int(validate=validate.Range(min=0))
    tags = fields.List(fields.Str(), load_default=list)
    address = fields.Nested(AddressSchema)


def parse_user(payload: dict) -> dict:
    try:
        user = UserSchema().load(payload)
    except ValidationError as e:
        return {"ok": False, "errors": e.messages}
    return {"ok": True, "name": user["name"], "city": user["address"].city}


def main() -> None:
    good = parse_user({"name": "Ada", "age": 36, "address": {"city": "London", "zip_code": "12345"}})
    bad = parse_user({"name": "Bob", "age": -1, "address": {"city": "Paris", "zip_code": "1"}})
    print(good, sorted(bad["errors"]))


if __name__ == "__main__":
    main()
