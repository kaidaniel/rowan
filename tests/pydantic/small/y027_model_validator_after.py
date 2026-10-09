# @model_validator(mode="after") gets the validated instance: it reads fields (always there) and may fill optional ones
from pydantic import BaseModel, ValidationError, model_validator


class Window(BaseModel):
    start: int
    end: int
    label: str | None = None

    @model_validator(mode="after")
    def check(self):
        if self.start > self.end:                #@ ok start ; ok end
            raise ValueError("start after end")
        if self.label is None:
            self.label = f"{self.start}-{self.end}"
        return self


def window(d):
    w = Window(**d)                              #@ possibly end
    #@row w: present={start,end,label} definite={start,end,label} closed
    return w.label


w = Window(start=1, end=3)
lb = w.label
#@value lb: exactly={"1-3"}
nfs = len(w.model_fields_set)
#@value nfs: exactly={3}
w2 = Window(start=0, end=0, label="z")
lb2 = w2.label
#@value lb2: exactly={"z"}

if __name__ == "__main__":
    window({"start": 1, "end": 2})
    window({"start": 1, "end": 2, "label": "x"})
    for bad in ({"start": 5, "end": 1}, {"start": 1}):
        try:
            window(bad)
        except ValidationError:
            pass

#@schema param window.d: always={start,end} only={start,end}
