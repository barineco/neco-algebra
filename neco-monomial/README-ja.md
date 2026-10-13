# neco-monomial

[English](README.md)

`neco-monomial` は、冪根を厳密に表すための単項式を提供する crate です。単項式は素数の底ごとに既約有理数の指数を持ち、$ \sqrt{12} = 2\sqrt{3} $ のような値を、近似を挟まずに扱えます。

直接依存する crate は `neco-bigint` だけです。

## 提供する演算

- 合成数や重複した底を許す、構成用の入力
- 有限回の試し割りによる、素数の底を持つ正規形への正規化
- 乗算、零でない値による除算、既約有理数の指数による冪
- 有理数の係数と冪根の基底 ( `RadicalBasis` ) への一意な分解
- 指数の分母が零の冪と、無効な零の冪 ( `0^0`、零の負冪 ) を、まとめて順序付きの集合として報告する検査

この単項式は、整数の素因数分解で各素数に付く指数を、整数から有理数へ拡張した形と捉えられます。整数の指数は有理数を、整数でない指数は冪根を表します。単項式の正規形は、符号と、素数の昇順に並べた指数の列からなり、次のように値を保持します。

$$ \sqrt{12} = 2^{1} \cdot 3^{1/2}, \qquad \sqrt{2} \cdot \sqrt{8} = 2^{1/2} \cdot 2^{3/2} = 2^{2} = 4 $$

`split_radical` は $ 2^{1} \cdot 3^{1/2} $ を、有理数の係数 $ 2 $ と冪根の基底 $ 3^{1/2} $ に分解します。

## 使用例

12 の平方根を正規化し、係数 2 と基底 $ \sqrt{3} $ に分解する例です。

```rust
use neco_bigint::{BigInt, BigUint, RawRational};
use neco_monomial::{RawMonomial, RawPower};

fn main() {
    let exponent = RawRational::new(
        BigInt::try_from(1_i32).unwrap(),
        BigUint::try_from(2_u32).unwrap(),
    );
    let raw = RawMonomial::positive(vec![RawPower::new(
        BigUint::try_from(12_u32).unwrap(),
        exponent,
    )]);
    let value = raw.normalize().unwrap();
    let (coefficient, basis) = value.split_radical().unwrap();
    let expected = BigInt::try_from(2_i32).unwrap();
    assert_eq!(coefficient.numerator(), &expected);
    let prime = &basis.factors()[0].0;
    assert_eq!(prime.value().to_u32(), Some(3));
}
```

## 公開型

- `RawPower`: 構成用の非負の底と、約分する前の指数
- `RawMonomial`: 明示的な零、または符号と構成用の因子の列
- `NormalizationErrors`: 重複を除いて順序付けた、正規化の失敗の集合
- `MonomialErrorKind`: 単項式の演算と正規化が返す失敗
- `ProvenPrime`: 試し割りで素数と確かめた値
- `Monomial`: 零、または符号と昇順の素数の指数の列を持つ正規形
- `RadicalBasis`: 指数が `0 < exponent < 1` を満たす、素数の指数の列

正規形のフィールドは private で、所有する値の複製には `try_clone` を使います。
`NormalizationErrors` は、失敗から値を構成する操作に加えて、所有する失敗を参照する操作と、所有する失敗を取り出す操作を持ちます。
`RadicalBasis` の値は、因子の列を検証してから構築します。

- `NormalizationErrors::from_one`: 一つの失敗から構成する
- `NormalizationErrors::from_errors`: 失敗の列を整列して構成する
- `NormalizationErrors::errors`: 先頭の失敗と追加の失敗を順に参照する
- `NormalizationErrors::into_parts`: 所有する失敗を、先頭と追加に分けて取り出す
- `RadicalBasis::try_from_sorted_factors`: 昇順で相異なる素数と、0 と 1 の間に真に含まれる指数を検証して構築する

## 失敗

`MonomialErrorKind` は、失敗の条件を種類ごとに区別します。

- `DivisionByZero`: 除数が零
- `ZeroToNegativePower`: 零を負の指数で冪乗した
- `UndefinedZeroPower`: `0^0` を求めた
- `EvenRootOfNegative`: 負数の偶数次の根を求めた
- `InvalidRadicalBasis`: 基底の列の検証に失敗した
- `CapacityOverflow`: 演算の前に検査する容量の上限を超えた
- `AllocationFailure { requested_elements }`: 確保に失敗した。必要な総要素数を保持する
- `Bigint(BigintError)`: 下位 crate の失敗

入力に、指数の分母が零の冪、`0^0`、零の負冪が含まれる場合、正規化はそれぞれの失敗を集め、重複を除いて順序付けた `NormalizationErrors` として返します。容量の超過と確保の拒否は演算をその場で中断し、単一の失敗として返ります。

## ランタイム構成

既定では標準ライブラリを使います。既定機能を無効にすると、動的メモリ確保だけを前提とする `core + alloc` の構成になります。値の振る舞いと失敗の種類は、どちらの構成でも同じです。

## ライセンス

MIT License です。
