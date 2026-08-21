# neco-complex

[English](README.md)

`neco-complex` は、数値線形代数と信号処理に使う複素スカラーを提供します。実部と虚部は private なフィールドです。構築時には成分の有限性や、後続の零除算を検証しません。算術は成分型の演算を使い、検証失敗を返さない形です。

## 公開 API

- `Complex<T>`: private な成分を保持し、アクセサで観測する複素スカラー
- `Complex::new(re, im)`: 任意の実部と虚部をそのまま保持
- `Complex::real`: 実部を参照として観測
- `Complex::imaginary`: 虚部を参照として観測
- `Complex::real_value`: 実部の値を返す
- `Complex::imaginary_value`: 虚部の値を返す
- `Complex::set_real`: 実部を置き換える
- `Complex::set_imaginary`: 虚部を置き換える
- `Complex::conjugate`: 共役 スカラーを返す
- `Complex::norm_squared`: 二乗ノルムを返す
- `Complex<f64>::from_real`: 実数 スカラーを埋め込む
- `Complex<f32>::argument`: `std` の構成で偏角を返す
- `Complex<f64>::argument`: `std` の構成で偏角を返す
- `Complex<f64>::norm`: `std` の構成でユークリッドノルムを返す

## ランタイム構成

既定の構成は標準ライブラリを使います。既定機能を無効にすると、動的メモリ確保を必要としない `core` の構成になります。

## ライセンス

MIT License です。
