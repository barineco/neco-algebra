# neco-linear-exact

[English](README.md)

この crate は厳密な密行列の線形演算を提供します。有理数、正規化した冪根の形式和、実代数的数を扱います。各行列は検証済みの形状と、行優先順で保持する要素を持ちます。

## 公開 API

- `ExactScalar`: 失敗を返すスカラー演算
- `ExactLinearError`: 演算の失敗
- `ExactMatrix<T>`: 形状と格納長を検証した行優先の厳密行列
- `ExactLinearSolution<T>`: 一意解、核の基底を伴うアフィン解、解なしの三状態
- `determinant`: 正方行列の行列式
- `rank`: 行列の階数
- `kernel_basis`: 核の基底ベクトル
- `solve`: 連立一次方程式の解
- `project_vector_f64`: 認証付きベクトル射影
- `project_matrix_f64`: 認証付き行列射影

`ExactMatrix<T>` は、形状、行数、列数、座標で指定した値、行優先の全要素を観測できます。認証付き射影は、厳密なベクトルまたは行列を入力とし、次の値を保持します。

- `CertifiedVectorProjection`: 射影方針、射影後のベクトル、各要素の証明値、絶対誤差上限の総和
- `CertifiedMatrixProjection`: 射影方針、射影後の行列、行優先の各要素の証明値、絶対誤差上限の総和

ガウス消去法は左の列から順に調べます。現在の pivot 行以降で最初に零でない行を次の pivot 行に選びます。この決定的な選択順から、行交換、核の基底、解ベクトルが決まります。

## ランタイム構成

既定の構成は標準ライブラリを使います。既定機能を無効にすると、`alloc` を使う `no_std` の構成になります。

## ライセンス

MIT License です。
