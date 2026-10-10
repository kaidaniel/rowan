"""Processing layer on pandas: events as a frame, transform plugins, per-user aggregates,
a region x source pivot and a merge with user profiles."""
import pandas as pd

from dp.registry import Plugin, register

FRAME_COLUMNS = ["id", "user", "amount", "region", "ts", "source"]


def events_frame(events):
    rows = [e.model_dump(include=set(FRAME_COLUMNS)) for e in events]
    df = pd.DataFrame(rows, columns=FRAME_COLUMNS)
    df["day"] = df["ts"].str.slice(0, 10)
    return df


class Transform(Plugin):
    """Base of the frame transforms."""

    def apply(self, df, settings):
        raise NotImplementedError


@register("transform", "threshold")
class ThresholdFilter(Transform):
    """Drop events below the configured amount."""

    def apply(self, df, settings):
        return df[df["amount"] >= settings.threshold].reset_index(drop=True)


@register("transform", "convert", aliases=("fx",))
class CurrencyConvert(Transform):
    """Convert EUR amounts into the settings currency."""

    RATES = {"EUR": 1.0, "USD": 1.5, "NOK": 10.0}

    def apply(self, df, settings):
        rate = self.RATES.get(settings.currency, 1.0)
        out = df.copy()
        out["amount"] = out["amount"] * rate
        out["currency"] = settings.currency
        return out


@register("transform", "flag")
class FlagLarge(Transform):
    """Mark events at or above `limit` (default 20)."""

    def apply(self, df, settings):
        out = df.copy()
        out["large"] = out["amount"] >= self.option("limit", 20)
        return out


def by_user(df):
    g = df.groupby("user", as_index=False).agg(n=("id", "count"), total=("amount", "sum"), regions=("region", "nunique"))
    return g.sort_values(["total", "user"], ascending=[False, True]).reset_index(drop=True)


def pivot_region_source(df):
    return df.pivot_table(index="region", columns="source", values="amount", aggfunc="sum", fill_value=0)


def daily(df):
    return df.groupby(["day", "region"], as_index=False)["amount"].sum()


def with_profiles(users, profiles):
    prof = pd.DataFrame(profiles, columns=["user", "tier", "since"])
    m = users.merge(prof, on="user", how="left")
    m["tier"] = m["tier"].fillna("none")
    m["since"] = m["since"].fillna(0).astype(int)
    return m


def pivot_dict(pivot):
    return {src: {region: float(v) for region, v in pivot[src].items()} for src in pivot.columns}
