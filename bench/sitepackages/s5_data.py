"""Scenario 5: the data-science stack (numpy and pandas are modelled, so never followed;
scikit-learn, scipy, joblib and threadpoolctl are)."""

import numpy as np
import pandas as pd
from sklearn.linear_model import LinearRegression
from sklearn.model_selection import train_test_split
from sklearn.preprocessing import StandardScaler


def load() -> pd.DataFrame:
    rng = np.random.default_rng(0)
    n = 200
    df = pd.DataFrame({
        "size": rng.uniform(30, 200, n),
        "rooms": rng.integers(1, 6, n),
        "age": rng.uniform(0, 80, n),
    })
    df["price"] = 1000 * df["size"] + 5000 * df["rooms"] - 300 * df["age"] + rng.normal(0, 5000, n)
    return df


def fit(df: pd.DataFrame) -> dict:
    X = df[["size", "rooms", "age"]].to_numpy()
    y = df["price"].to_numpy()
    X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.25, random_state=0)
    scaler = StandardScaler().fit(X_train)
    model = LinearRegression().fit(scaler.transform(X_train), y_train)
    score = model.score(scaler.transform(X_test), y_test)
    return {"r2": score, "coef": model.coef_.tolist(), "n_test": len(y_test)}


def main() -> None:
    df = load()
    summary = df.groupby("rooms")["price"].mean()
    result = fit(df)
    print(summary.round(0))
    print(f"R^2 = {result['r2']:.3f}, {result['n_test']} test rows")


if __name__ == "__main__":
    main()
