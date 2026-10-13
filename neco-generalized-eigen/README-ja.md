# neco-generalized-eigen

[English](README.md)

`neco-generalized-eigen` は、一般化固有値問題に用いる検証済みの公開型を提供します。`core + alloc` の構成でも利用できます。問題のデータ、固有対、質量内積で正規直交する固有空間、質量内積による射影、収束状態、スペクトルシフトを、検証を通った値として固有値ソルバーとの間で受け渡せます。

## 公開 API

- `GeneralizedEigenProblem`: 形の整合した、密な剛性行列と質量行列
- `GeneralizedEigenProblem::from_dense`: 密な問題を検査付きで構築する
- `GeneralizedEigenProblem::from_csr`: CSR 形式の行列を検査付きで密な行列へ変換して構築する
- `EigenResidual`: 絶対残差と相対残差
- `EigenResidual::meets_tolerance`: 絶対残差または相対残差による収束判定
- `Eigenpair`: 検証済みの固有値、固有ベクトル、残差
- `Eigenspace`: 与えた問題の一つの固有値に属する、質量内積で正規直交する基底
- `EigenProjector`: 固有空間への、質量内積による射影
- `ConvergenceStatus`: 検証付きのコンストラクタからだけ構築できる、ソルバーの進行の情報
- `EigenShift`: ソルバーが結果に記録するスペクトルシフト。有限な `Complex<f64>` の値
- `GeneralizedEigenError`: 検証と線形代数の失敗

`Eigenspace::new` は、同じ入力の問題と指定した固有値から、各基底ベクトルの残差を再計算します。各基底ベクトルの質量ノルム、基底ベクトルどうしの直交性、固有対の由来 ( 残差が入力の問題から再計算した値と一致すること ) の検証を通った場合だけ、固有空間を構築できます。

## モード数

収束状態は、要求した個数、返した個数、許容差を満たした個数を区別します。

```text
requested_modes: ソルバーに要求した個数
returned_modes: 固有空間として返した個数
converged_modes: 返したもののうち、残差が許容差を満たす個数
```

相対残差は、絶対残差を、三つの値のうち最大のもので割った値です。

- 剛性行列と固有ベクトルの積のノルム
- 質量行列と固有ベクトルの積に固有値を掛けたもののノルム
- 正の正規化数の最小値 ( `f64::MIN_POSITIVE` )

ソルバーは、厳密に重複する固有値の固有空間を完全な形で返します。そのため、返した個数と収束した個数は、要求した個数を超える場合があります。収束した個数は返した個数以下であり、収束した結果では二つの個数が等しくなります。

## 依存

- `neco-linear-types`: ベクトルと線形作用素の型
- `neco-linear-dense`: 密な問題行列
- `neco-sparse`: CSR 形式の問題行列の変換
- `neco-complex`: スペクトルシフトを表す複素数

## ランタイム構成

既定では標準ライブラリを使います。既定機能を無効にすると、動的メモリ確保だけを前提とする `core + alloc` の構成になります。

## ライセンス

MIT License です。
