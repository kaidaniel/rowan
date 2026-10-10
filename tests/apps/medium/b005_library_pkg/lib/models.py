"""Models: pydantic for what comes in (a book with a normalising isbn validator, a loan
request with bounded days), a dataclass for the loans the repository keeps."""
from dataclasses import dataclass, field
from datetime import date
from typing import Literal, Optional

from pydantic import BaseModel, Field, field_validator


class BookIn(BaseModel):
    isbn: str = Field(min_length=4)
    title: str
    genre: Literal["fiction", "science", "history"] = "fiction"
    copies: int = Field(default=1, ge=0)

    @field_validator("isbn")
    @classmethod
    def digits(cls, v):
        v = v.replace("-", "")
        if not v.isdigit():
            raise ValueError("isbn must be digits")
        return v


class LoanIn(BaseModel):
    member: str
    isbn: str
    days: int = Field(default=14, ge=1, le=28)
    note: Optional[str] = None


@dataclass
class Loan:
    id: int
    member: str
    isbn: str
    start: date
    due: date
    returned: Optional[date] = None
    flags: list = field(default_factory=list)
