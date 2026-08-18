# neco-formsum

[English](README.md)

`neco-formsum` は、正規化した冪根単項式の有理係数和 ( 形式和 ) を厳密に扱う crate です。計算途中の値を記号のまま保持し、同値の判定は疎な正規形の構造照合で行います。符号の判定には、厳密値を含むことが証明された二進有理数の区間を使います。

## 入力と正規形

構成用の入力と正規形の役割は次のとおりです。

- `RawTerm`: 約分前の有理係数と構成用の単項式
- `RawFormSum::normalize`: 全入力の検証と正規化
- `FormSum`: 辞書順で相異なる基底と非零係数を持つ正規形

正規化は、単項式の有理数因子を係数へ移し、同じ `RadicalBasis` を持つ項をまとめ、係数が零になった項を取り除きます。正規化後に項が一つも残らない形式和は零を表します。たとえば次の値は、基底 $ 1, 2^{1/2}, 3^{1/2} $ への係数 $ 1, 2, -1 $ を持つ三項の正規形です。

$$ 1 + 2\sqrt{2} - \sqrt{3} $$

正規化の結果は入力の順序に依存しません。入力に意味上の失敗が含まれる場合は、重複を除いて昇順に並べた `NormalizationErrors<FormSumErrorKind>` が返ります。演算を続けられない失敗はその場で処理を止め、一件の失敗として返ります。

## 厳密演算

形式和は加算・減算・乗算・除算・逆元を提供し、どの結果も正規形で返します。除算と逆元は、有限な冪根拡大の上で厳密な有理連立方程式を解いて求めます。

- `FormSum`: 厳密演算の入力と結果
- `FormSumErrorKind::DivisionByZero`: 零による除算

```rust
use neco_formsum::FormSum;

fn main() -> Result<(), neco_formsum::FormSumErrorKind> {
    let one = FormSum::one()?;
    let two = one.add(&one)?;
    let quotient = two.div(&one)?;

    assert_eq!(quotient, two);
    assert!(FormSum::zero().is_zero());
    Ok(())
}
```

## 有限な冪根拡大

`extension_with` は、二つの値をともに含む最小の共通拡大を構成します。たとえば $ \sqrt{2} $ と $ \sqrt{3} $ を含む最小の拡大は $ \mathbb{Q}(\sqrt{2}, \sqrt{3}) $ で、次元 $ D = 4 $ の基底を持ちます。`RadicalExtension` は昇順の素数列、指数の分母列、基底の数を公開します。基底の並びは、最後の座標が最も速く変わる混合基数 ( mixed-radix ) 順です。

座標に関する公開操作は次のとおりです。

- `coordinates_with`: 必要な素数と分母を含む拡大の上で座標化する
- `RadicalCoordinates`: 形式和への復元と、列優先の乗算行列を提供する

行列の添字は次の式で求めます。

```text
row + D * column
```

この要素は、列で指定した基底元を値へ乗じた結果の、行で指定した基底元の係数を表します。

`annihilating_coefficients` は、原始的な整数係数の消去多項式を返します。係数は次数の低い順に並び、最高次の係数は正です。

## 区間と符号

- `enclose(bits)`: 厳密値を含み、幅が `2^-bits` 以下の区間を返す
- `sign`: 構造的な零の確認、または零を含まない区間から符号を得る

冪根の区間は、整数冪の不等式だけを根拠に構成します。

## 失敗とメモリ確保

失敗に関わる公開型は次のとおりです。

- `FormSumErrorKind`: 下位 crate の失敗、零除算、次元の超過、確保の拒否
- `DimensionResource`: 指数の分母、基底の数、行列の要素数の区別

可変長の格納領域を所有する型は `try_clone` を提供し、複製時のメモリ確保の失敗を `Result` で観測できます。

## 機能

既定の `std` 機能は、標準エラー型との連携と、依存 crate の同名の機能を有効にします。既定機能を無効にすると、同じ値と同じ失敗を扱う `core + alloc` 構成になります。

```console
cargo check -p neco-formsum --no-default-features
```

実行時の依存:

- `neco-bigint`
- `neco-monomial`

## ライセンス

MIT License です。
