# neco-linear-exact

[English](README.md)

この crate は厳密な密行列の線形演算を提供します。有理数、正規化した冪根の形式和、実代数的数を扱います。各行列は検証済みの形状と、行優先順で保持する要素を持ちます。

## 公開 API

- `ExactScalar`: 失敗を返すスカラー演算
- `ExactLinearError`: 演算の失敗
- `ExactMatrix<T>`: 行優先の厳密行列
- `ExactLinearSolution<T>`: 連立方程式の解
- `determinant`: 正方行列の行列式
- `rank`: 行列の階数
- `kernel_basis`: 核の基底ベクトル
- `solve`: 連立一次方程式の解
- `project_vector_f64`: 認証付きベクトル射影
- `project_matrix_f64`: 認証付き行列射影

認証付き射影は、厳密なベクトルまたは `ExactMatrix<T>` を入力とし、次の値を保持します。

- `CertifiedVectorProjection`: 方針、各要素の証明値、各要素の絶対誤差上限の総和
- `CertifiedMatrixProjection`: 方針、行優先の各要素の証明値、射影後の `DenseMatrix<f64>`、絶対誤差上限の総和

消去法は左の列から順に調べます。現在のピボット行以降で最初に零でない行を、ピボット行として選びます。この規則から行交換、核の基底、解ベクトルが決まります。

## ランタイム構成

既定の構成は標準ライブラリを使います。既定機能を無効にすると、`alloc` を使う `no_std` の構成になります。

## ライセンス

MIT License です。
