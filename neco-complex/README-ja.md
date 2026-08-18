# neco-complex

[English](README.md)

`neco-complex` は数値線形代数と信号処理の複素スカラーを提供します。実部と虚部は private なフィールドとし、算術、共役、ノルム、偏角を提供します。

## 公開 API

- `Complex<T>`: private な成分を保持し、アクセサーで観測する複素スカラー
- `Complex::new(re, im)`: 複素 スカラーを構築
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
