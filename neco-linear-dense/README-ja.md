# neco-linear-dense

[English](README.md)

`neco-linear-dense` は、数値線形代数に使う密行列を提供します。行列は形状と一致する個数の要素を列優先順で保持します。構築時には格納長、要素数の容量、表現できる格納長、メモリ確保を検査します。`f64` の行列ベクトル積は、入力ベクトルの長さも検査します。

## 公開 API

- `DenseMatrix<T>`: 形状と要素を private なフィールドで保持する密行列
- `from_column_major`: 列優先の要素から行列を構築
- `from_row_major`: 行優先の要素を列優先の保存へ変換
- `try_zeros`: 一つの値で要素を初期化して構築
- `value`: 検証済みの行添字と列添字で要素を参照
- `LinearOperator<f64>`: 行列とベクトルの積を計算

## ランタイム構成

既定の構成は標準ライブラリを使います。既定機能を無効にすると、`alloc` を使う `no_std` の構成になります。

## ライセンス

MIT License です。
