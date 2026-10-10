"""Domain events: an Enum of kinds and a dataclass per event."""
from collections import Counter
from dataclasses import asdict, dataclass, field
from enum import Enum


class Kind(Enum):
    PLACED = "placed"
    REJECTED = "rejected"
    SHIPPED = "shipped"
    CANCELLED = "cancelled"


@dataclass
class Event:
    kind: Kind
    order_id: str
    data: dict = field(default_factory=dict)
    seq: int = 0


class EventLog:
    def __init__(self):
        self.events = []

    def emit(self, kind, order_id, **data):
        ev = Event(kind, order_id, dict(data), len(self.events) + 1)
        self.events.append(ev)
        return ev

    def counts(self):
        return dict(Counter(e.kind.value for e in self.events))

    def last(self):
        ev = self.events[-1]
        rec = asdict(ev)
        rec["kind"] = ev.kind.value
        return rec
