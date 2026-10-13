# neco-sparse

[English](README.md)

`neco-sparse` は、数値線形代数で使う疎行列の crate で、標準ライブラリを使わない構成でも利用できます。座標形式 ( COO ) の行列を、圧縮行形式 ( CSR ) の行列へ変換できます。変換の際は座標を並べ替え、同じ座標の値を一つにまとめます。

## 公開 API

- `CooMatrix<T>`: 形状の範囲内の座標と値を任意の順序で保持する。同じ座標の重複を許す
- `CooMatrix::to_csr`: 行と列の順に安定に並べ、重複した座標の値を加算する
- `CsrMatrix<T>`: 行オフセット、列添字、値を検証済みの形で保持する。行オフセットは長さが行数 + 1 の配列で、各行の要素が値の配列のどこから始まるかを示す
- `CsrMatrix::from_parts`: 行オフセット、列添字、値の不変条件を検査して構築する
- `CsrMatrix::row`: 一行を `CsrRow` として借用する
- `CsrRow`: 列添字、値、その組を読み出す
- `LinearOperator<f64>`: CSR 行列とベクトルの積を計算し、入力の長さを検査する

直接依存する crate は `neco-linear-types` だけです。
CSR 形式の行列から密行列への変換は、`neco-generalized-eigen` の `GeneralizedEigenProblem::from_csr` が行います。

## ランタイム構成

既定では標準ライブラリを使います。既定機能を無効にすると、動的メモリ確保だけを前提とする `core + alloc` の構成になります。

## ライセンス

MIT License です。
