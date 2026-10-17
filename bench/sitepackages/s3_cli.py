"""Scenario 3: a small CLI tool on click and rich."""

import json

import click
from rich.console import Console
from rich.table import Table

console = Console()

STOCK = [
    {"sku": "A100", "name": "Widget", "qty": 120, "price": 4.5},
    {"sku": "B310", "name": "Gadget", "qty": 3, "price": 24.99},
    {"sku": "C001", "name": "Gizmo", "qty": 0, "price": 120.0},
]


def low_stock(rows: list[dict], threshold: int) -> list[dict]:
    return [r for r in rows if r["qty"] < threshold]


def stock_table(rows: list[dict], title: str) -> Table:
    table = Table(title=title)
    table.add_column("SKU")
    table.add_column("Name")
    table.add_column("Qty", justify="right")
    table.add_column("Value", justify="right")
    for r in rows:
        value = r["qty"] * r["price"]
        table.add_row(r["sku"], r["name"], str(r["qty"]), f"{value:.2f}")
    return table


@click.group()
@click.option("--verbose", is_flag=True, help="Say more.")
@click.pass_context
def cli(ctx: click.Context, verbose: bool) -> None:
    ctx.ensure_object(dict)
    ctx.obj["verbose"] = verbose


@cli.command()
@click.option("--threshold", type=int, default=10, show_default=True)
@click.option("--json-out", "as_json", is_flag=True)
@click.pass_context
def report(ctx: click.Context, threshold: int, as_json: bool) -> None:
    rows = low_stock(STOCK, threshold)
    if as_json:
        click.echo(json.dumps(rows))
        return
    console.print(stock_table(rows, f"Below {threshold}"))
    if ctx.obj["verbose"]:
        console.print(f"[dim]{len(rows)} of {len(STOCK)} items[/dim]")


@cli.command()
@click.argument("sku")
def show(sku: str) -> None:
    match = [r for r in STOCK if r["sku"] == sku.upper()]
    if not match:
        raise click.BadParameter(f"no such SKU {sku}")
    item = match[0]
    console.print(f"{item['name']}: {item['qty']} at {item['price']}")


if __name__ == "__main__":
    cli(["--verbose", "report", "--threshold", "5"], standalone_mode=False)
