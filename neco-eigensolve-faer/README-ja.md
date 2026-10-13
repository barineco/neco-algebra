# neco-eigensolve-faer

[English](README.md)

`neco-eigensolve-faer` は、`faer` を使って固有値問題と複素の連立一次方程式を解き、複素行列の条件数を計算するアダプターの crate です。

- 正定値の質量行列を持つ実対称一般化固有値問題の求解
- 実の非対称固有値問題の求解
- 複素の密な連立一次方程式の求解
- 複素行列の 2 ノルムの条件数の計算

引数と結果の型は、このリポジトリの線形代数と固有値問題の crate が定めます。この crate に固有の失敗は、`EigensolveFaerError` の値で返ります。

## API

```text
solve_symmetric_f64(&GeneralizedEigenProblem, EigensolveConfig)
  -> Result<EigensolveResult, EigensolveFaerError>

solve_request_symmetric_f64(EigensolveRequest<R>)
  -> Result<EigensolveResult<R>, EigensolveFaerError>
```

```text
approximate_mass_solve(&DenseMatrix<f64>, &Vector<f64>)
  -> Result<Vector<f64>, EigensolveFaerError>
```

```text
solve_nonsymmetric_f64(&DenseMatrix<f64>)
  -> Result<NonsymmetricEigenDecomposition, EigensolveFaerError>

NonsymmetricEigenDecomposition::eigenvalues(&self) -> &[Complex<f64>]
NonsymmetricEigenDecomposition::eigenvectors(&self) -> &DenseMatrix<Complex<f64>>

ComplexLu::factor(&DenseMatrix<Complex<f64>>)
  -> Result<ComplexLu, EigensolveFaerError>

ComplexLu::solve(&self, &DenseMatrix<Complex<f64>>)
  -> Result<DenseMatrix<Complex<f64>>, EigensolveFaerError>

condition_number_2(&DenseMatrix<Complex<f64>>)
  -> Result<f64, EigensolveFaerError>
```

### 計算結果

- `EigensolveResult<R>`: 質量内積で正規化した固有ベクトル、入力の問題から計算した残差、零のスペクトルシフト、要求から移した射影参照 ( 呼び出し側が型 `R` で渡す任意の値 )
- `EigensolveFaerError`: 行列、外部ソルバー、下位 crate の失敗
- `approximate_mass_solve`: 質量行列 $M$ を係数行列とする連立一次方程式 $M z = b$ の近似解。ソルバーと同じ Cholesky 分解の経路で求める。呼び出し側は、この近似解を、独立に求めた前進誤差の証明付きの上界を補正する入力としてだけ使い、上界そのものとしては使わない

入力行列の条件は、剛性行列と質量行列で異なります。

- 剛性行列: 全要素が有限値で対称。条件に反する入力は `InvalidStiffnessMatrix` として返る
- 質量行列: 全要素が有限値で対称かつ正定値。非有限の要素を含む入力と、対称でない入力は `InvalidMassMatrix`、Cholesky 分解の対角が 0 以下になる入力は `NonPositiveMassMatrix` として返る

厳密に重複する固有値は、完全な固有空間として返ります。要求を受け取る API は、射影参照をそのまま計算結果へ移します。

### 非対称固有値、複素 LU 分解、条件数

- 固有値: すべての固有値を複素数で返す
- 固有ベクトル: 右固有ベクトルを、列優先の複素行列の列として返す
- 正規化: 2 ノルムを 1 にし、絶対値が最大値の $(1 - 10^{-12})$ 倍以上の成分のうち、添字が最小の成分を正の実数にする
- 共役の対: 虚部が負の固有値の固有ベクトルは、虚部が正の固有値の固有ベクトルの厳密な共役である
- 順序: 虚部の降順に並べ、虚部が等しい場合は実部の降順に並べる。両方が等しい固有値は、下位の解法が返した順を保つ

結果は固有値と固有ベクトルだけを持ちます。すべての固有対の残差は後述の契約で上から抑えられ、呼び出し側は自分の行列から残差を再計算できます。

複素行列の LU 分解は `ComplexLu::factor` で計算し、連立一次方程式は、分解の結果の `solve` で解きます。
条件数は `condition_number_2` で計算します。

- 分解: 部分ピボット選択付きの LU 分解を値として保持し、任意の数の右辺に使える
- 求解: n × m の右辺に対して n × m の解を返す
- ピボットの確認: 分解の後に U の対角要素を調べ、0 または非有限の要素のうち添字が最小のものを、演算の名前、添字、ピボットの大きさとともに失敗として返す
- 列の数が 0 の右辺: 列の数が 0 の解を返す
- 条件数: 正方で要素が有限の、零でない行列について、最大の特異値と最小の特異値の比を返す。最小の特異値が 0 の場合は正の無限大を返す
- 許容の判定: 条件数が許容範囲にあるかは、呼び出し側が判定する

これらの操作の下位の演算はすべて逐次で実行するため、結果は faer の並列処理の大域設定に依存しません。
作業領域、結果の領域、並べ替えの領域の確保に失敗した場合は、その失敗を値として返します。
同じ機械と同じビルドでは、同じ入力に対して同じビット列の結果を返します。この保証の範囲は、その機械とビルドだけです。

固有分解と条件数の計算では、入力の行列を、要素の絶対値の最大値以下で最大の 2 の冪で割ってから下位の解法を呼びます。
固有値は元の尺度へ戻し、条件数は比であるため値が保たれます。固有ベクトルは、尺度を変えた行列から求めた値を正規化して返します。
固有分解の精度は、最大の要素の尺度に対する後退誤差で決まります。尺度を変えた後の行列では、最大の要素の $2^{-1022}$ 倍程度より小さい要素が、非正規数または 0 になります。
LU 分解では、元の尺度のまま下位の解法を呼びます。

**後退誤差の大きい固有対の作り直し**

- 契約: 返すすべての固有対について、後退誤差を 1e-12 以下にする。後退誤差は、残差のノルムを、行列のフロベニウスノルムと固有ベクトルのノルムの積で割った値である
- 検査: 下位の分解の後に、すべての固有対の後退誤差を求める
- 作り直しの条件: 後退誤差が機械イプシロンの 16 倍を超える複素の固有対を、相対距離 1e-6 以内の固有値とまとめ、Hessenberg 形に対するシフト逆反復 ( shift-and-invert ) の部分空間反復と Rayleigh-Ritz 法で作り直す
- 上限: 群ごとの反復は 8 回まで。作り直した群の固有ベクトルには、2 ノルムの条件数が 1e8 以下であることを求める
- 置き換え: 作り直した固有ベクトルは、後退誤差が元より小さい場合に限り、元の固有ベクトルを置き換える
- 実軸に近い群: 群に属する固有値の平均の虚部が、その平均の絶対値の 1e-6 倍以下の群は、作り直しの前に失敗として返す
- 出力の保存: 作り直す固有対が無い行列では、上記の正規化と順序の規則を適用した下位の分解の出力と、ビット単位で一致する結果を返す

失敗の種類は、`EigensolveFaerError` の列挙子で区別します。

```text
InvalidMatrix { operation, reason }
  正方でない、空、または非有限の値を含む入力。条件数の計算に渡した零行列。
  operation は失敗した関数の名前。
SingularPivot { operation, index, magnitude }
  分解で現れた 0 または非有限のピボットと、その大きさ。
RightHandSide { operation, expected, actual }
  右辺の行の数が、分解した行列の次元と異なる。
Linear(AllocationFailure | CapacityOverflow)
  作業領域、結果の領域、並べ替えの領域の確保に失敗した。
EigenvectorRefinement { operation, index, residual, kind }
  ResidualExceeded: 作り直しの後も、後退誤差が 1e-12 を超える固有対。
  RealEigenvalue: 後退誤差が機械イプシロンの 16 倍を超える実の固有値。
  LinearlyDependent: 作り直した後の固有ベクトルの条件数が 1e8 を超えた群、または部分空間反復の直交化で一次従属を検出した群。
  NearRealAxis: 実軸に近い群。作り直しの前に返す。
  SubproblemFailed: 小さな固有値問題または特異値問題の収束に失敗した。
  index は返す順での位置。
NonFiniteResult { operation }
  固有値、固有ベクトル、または解が非有限の値を持つ。
ExternalSolver { operation, reason }
  固有分解または特異値分解の収束に失敗した。または、零の固有ベクトルか、共役の相手を欠く複素の固有値を返した。
```

過剰減衰や安定性といった物理的な解釈は、呼び出し側が行います。

### すべての固有ベクトルを求める三重対角 QR 法

このアダプターは、公開関数 `faer::linalg::evd::self_adjoint_evd` を一回呼び、すべての固有値と、対応するすべての固有ベクトルの列を計算します。
`recursion_threshold` には、行列の次元を n として、桁あふれを検査した加算で求めた n + 1 を指定し、固有ベクトルを伴う三重対角 QR 法を選びます。
この呼び出しは、faer の並列処理の大域設定に従って実行します。
質量行列が厳密な単位行列の場合も一般の正定値行列の場合も、固有値分解の呼び出しは、この一回だけです。

質量行列が一般の正定値行列の場合は、Cholesky 分解による標準問題への変換、逆三角行列による列の変換、全要素を使う質量内積を用います。
質量行列が厳密な単位行列の場合は、剛性行列を直接使い、単位行列の逆行列の格納と乗算を省いて、有限な未正規化の列を得ます。
単位質量の場合は、正規化の前に、零でない有限の成分のビット列を保ち、正負の零だけを正の零に置き換えます。
正規化、絶対値が最大の成分の符号の選択、公開する残差と固有空間の検証には、引き続き元の入力問題を使います。

分解後の収束状態は、公開する残差の許容条件で決まります。この crate は数値解と残差を返し、前進誤差の証明付きの上界は `neco-eigensolve-certified` が求めます。
`EigensolveConfig` の `max_iterations` には正の値を指定します。ただし、QR 法は独自の内部の反復上限を使います。
返す反復数の 1 はアダプターが行った一回の分解を表し、QR 法の内部の反復数やその上限とは別の値です。

固有値と固有ベクトルの格納領域は、要素数が `usize` で表せ、バイト数が `isize::MAX` 以下であることを確かめてから、確保の失敗を値として受け取れる方法で確保します。
格納領域と作業領域のどちらでも、容量の超過は `Linear(CapacityOverflow)`、確保の拒否は `Linear(AllocationFailure)` として返ります。作業領域の確保で要求する数は、バイト数です。
空の行列に対しては、作業領域を要求する前に、空の固有値の列と、空の固有ベクトルの列を返します。分解が返すその他の失敗は、`ExternalSolver` として返します。
これらの保証は、固有値と固有ベクトルの格納領域と、作業領域に適用されます。入力の変換と一般の質量の変換では、`faer` の行列を確保します。
この確保の失敗と、外部のライブラリの中で起きる panic は、Rust の標準の処理 ( panic または異常終了 ) で演算を止めます。

## ランタイム構成

このアダプターは `faer` の標準ライブラリ機能を使うため、標準ライブラリを必要とします。

## ライセンス

MIT License です。
