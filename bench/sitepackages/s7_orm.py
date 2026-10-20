"""Scenario 7: a small data layer on SQLAlchemy 2 (declarative models, a session on an
in-memory SQLite database, select queries): an ORM whose import builds registries."""

from sqlalchemy import ForeignKey, String, create_engine, func, select
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column, relationship


class Base(DeclarativeBase):
    pass


class Customer(Base):
    __tablename__ = "customer"
    id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str] = mapped_column(String(50))
    orders: Mapped[list["Order"]] = relationship(back_populates="customer")


class Order(Base):
    __tablename__ = "orders"
    id: Mapped[int] = mapped_column(primary_key=True)
    customer_id: Mapped[int] = mapped_column(ForeignKey("customer.id"))
    total: Mapped[float]
    customer: Mapped[Customer] = relationship(back_populates="orders")


def seed(session: Session) -> None:
    ada = Customer(name="Ada", orders=[Order(total=12.5), Order(total=30.0)])
    bob = Customer(name="Bob", orders=[Order(total=7.25)])
    session.add_all([ada, bob])
    session.commit()


def totals(session: Session) -> list[dict]:
    q = select(Customer.name, func.sum(Order.total)).join(Order).group_by(Customer.name).order_by(Customer.name)
    return [{"name": name, "total": total} for name, total in session.execute(q)]


def main() -> None:
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    with Session(engine) as session:
        seed(session)
        rows = totals(session)
    for r in rows:
        print(f"{r['name']}: {r['total']:.2f}")


if __name__ == "__main__":
    main()
