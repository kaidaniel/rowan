# models handed between functions: getattr / hasattr with literal names on instances of several classes, vars(), dict(m) and iteration over a model
from pydantic import BaseModel, computed_field


class Book(BaseModel):
    title: str
    isbn: str


class Film(BaseModel):
    title: str
    minutes: int = 90

    @computed_field
    @property
    def hours(self) -> float:
        return self.minutes / 60


def describe(work):
    t = work.title                               #@ ok title
    has_isbn = hasattr(work, "isbn")
    length = getattr(work, "minutes", None)
    attrs = vars(work)
    #@row attrs: present={title,isbn,minutes} definite={title} closed
    as_dict = dict(work)
    #@row as_dict: present={title,isbn,minutes} definite={title} absent={hours} closed
    names = [name for name, _ in work]
    return t, has_isbn, length


def isbn_of(work):
    return work.isbn                             #@ possibly isbn


b = describe(Book(title="b", isbn="1"))[1]
#@value b: exactly={True}
f = describe(Film(title="f"))
fl = f[2]
#@value fl: exactly={90}
fb = f[1]
#@value fb: exactly={False}
hrs = hasattr(Film(title="g"), "hours")
#@value hrs: exactly={True}

if __name__ == "__main__":
    for w in (Book(title="x", isbn="2"), Film(title="y", minutes=120)):
        describe(w)
        try:
            isbn_of(w)
        except AttributeError:
            pass
