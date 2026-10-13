# neco-eigensolve-certified

[English](README.md)

`neco-eigensolve-certified` は、一般化固有値問題の各固有対について、前進誤差の証明付きの上界と、周波数の証明付きの区間を計算します。
数値計算は、入力の `MassNormalizationMethod` に応じて、同じリポジトリの二つのソルバーのいずれかへ委譲します。

- `Identity`: 質量行列を単位行列として扱い、`neco-eigensolve` へ委譲する
- `PositiveDefiniteCholesky`: 質量行列を正定値行列として Cholesky 分解し、`neco-eigensolve-faer` へ委譲する

## 公開 API

- `solve_certified_generalized_eigen`: 各固有対の誤差の上界を証明付きで求める関数
- `CertifiedEigenSolution`: 固有空間、収束状態、質量行列の最小固有値の下界、入力への参照をまとめた解
- `CertifiedEigenspace`: 一つの固有値と、その証明付きの基底
- `CertifiedEigenpair`: 三つの証明付きの値を持つ固有対
- `CertifiedFrequency`: 周波数の推定値と、外向きに丸めた区間
- `CertifiedEigenError`: 求解の失敗 ( 後述 )
- `certify_spectral_clusters`: 完全なスペクトルを、モード番号を確定したクラスタへ分割する
- `SpectralClusters`、`SpectralCluster`: 分割の全体と、その中の一つのクラスタ
- `ModeOrdinal`: 確定したモード番号、またはクラスタ内の番号の範囲
- `correspond_to_reference`: 二つの解の部分空間どうしの距離の上界を求める
- `ModeCorrespondence`、`BlockCorrespondence`: その対応と、対応を構成するブロック
- `ClusterError`: 上の二つの関数が返す失敗

## 保証する範囲

剛性行列 $K$ と質量行列 $M$ の組を行列ペンシルと呼びます。行列ペンシルに対する一般化固有値問題は、次の式で表されます。

$$ K x = \lambda M x $$

固有対ごとの上界は、入力の浮動小数点行列が表す厳密なペンシルの対称部分について、その固有値のうち少なくとも一つまでの距離を上から抑えます。後述のクラスタの値は、同じ厳密なペンシルについて、固有値の個数、クラスタの間隔、条件数の上界、部分空間の角の上界を表します。これらは、固有値までの距離とは別の量です。

- 各固有対については、その距離以内に固有値が存在することだけを主張する。一方、モード番号は、完全なスペクトルの分割で単独になったクラスタについてだけ確定する ( 後述 )
- 厳密な剛性行列と質量行列が、それぞれ対称であることを前提とする。この前提は、この crate では検査できない

`CertifiedFrequency` は、固有値を角振動数の二乗とみなします。

- `value`: 固有値 $\lambda$ に対する $\sqrt{\lambda} / 2\pi$ の推定値
- `lower_bound`、`upper_bound`: 同じ単位で表した区間の両端

## 誤差モデル

浮動小数点の残差 ( `EigenResidual` ) は、残差の収束判定にだけ使います。この残差は桁落ちによって真の残差より小さく丸まることがあるため、前進誤差の上界の根拠には使いません。

入力の行列の各要素は、厳密値を f64 へ丸めた値です。この丸めの絶対誤差の上界を、行列の全要素について合計した値を、射影誤差と呼びます。剛性行列の射影誤差を $\eta_K$、質量行列の射影誤差を $\eta_M$ と書きます。どちらも、行列入力の `absolute_error_bound` で読み出せる値です。

質量行列の最小固有値の下界を、質量の下界 $\sigma$ と呼びます。`Identity` では、$\sigma = 1$ です。`PositiveDefiniteCholesky` では、Gershgorin の円板から $\sigma$ を求めます。各行で、対角要素から非対角要素の絶対値の和を引きます。全ての行のこの差のうち、最小のものが $\sigma$ です。

`CertifiedEigenpair` は、三つの厳密な値を個別に保持します。

- `computational_bound`: 固有値の推定値から、同じ浮動小数点ペンシルの固有値までの距離の上界。`Dyadic` の厳密な演算で計算する
- `projection_bound`: 浮動小数点ペンシルと厳密なペンシルの距離の上界。射影誤差 $\eta_K$、$\eta_M$ から導く
- `enclosure_radius`: 上の二つの上界の和

`CertifiedFrequency` は、推定値と、各演算を 1 ビットずつ外向きに丸めた区間を返します。`enclosure_radius` の値が正しい限り、この区間は、厳密なペンシルの対称部分の少なくとも一つの固有値に対応する周波数を含みます。

## 入力の来歴を読む

`CertifiedEigenSolution::projection_reference` は入力をそのまま返すため、証明付きの各値は常に元の式まで遡れます。

```text
let reference = solution.projection_reference();
let entry = reference.stiffness().entry(row, column)?;
entry.value();       // ソルバーが使った f64 の値
entry.expression();  // 元の式の識別子 ( ExprId )
reference.stiffness().absolute_error_bound();  // 剛性行列の絶対誤差の上界 ( Dyadic )
```

## クラスタとモード番号

クラスタは `certify_spectral_clusters` が作ります。この関数は、返った固有対の個数が行列の次元に等しい完全なスペクトルだけを受け付け、区間が重なる固有対を、区間どうしが互いに素になるまで併合します。その結果、各クラスタは固有対の個数と同じ数の厳密な固有値をちょうど含み、それらの固有値の昇順での位置が確定します。

- `ModeOrdinal::Certified`: 固有対がクラスタの中で単独であり、モード番号が確定する
- `ModeOrdinal::WithinCluster`: 固有対が他の固有対とクラスタを共有しており、クラスタの番号の範囲だけが分かる
- `SpectralCluster`: 番号の範囲、区間とその半径、隣のクラスタとの間隔、条件数の上界、部分空間の角の上界
- `SpectralClusters`: 一つの解のクラスタと、固有対ごとのモード番号の主張

重複する固有値と、証明付きの誤差の上界より互いに近い固有値は、必ず 2 個以上の固有対を含む一つのクラスタにまとまります。そのため、これらの固有対のモード番号は、クラスタ内の番号の範囲 ( `ModeOrdinal::WithinCluster` ) になります。部分空間の角の上界は、クラスタの固有ベクトルが張る空間と厳密な固有空間との最大主角について、その正弦の上界です。角は、厳密なペンシルの質量内積で測ります。上界は 1 で頭打ちにします。値 1 は、正弦が取りうる最大値で、自明な上界です。

## 別の解との対応

`correspond_to_reference` は、候補の解を参照の解のクラスタと比べる関数です。比較は、固有ベクトルの符号と順序に依らずに行います。モード番号のブロックごとに、二組の固有ベクトルがそれぞれ張る部分空間どうしの距離の上界を求めます。ブロックは、両方の解でクラスタの境界になっている位置で始まって終わる、最も細かい番号の連なりです。

比べる対象は参照の解の厳密なペンシルで、二つの浮動小数点ペンシルの差は射影誤差に加えます。そのため、二つの解が同じペンシルから得られたという前提は必要ありません。

- `ModeCorrespondence`: ブロックの集まり
- `BlockCorrespondence`: ブロックのモード番号、両側のクラスタの添字の範囲、距離の上界

距離の上界は 0 以上 1 以下の値です。この値は、二つの部分空間への射影の差の 2 ノルムを、参照の解の厳密なペンシルの質量内積で測った値の上界です。上界は 1 で頭打ちにします。値 1 は、この 2 ノルムが取りうる最大値で、自明な上界です。

クラスタを作る関数と対応を求める関数は、どちらも失敗を `ClusterError` として返します。

- `IncompleteSpectrum`: 返った固有対の個数が次元と異なる
- `DimensionMismatch`: 参照と候補の次元が異なる
- `GramNotPositive`: ある範囲の固有ベクトルが一次独立であることを、正の下界で示せない
- `MassPerturbationTooLarge`: 質量の差が候補の質量の下界に達した
- `ExactArithmetic`: 厳密な演算で値を表現できない
- `Allocation`: ベクトルまたは行列の確保に失敗した

## 求解の失敗

`CertifiedEigenError` は、求解の失敗を種類ごとに区別します。

- `InvalidProblemShape`: 剛性行列と質量行列の形が合わない
- `InvalidStiffness`、`InvalidMass`: 委譲先のソルバーが返した、行列に由来する失敗
- `SolverInternal`: 委譲先のソルバーが返した、それ以外の失敗
- `RequestedModesExceedDimension`: 要求したモード数が次元を超えた
- `ResidualNotConverged`: 許容値を満たさない残差がある
- `Undeterminable`: 証明付きの上界を得られない。固有対ごとに次の理由を一つ持つ
  - `MassLowerBoundUnavailable`: 質量の下界 $\sigma$ が正の値にならない
  - `ProjectionPerturbationTooLarge`: 質量行列の射影誤差の 2 倍 $2\eta_M$ が、質量の下界 $\sigma$ を超えた
  - `NegativeEigenvalue`: 固有値が負
  - `EnclosureLowerBoundBelowZero`: 区間の下端が負
  - `NonFiniteFrequencyBound`: 周波数の区間の端点が、無限大または NaN に丸まった
- `ExactArithmetic`: 厳密な演算で値を表現できない
- `Allocation`: ベクトルまたは行列の確保に失敗した

## 依存

- `neco-eigensolve`、`neco-eigensolve-faer`: 委譲先の数値ソルバー
- `neco-generalized-eigen`: 問題、固有対、収束状態の型
- `neco-linear-exact`: 証明付きの行列入力と、その射影誤差
- `neco-bigint`: 各上界の計算を支える `Dyadic` の厳密な演算

## ランタイム構成

`neco-eigensolve-faer` に依存するため、標準ライブラリを必要とします。

## ライセンス

MIT License です。
