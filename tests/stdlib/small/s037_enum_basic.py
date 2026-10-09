# Enum members, .name / .value, iteration, Enum['NAME'] (KeyError) and Enum(value) (ValueError)
from enum import Enum


class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3


def code_of(name):
    return Color[name.upper()].value


def name_of(v):
    try:
        return Color(v).name
    except ValueError:
        return "unknown"


if __name__ == "__main__":
    a = code_of("red")                           #@value a: exactly={1}
    b = code_of("Blue")                          #@value b: exactly={3}
    n1 = name_of(2)                              #@value n1: exactly={GREEN}
    n2 = name_of(9)                              #@value n2: exactly={unknown}
    names = tuple(c.name for c in Color)         #@value names: exactly={("RED","GREEN","BLUE")}
    try:
        Color["PURPLE"]                          #@ missing PURPLE
    except KeyError:
        pass
    try:
        Color.PINK                               #@ missing PINK
    except AttributeError:
        pass
    mem = Color.__members__                      #@row mem: present={RED,GREEN,BLUE} definite={RED,GREEN,BLUE} closed
    same = Color(3) is Color.BLUE                #@value same: exactly={True}
    by_name = {c.name: c.value for c in Color}   #@row by_name: present={RED,GREEN,BLUE} definite={RED,GREEN,BLUE} closed
