# neco-eigensolve

[English](README.md)

`neco-eigensolve` は、`no_std + alloc` 環境で実対称一般化固有値問題を決定的な手順で解く数値ソルバーです。

## 公開 API

- `EigensolveConfig`: 要求するモード数、許容差、反復上限
- `EigensolveError`: 入力・設定・結果の検証とメモリ確保の失敗
- `EigensolveRequest<R>`: 問題、構成、利用者が所有する射影参照
- `EigensolveResult<R>`: 固有空間、収束状態、スペクトルシフト、射影参照
- `solve_symmetric_f64`: 密な実対称問題
- `solve_request_symmetric_f64`: 射影参照を計算結果へ移す要求 API
- `solve_csr_symmetric_f64`: CSR の実対称問題

## 計算エンジン

Jacobi の計算部は厳密な単位質量行列を受理します。行と列の順で最大の非対角要素を選び、決定的な回転を適用し、正規の符号を持つ基底ベクトルから固有空間を返します。

固有空間と収束の規則は次のとおりです。

- 算出した固有値が厳密に等しい基底だけが一つの固有空間をなし、許容差は残差による収束の判定にだけ使います
- 要求する個数が固有空間の途中に達しても分断せず、完全な固有空間を返すため、返却数は要求数を超える場合があります
- 絶対残差または相対残差が対応する許容差を満たすモードを収束と判定します
- 結果のスペクトルシフトは常に `Complex<f64>::zero()` です

## 依存

- `neco-generalized-eigen`: 一般化問題と結果の型
- `neco-linear-dense`: 密な問題行列
- `neco-sparse`: CSR 問題行列
- `neco-complex`: 複素スペクトルシフト

## ランタイム構成

既定の構成は標準ライブラリを使います。既定機能を無効にすると、標準ライブラリを使わない `no_std + alloc` 構成になります。

## ライセンス

MIT License です。
