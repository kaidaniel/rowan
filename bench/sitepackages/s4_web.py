"""Scenario 4: a web app on flask with pydantic request models (pydantic is modelled, so
never followed; flask, werkzeug, jinja2 and their dependencies are)."""

from flask import Flask, abort, jsonify, request
from pydantic import BaseModel, Field, ValidationError

app = Flask(__name__)

ITEMS: dict[int, dict] = {
    1: {"id": 1, "name": "Widget", "price": 4.5, "tags": ["small"]},
    2: {"id": 2, "name": "Gadget", "price": 24.99, "tags": []},
}


class ItemIn(BaseModel):
    name: str = Field(min_length=1)
    price: float = Field(gt=0)
    tags: list[str] = []


def item_out(item: dict) -> dict:
    return {"id": item["id"], "name": item["name"], "price": round(item["price"], 2), "tag_count": len(item["tags"])}


@app.get("/items")
def list_items():
    limit = request.args.get("limit", default=10, type=int)
    return jsonify([item_out(i) for i in list(ITEMS.values())[:limit]])


@app.get("/items/<int:item_id>")
def get_item(item_id: int):
    item = ITEMS.get(item_id)
    if item is None:
        abort(404)
    return jsonify(item_out(item))


@app.post("/items")
def create_item():
    try:
        body = ItemIn.model_validate(request.get_json(force=True))
    except ValidationError as exc:
        return jsonify({"errors": exc.errors()}), 422
    new_id = max(ITEMS) + 1
    ITEMS[new_id] = {"id": new_id, **body.model_dump()}
    return jsonify(item_out(ITEMS[new_id])), 201


if __name__ == "__main__":
    client = app.test_client()
    resp = client.get("/items/1")
    data = resp.get_json()
    print(resp.status_code, data["name"])
    created = client.post("/items", json={"name": "Gizmo", "price": 3.0})
    print(created.status_code, created.get_json())
