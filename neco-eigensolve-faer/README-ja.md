# neco-eigensolve-faer

[英語版](README.md)

この crate は、正定値の質量行列を持つ実対称一般化固有値問題を解きます。公開する問題・構成・要求・計算結果には線形代数 crate 群の型を使い、この crate 固有の失敗として `EigensolveFaerError` を定義します。

## API

```text
solve_symmetric_f64(GeneralizedEigenProblem, EigensolveConfig)
  -> Result<EigensolveResult, EigensolveFaerError>

solve_request_symmetric_f64(EigensolveRequest<R>)
  -> Result<EigensolveResult<R>, EigensolveFaerError>
```

### 計算結果

- `EigensolveResult<R>`: 質量内積で正規化した固有ベクトル、残差、射影参照
- `EigensolveFaerError`: 行列と計算の失敗

剛性行列と質量行列は、全要素が有限値で対称である必要があります。質量行列は正定値である必要があります。

## ランタイム構成

このアダプターは `faer` の標準ライブラリ機能を利用するため、標準ライブラリを必要とします。

## ライセンス

MIT License です。
