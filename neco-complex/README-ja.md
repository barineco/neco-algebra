# neco-complex

[English](README.md)

`neco-complex` は、数値線形代数と信号処理に使う複素数のスカラーの型を提供します。
実部と虚部は private なフィールドとして保持し、構築時には任意の成分の値を受け取ります。算術は成分の型の演算に従い、非有限値と零除算の扱いも成分の型と同じです。

## 公開 API

- `Complex<T>`: private な成分を保持し、アクセサで読み出す複素数のスカラー
- `Complex::new(re, im)`: 与えた実部と虚部をそのまま保持する
- `Complex::real`: 実部への参照を返す
- `Complex::imaginary`: 虚部への参照を返す
- `Complex::real_value`: 実部の値を返す
- `Complex::imaginary_value`: 虚部の値を返す
- `Complex::set_real`: 実部を置き換える
- `Complex::set_imaginary`: 虚部を置き換える
- `Complex::conjugate`: 共役複素数を返す
- `Complex::norm_squared`: 絶対値の二乗を返す
- `Complex<f64>::from_real`: 実数のスカラーを複素数へ埋め込む
- `Complex<f32>::argument`: `std` の構成で偏角を返す
- `Complex<f64>::argument`: `std` の構成で偏角を返す
- `Complex<f64>::norm`: `std` の構成で絶対値 ( ユークリッドノルム ) を返す

## ランタイム構成

既定では標準ライブラリを使います。既定機能を無効にすると、動的メモリ確保を必要としない `core` だけの構成になります。

## ライセンス

MIT License です。
