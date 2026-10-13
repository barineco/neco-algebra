# neco-linear-exact

[English](README.md)

`neco-linear-exact` は、厳密な密行列の線形演算を提供する crate です。有理数、正規化した冪根の形式和、実代数的数を扱います。各行列は、検証済みの形状と、行優先の順で格納した要素を持ちます。

## 公開 API

- `ExactScalar`: 失敗を値として返すスカラー演算
- `ExactLinearError`: 演算の失敗
- `ExactMatrix<T>`: 形状と格納長を検証した、行優先の厳密な行列
- `ExactLinearSolution<T>`: 一意な解、核の基底を伴うアフィン解、解なしの三つの状態
- `determinant`: 正方行列の行列式
- `rank`: 行列の階数
- `kernel_basis`: 核の基底ベクトル
- `solve`: 連立一次方程式の解
- `CertifiedMatrixInput`: 数値の行列、証明付きの式の組 ( `CertifiedF64Bundle` )、行優先の式の識別子を対応付けた入力
- `CertifiedMatrixEntry`: 一つの行列位置について、式の識別子と証明付きの値を保持する値
- `MassNormalizationReference`: 質量行列の入力に、その扱い ( 単位行列として扱うか、正定値行列として Cholesky 分解するか ) を対応付けた参照
- `CertifiedGeneralizedEigenInput`: 剛性行列の入力と、質量行列の入力とその扱いを結合した入力
- `project_vector_f64`: ベクトルの証明付きの射影
- `project_matrix_f64`: 行列の証明付きの射影

## 証明付きの行列入力

`CertifiedMatrixInput::from_product_projection` は、密行列と、厳密計算の結果に含まれる一つの証明付きの式の組 ( `CertifiedF64Bundle` ) を照合して、行列入力を構築します。照合では、計算結果と式の組のそれぞれが持つ来歴を使います。

- `ExactComputationProduct`: 厳密計算の結果と、その来歴
- `CertifiedF64Bundle`: 証明付きの式と、その来歴

照合で一致を確かめる項目は、計算結果と式の組の両方が持つ値です。

- 形状と、行優先の式の識別子
- `neco-expr` が定める所有者、消費点、要求精度、実装元
- 不透明な計算結果の識別子
- `f64` のビット列

構築した入力は、照合した `CertifiedF64Bundle` を来歴の原典として保持します。
各要素の値と来歴は `entry` で読み出し、検証の失敗は `NecoFailure` で受け取ります。

- `entry(row, column)`: 行列位置、式の識別子、消費点、証明付きの値、包含区間、射影の絶対誤差の上限
- `NecoFailure`: 検証に失敗した行列入力の種類 ( `MatrixInputRole`: 一般の行列、剛性行列、質量行列、質量行列とその扱いの参照 ) を報告する。要素に由来する失敗では、行と列の位置も報告する

`CertifiedGeneralizedEigenInput` は、剛性行列の入力と、必須の `MassNormalizationReference` を結合した入力です。

- 質量行列の扱い: `Identity` では単位行列として扱い、`PositiveDefiniteCholesky` では正定値行列として Cholesky 分解する
- `mass()`: 質量行列の入力とその扱いを、固有値ソルバーが読めるように返す

## 行列の観測と証明付きの射影

`ExactMatrix<T>` からは、形状、行数、列数、座標で指定した値、行優先の全要素を読めます。
証明付きの射影は、厳密なベクトルまたは行列の各要素から、有限な `f64` の値と、厳密値を含む区間を求めます。射影の結果は、ベクトルと行列のそれぞれの型で返ります。

- `CertifiedVectorProjection`: 射影の方針、射影後のベクトル、各要素の証明付きの値、絶対誤差の上限の総和
- `CertifiedMatrixProjection`: 射影の方針、射影後の行列、行優先の各要素の証明付きの値、絶対誤差の上限の総和

## ガウス消去法

ガウス消去法は、列を左から順に調べ、現在のピボット行以降で最初に零でない行を次のピボット行に選びます。この決定的な選び方によって、行の交換、核の基底、解ベクトルが決まります。

## ランタイム構成

既定では標準ライブラリを使います。既定機能を無効にすると、動的メモリ確保だけを前提とする `core + alloc` の構成になります。

## ライセンス

MIT License です。
