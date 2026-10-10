# numpy model: standardisation, a train / test split by slicing, least squares through the normal equations, predictions and metrics, a k-means step with broadcasting distances
import numpy as np


def standardize(X):
    mu = X.mean(axis=0)  #@shape mu: [*X.shape[1:]]
    sd = X.std(axis=0, keepdims=True)  #@shape sd: [1, *X.shape[1:]]
    return (X - mu) / sd


def add_bias(X):
    n, d = X.shape
    ones = np.ones((n, 1))  #@shape ones: [X.shape[0], 1]
    return np.hstack([ones, X])  #@shape: [X.shape[0], X.shape[1] + 1]


def fit(X, y):
    n, d = X.shape
    (m,) = y.shape
    XtX = X.T @ X  #@shape XtX: [X.shape[1], X.shape[1]]
    Xty = X.T @ y  #@shape Xty: [X.shape[1]]
    w = np.linalg.solve(XtX, Xty)
    return w


def predict(X, w):
    n, d = X.shape
    return X @ w


def mse(y, yhat):
    err = y - yhat
    return float((err ** 2).mean())


def assign(X, C):
    n, d = X.shape
    k, d2 = C.shape
    diff = X[:, None, :] - C[None, :, :]  #@shape diff: [X.shape[0], C.shape[0], max(X.shape[1], C.shape[1])]
    dist = (diff ** 2).sum(axis=-1)  #@shape dist: [X.shape[0], C.shape[0]]
    lab = dist.argmin(axis=1)  #@shape lab: [X.shape[0]]
    return lab


def centroids(X, lab, k):
    n, d = X.shape
    C = np.zeros((k, d))  #@shape C: [k, X.shape[1]]
    for j in range(k):
        C[j] = X[lab == j].mean(axis=0)
    return C


if __name__ == "__main__":
    X = np.stack([np.arange(20.0), (np.arange(20.0) % 7) ** 2], axis=1)  #@shape X: [20, 2]
    y = X @ np.array([2.0, -1.0]) + 3.0  #@shape y: [20]
    Z = standardize(X)  #@shape Z: [20, 2]
    A = add_bias(Z)  #@shape A: [20, 3]
    train, test = A[:15], A[15:]  #@shape train: [15, 3]; test: [5, 3]
    w = fit(train, y[:15])
    yhat = predict(test, w)  #@shape yhat: [5]
    C0 = X[:3]  #@shape C0: [3, 2]
    lab = assign(X, C0)  #@shape lab: [20]
    C1 = centroids(X, lab, 3)  #@shape C1: [3, 2]
