# neco-linear-types

[English](README.md)

`neco-linear-types` は、数値線形代数で使う形状、ベクトルの保持、線形作用の公開型を提供します。標準機能を無効にした構成では、標準ライブラリを使わず、動的メモリ確保だけで利用できます。

## 公開 API

- `Shape`: 行数と列数を保持し、要素数の容量超過を検査
- `RowIndex`, `ColumnIndex`: 形状の検証を通して生成する添字
- `Vector<T>`: 値を所有し、長さ、要素、全要素を観測
- `LinearOperator<T>`: 入力長、出力長、作用を定める線形作用素
- `LinearError`: 次元、添字、格納長、容量、メモリ確保、格納状態の失敗

線形作用素の公開シグネチャは次のとおりです。

```text
domain(&self) -> usize
codomain(&self) -> usize
apply(&self, input: &Vector<T>) -> Result<Vector<T>, LinearError>
```

入力と出力の長さは、各実装が検査します。

## ランタイム構成

既定の構成は標準ライブラリを使います。既定機能を無効にすると、標準ライブラリを使わない `no_std + alloc` 構成になります。

## ライセンス

MIT License です。
