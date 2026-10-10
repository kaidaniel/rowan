"""Request and stored models; the public form is a JSON-mode dump without the None fields."""
from datetime import date
from enum import Enum, IntEnum
from typing import Optional

from pydantic import BaseModel, field_validator


class Priority(IntEnum):
    LOW = 1
    NORMAL = 2
    HIGH = 3


class Status(str, Enum):
    OPEN = "open"
    DONE = "done"


class TaskIn(BaseModel):
    title: str
    priority: Priority = Priority.NORMAL
    due: Optional[date] = None
    labels: list[str] = []

    @field_validator("title")
    @classmethod
    def strip_title(cls, v):
        v = v.strip()
        if not v:
            raise ValueError("empty title")
        return v


class Task(TaskIn):
    id: int
    status: Status = Status.OPEN
    closed_by: Optional[str] = None

    def public(self):
        d = self.model_dump(mode="json", exclude_none=True)
        #@row d: present={title,priority,due,labels,id,status,closed_by} definite={title,priority,labels,id,status} closed
        return d
