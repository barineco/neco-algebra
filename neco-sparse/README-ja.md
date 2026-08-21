# neco-sparse

[English](README.md)

`neco-sparse` は、標準ライブラリを使わない構成でも利用できる数値線形代数用の疎行列 crate です。座標を列挙する格納形式から、行単位で圧縮した格納形式へ変換できます。変換時には座標を並べ替え、同じ座標の値を一つにまとめます。

## 公開 API

- `CooMatrix<T>`: 形状内の座標と値を任意の順序で保持し、重複を許容
- `CooMatrix::to_csr`: 行と列の順で安定に並べ、重複座標の値を加算
- `CsrMatrix<T>`: 行ごとの格納範囲、列添字、値を検証済みの形で保持
- `CsrMatrix::from_parts`: 行オフセット、列添字、値の不変条件を検査して構築
- `CsrMatrix::row`: 一行を `CsrRow` として借用
- `CsrRow`: 列添字、値、対応する組を観測
- `LinearOperator<f64>`: CSR 行列とベクトルの積を計算し、入力長を検査

直接利用する crate は次の一つです。

```text
neco-linear-types
```

密行列との変換は上位の統合 crate が担います。

## 実行構成

既定の構成では標準ライブラリを使います。既定機能を無効にすると、動的メモリ確保を使う `no_std` 構成になります。

## ライセンス

MIT License です。
