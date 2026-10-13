"""Driver for the sktime benchmark: the forecasting workflow of sktime's tutorials on a pandas
series (split, horizon, naive / trend / reduction forecasters, pipelines with transformers,
backtesting, tuning, metrics), the data container checks and conversions (mtypes), and a
panel classifier."""

import numpy as np
import pandas as pd

from sktime.datatypes import check_is_mtype, check_raise, convert_to, mtype
from sktime.forecasting.base import ForecastingHorizon
from sktime.forecasting.compose import (
    EnsembleForecaster,
    TransformedTargetForecaster,
    make_reduction,
)
from sktime.forecasting.model_evaluation import evaluate
from sktime.forecasting.model_selection import ForecastingGridSearchCV
from sktime.forecasting.naive import NaiveForecaster
from sktime.forecasting.trend import PolynomialTrendForecaster, TrendForecaster
from sktime.performance_metrics.forecasting import (
    MeanAbsolutePercentageError,
    mean_absolute_error,
    mean_absolute_percentage_error,
)
from sktime.split import ExpandingWindowSplitter, SlidingWindowSplitter, temporal_train_test_split
from sktime.transformations.series.boxcox import LogTransformer
from sktime.transformations.series.detrend import Deseasonalizer, Detrender
from sktime.transformations.series.difference import Differencer
from sktime.transformations.series.lag import Lag
from sktime.transformations.series.summarize import WindowSummarizer
from sktime.classification.distance_based import KNeighborsTimeSeriesClassifier
from sktime.transformations.panel.reduce import Tabularizer
from sklearn.linear_model import LinearRegression

# monthly passengers, like sktime's airline dataset
index = pd.period_range("1949-01", periods=144, freq="M")
trend = np.linspace(110, 430, 144)
season = 1 + 0.2 * np.sin(np.arange(144) * 2 * np.pi / 12)
y = pd.Series(np.round(trend * season), index=index, name="passengers")
X = pd.DataFrame({"promo": (np.arange(144) % 12 == 11).astype(float), "temp": np.cos(np.arange(144) / 2)}, index=index)

check_raise(y, mtype="pd.Series")
valid, msg, meta = check_is_mtype(y, mtype="pd.Series", return_metadata=True)
as_frame = convert_to(y, to_type="pd.DataFrame")
as_numpy = convert_to(y, to_type="np.ndarray")
kind = mtype(X, as_scitype="Series")

y_train, y_test, X_train, X_test = temporal_train_test_split(y, X, test_size=36)
fh = ForecastingHorizon(y_test.index, is_relative=False)

forecasters = {
    "last": NaiveForecaster(strategy="last"),
    "seasonal": NaiveForecaster(strategy="last", sp=12),
    "drift": NaiveForecaster(strategy="drift"),
    "trend": PolynomialTrendForecaster(degree=2),
    "pipeline": TransformedTargetForecaster(
        [
            ("log", LogTransformer()),
            ("deseasonalize", Deseasonalizer(model="multiplicative", sp=12)),
            ("detrend", Detrender(forecaster=TrendForecaster())),
            ("forecast", NaiveForecaster(strategy="drift")),
        ]
    ),
    "reduction": make_reduction(LinearRegression(), window_length=15, strategy="recursive"),
    "ensemble": EnsembleForecaster(
        [("naive", NaiveForecaster(sp=12)), ("trend", PolynomialTrendForecaster(degree=1))]
    ),
}

scores = {}
for name, forecaster in forecasters.items():
    forecaster.fit(y_train, fh=fh)
    y_pred = forecaster.predict()
    scores[name] = {
        "mape": mean_absolute_percentage_error(y_test, y_pred, symmetric=False),
        "mae": mean_absolute_error(y_test, y_pred),
        "first": float(y_pred.iloc[0]),
    }
best = min(scores, key=lambda n: scores[n]["mape"])

interval = forecasters["drift"].predict_interval(coverage=0.9)
updated = forecasters["seasonal"].update_predict_single(y_test.iloc[:12])

cv = ExpandingWindowSplitter(fh=[1, 2, 3], initial_window=96, step_length=12)
backtest = evaluate(forecaster=NaiveForecaster(sp=12), y=y, cv=cv, strategy="refit", scoring=MeanAbsolutePercentageError())
grid = ForecastingGridSearchCV(
    NaiveForecaster(),
    cv=SlidingWindowSplitter(fh=[1, 2, 3], window_length=60, step_length=12),
    param_grid={"strategy": ["last", "mean", "drift"], "sp": [1, 12]},
)
grid.fit(y_train)
tuned = grid.best_params_

features = WindowSummarizer(lag_feature={"lag": [1, 12], "mean": [[1, 3]]}).fit_transform(y)
lags = Lag(lags=[1, 2, 3]).fit_transform(X)
diffs = Differencer(lags=[1, 12]).fit_transform(y)

# a small panel: 20 instances of 3 variables over 30 steps, two classes
rng = np.random.default_rng(0)
panel = rng.normal(size=(20, 3, 30))
labels = np.array(["up", "down"] * 10)
panel[labels == "up"] += np.linspace(0, 2, 30)
clf = KNeighborsTimeSeriesClassifier(n_neighbors=1, distance="euclidean")
clf.fit(panel[:14], labels[:14])
predicted = clf.predict(panel[14:])
flat = Tabularizer().fit_transform(convert_to(panel, to_type="nested_univ"))

summary = {
    "valid": valid,
    "meta": {k: meta[k] for k in ("is_univariate", "is_equally_spaced", "has_nans")},
    "kind": kind,
    "frame": list(as_frame.columns),
    "numpy": as_numpy.shape,
    "scores": scores,
    "best": best,
    "interval": list(interval.columns),
    "updated": len(updated),
    "backtest": float(backtest.iloc[:, 0].mean()),
    "tuned": tuned,
    "features": list(features.columns),
    "lags": list(lags.columns),
    "diffs": int(diffs.isna().sum()),
    "accuracy": float((predicted == labels[14:]).mean()),
    "flat": flat.shape,
}
print(summary)
