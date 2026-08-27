# neco algebra

[English](README.md)

neco algebra は、代数的数を厳密に扱う Rust crate 群です。数を式のまま保持して計算し、誤差上限を伴う浮動小数点値への近似は最後に一度だけ行います。

判定はすべて正規形の上で行います。値が零か、二つの値が等しいかは、正規形の構造を照合して決定します。たとえば次の等式は、左辺の積が正規化によって単一の項 `1` になるため、近似を使わずに確定できます。

$$
(\sqrt{3} + \sqrt{2})(\sqrt{3} - \sqrt{2}) = 1
$$

形式和層と代数的数層の符号は、最初に構造上の零を判定します。値が非零なら、厳密値を含む区間を必要な幅まで精密化して符号を決定します。

## 依存関係

矢印は依存される crate から、その crate を利用する crate へ向きます。

```mermaid
flowchart LR
    bigint["neco-bigint"]
    monomial["neco-monomial"]
    formsum["neco-formsum"]
    algnum["neco-algnum"]
    expr["neco-expr"]
    complex["neco-complex"]
    linear_types["neco-linear-types"]
    linear_dense["neco-linear-dense"]
    sparse["neco-sparse"]
    generalized_eigen["neco-generalized-eigen"]
    eigensolve["neco-eigensolve"]
    eigensolve_faer["neco-eigensolve-faer"]
    linear_exact["neco-linear-exact"]
    faer["faer"]

    bigint --> monomial
    bigint --> formsum
    monomial --> formsum
    bigint --> algnum
    formsum --> algnum
    bigint --> expr
    monomial --> expr
    formsum --> expr
    algnum --> expr

    linear_types --> linear_dense
    linear_types --> sparse
    linear_types --> generalized_eigen
    linear_dense --> generalized_eigen
    sparse --> generalized_eigen
    complex --> generalized_eigen
    linear_types --> eigensolve
    linear_dense --> eigensolve
    sparse --> eigensolve
    complex --> eigensolve
    generalized_eigen --> eigensolve
    linear_types --> eigensolve_faer
    linear_dense --> eigensolve_faer
    complex --> eigensolve_faer
    generalized_eigen --> eigensolve_faer
    eigensolve --> eigensolve_faer
    linear_types --> linear_exact
    bigint --> linear_exact
    formsum --> linear_exact
    algnum --> linear_exact
    expr --> linear_exact
    faer --> eigensolve_faer
```

## crate 一覧

- [`neco-bigint`](https://crates.io/crates/neco-bigint): 任意精度の自然数・整数・有理数、二進有理数、検証済みの包含区間
- [`neco-monomial`](https://crates.io/crates/neco-monomial): 有理数指数を持つ単項式による冪根の厳密表現
- [`neco-formsum`](https://crates.io/crates/neco-formsum): 正規化した単項式の有理係数和
- [`neco-algnum`](https://crates.io/crates/neco-algnum): 最小多項式と実根番号で同定する実代数的数
- [`neco-expr`](https://crates.io/crates/neco-expr): 誤差上限付き浮動小数点値への式グラフ解決
- [`neco-complex`](https://crates.io/crates/neco-complex): 数値線形代数と信号処理の複素スカラー
- [`neco-linear-types`](https://crates.io/crates/neco-linear-types): 形状、ベクトル、線形作用素
- [`neco-linear-dense`](https://crates.io/crates/neco-linear-dense): 数値線形代数の密行列
- [`neco-sparse`](https://crates.io/crates/neco-sparse): 疎行列と COO から CSR への変換
- [`neco-generalized-eigen`](https://crates.io/crates/neco-generalized-eigen): 一般化固有値問題、固有空間、射影、収束状態の検証済み型
- [`neco-eigensolve`](https://crates.io/crates/neco-eigensolve): 単位質量行列を持つ実対称問題の決定的 Jacobi 固有値計算
- [`neco-eigensolve-faer`](https://crates.io/crates/neco-eigensolve-faer): 正定値質量行列を扱う `faer` アダプター
- [`neco-linear-exact`](https://crates.io/crates/neco-linear-exact): 有理数、冪根、実代数的数に対する厳密行列、ガウス消去法、連立一次方程式の解

## 本リポジトリを利用するリポジトリ

- [`neco-calculus`](https://github.com/barineco/neco-calculus): 厳密な多項式微積分
- [`neco-geometry`](https://github.com/barineco/neco-geometry): 厳密な代数幾何計算

既定の構成は次のとおりです。

- `neco-eigensolve-faer` 以外の crate: 既定で標準ライブラリを使い、既定機能を無効にすると `core + alloc` 構成で利用できます
- `neco-eigensolve-faer`: `faer` の標準ライブラリ機能を使うため、標準ライブラリが必要です

## ライセンス

MIT License です。[LICENSE](LICENSE) を参照してください。
