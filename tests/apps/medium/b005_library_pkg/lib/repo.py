"""Repository: books by isbn, members by id (two literal members), loans by id; lookups of
unknown ids raise LookupError."""
from lib.models import Loan


class Repo:
    def __init__(self):
        self.books = {}
        self.members = {"m1": {"name": "Ann", "tier": "gold"}, "m2": {"name": "Bo", "tier": "basic"}}
        self.loans = {}
        self.next_id = 1

    def add_book(self, data):
        rec = dict(data)
        rec["available"] = rec["copies"]         #@ ok copies
        self.books[rec["isbn"]] = rec
        #@row rec: present={isbn,title,genre,copies,available} definite={isbn,title,genre,copies,available} closed
        return rec

    def book(self, isbn):
        rec = self.books.get(isbn)
        if rec is None:
            raise LookupError(f"book {isbn}")
        return rec

    def member(self, mid):
        if mid not in self.members:
            raise LookupError(f"member {mid}")
        return self.members[mid]

    def new_loan(self, member, isbn, start, due):
        loan = Loan(self.next_id, member, isbn, start, due)
        self.loans[loan.id] = loan
        self.next_id += 1
        return loan
