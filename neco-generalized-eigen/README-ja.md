# neco-generalized-eigen

[English](README.md)

一般化固有値問題に用いる検証済みの公開型を提供します。数値計算の実装は、問題データ、固有対、質量内積で正規直交する固有空間、質量内積による射影、収束状態、複素シフトを公開値として利用します。

## 公開 API

- `GeneralizedEigenProblem`: 整合する密な剛性行列と質量行列
- `GeneralizedEigenProblem::from_dense`: 検査付きの密な問題の構築
- `GeneralizedEigenProblem::from_csr`: 検査付き CSR から密行列への変換
- `EigenResidual`: 絶対残差と相対残差
- `Eigenpair`: 検証済みの固有値、ベクトル、残差
- `Eigenspace`: 与えられた問題の一つの固有値に属する、質量内積で正規直交する基底
- `EigenProjector`: 固有空間への質量内積による射影
- `ConvergenceStatus`: 検証付きコンストラクタからだけ構築できるソルバーの進行情報
- `EigenShift`: 有限な `Complex<f64>` のシフト
- `GeneralizedEigenError`: 検証と線形代数の失敗

`Eigenspace::new` は、同じ入力問題と指定固有値から各残差を再計算します。各基底の質量ノルム、基底間の相互直交、固有対の由来が検証を通った場合だけ固有空間を構築できます。

## モード数

収束状態は、要求した個数、返却した個数、許容差を満たした個数を区別します。

```text
requested_modes: ソルバーに要求した個数
returned_modes: 固有空間として返却した個数
converged_modes: 残差が許容差を満たす返却済みの個数
```

相対残差は、絶対残差を剛性積のノルム・固有値で伸縮した質量積のノルム・最小の正の `f64` 値の最大値で除した値です。

ソルバーは、厳密に重複する固有値の固有空間を完全な形で返します。このため返却数と収束数は要求数を超える場合があります。収束数は返却数以下であり、収束した結果では返却数と収束数が等しくなります。

## 依存

- `neco-linear-types`: ベクトルと線形作用素の型
- `neco-linear-dense`: 密な問題行列
- `neco-sparse`: CSR 問題行列の変換
- `neco-complex`: 複素シフト

## ランタイム構成

既定の構成は標準ライブラリを使います。既定機能を無効にすると、標準ライブラリを使わない `no_std + alloc` 構成になります。

## ライセンス

MIT License です。
