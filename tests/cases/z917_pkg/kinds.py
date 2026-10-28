"""Type aliases another module of the program imports (z917)."""
from typing import Literal, Optional, Union


class Shape:
    def __init__(self):
        self.sides = 4


Justify = Literal["left", "right", "center"]
Size = Optional[int]
Drawable = Union[Shape, str]
