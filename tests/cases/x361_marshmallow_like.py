"""Weakness: unmodelled libraries. A schema object's `load` from an unknown library gives an
open result.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
from marshmallow import Schema, fields


class UserSchema(Schema):
    name = fields.Str()


data = UserSchema().load({"name": "a"})
data["name"]                                 #@ sound name
