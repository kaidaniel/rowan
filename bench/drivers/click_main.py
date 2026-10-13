import click
from click.testing import CliRunner


@click.group()
@click.option("--verbose", "-v", count=True, help="More output.")
@click.pass_context
def cli(ctx, verbose):
    """A small inventory tool."""
    ctx.ensure_object(dict)
    ctx.obj["verbose"] = verbose


@cli.command()
@click.argument("name")
@click.option("--qty", type=int, default=1, show_default=True)
@click.option("--tag", multiple=True)
@click.pass_obj
def add(obj, name, qty, tag):
    """Add an item."""
    if obj["verbose"]:
        click.echo(f"adding {name}")
    click.echo(f"{name}: {qty} {','.join(tag)}")


@cli.command()
@click.option("--format", "fmt", type=click.Choice(["json", "text"]), default="text")
def report(fmt):
    """Print a report."""
    click.secho("report", fg="green", bold=True)
    click.echo(fmt)


runner = CliRunner()
r1 = runner.invoke(cli, ["-v", "add", "tea", "--qty", "3", "--tag", "x"])
r2 = runner.invoke(cli, ["report", "--format", "json"])
out = r1.output + r2.output
code = r1.exit_code
