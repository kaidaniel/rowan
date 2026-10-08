"""Command objects that execute and undo against a document dict; a history stack of commands;
undo pops and reverts.

Expectations were written from the program text before running the engine."""


class SetField:
    def __init__(self, key, value):
        self.key = key
        self.value = value

    def execute(self, doc):
        self.old = doc.get(self.key)
        self.had = self.key in doc
        doc[self.key] = self.value

    def undo(self, doc):
        if self.had:
            doc[self.key] = self.old
        else:
            del doc[self.key]


class AppendTag:
    def __init__(self, tag):
        self.tag = tag

    def execute(self, doc):
        doc.setdefault("tags", []).append(self.tag)

    def undo(self, doc):
        doc["tags"].pop()


class Editor:
    def __init__(self, doc):
        self.doc = doc
        self.history = []

    def run(self, cmd):
        cmd.execute(self.doc)
        self.history.append(cmd)
        return self

    def undo(self):
        cmd = self.history.pop()
        cmd.undo(self.doc)
        return cmd


ed = Editor({"title": "t"})
ed.run(SetField("author", "ann")).run(AppendTag("x")).run(SetField("title", "T2"))
doc = ed.doc                              #@row doc: present={title,author,tags} definite={title,author,tags} closed
c1 = ed.history[0]                        #@row c1: present={key,value,old,had} definite={key,value,old,had} closed
c2 = ed.history[1]                        #@row c2: present={tag} definite={tag} closed
undone = ed.undo()                        #@row undone: present={key,value,old,had} definite={key,value,old,had} closed
# After undoing the title change the title is back and the author is still there.
_ = 0                                     #@row doc: present={title,author,tags} definite={title,author,tags} closed
ed.undo()
ed.undo()
# The author field was not in the document before its command: undo deletes it.
_ = 0                                     #@row doc: present={title,tags} definite={title,tags} closed
doc["author"]                             #@ missing author
doc["title"]                              #@ ok title
