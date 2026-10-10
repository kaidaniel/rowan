from rich.console import Console
from rich.table import Table
from rich.text import Text

console = Console(width=80)
table = Table(title="Orders")
table.add_column("id")
table.add_column("item")
table.add_row("1", "tea")
table.add_row("2", Text("coffee", style="bold"))
console.print(table)
console.print("[bold red]done[/]")
