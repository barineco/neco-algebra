# neco-monomial

[English](README.md)

`neco-monomial` は、冪根を厳密に表すための単項式を提供する crate です。単項式は素数の基底ごとに既約有理数の指数を持ち、`sqrt(12) = 2 sqrt(3)` のような値を近似を挟まずに扱えます。

依存:

- `neco-bigint`

## 機能

- 合成数や重複した基底を許す構成用の入力
- 有限回の試し割りによる、素数基底の正規形への正規化
- 乗算、非零の値による除算、既約有理数の指数による冪
- 有理数の係数と冪根の基底 ( `RadicalBasis` ) への一意な分解
- 無効な零冪をまとめて順序付き集合として報告する検査

位取りの指数を整数から有理数へ拡張した形と捉えられます。整数の指数は有理数を、非整数の指数は冪根を表します。単項式の正規形は符号と昇順の素数指数列で、次のように値を保持します。

$$ \sqrt{12} = 2^{1} \cdot 3^{1/2}, \qquad \sqrt{2} \cdot \sqrt{8} = 2^{1/2} \cdot 2^{3/2} = 2^{2} = 4 $$

`split_radical` は $ 2^{1} \cdot 3^{1/2} $ を、有理数の係数 $ 2 $ と冪根の基底 $ 3^{1/2} $ に分解します。

## 使用例

12 の平方根を正規化し、係数 2 と基底 `sqrt(3)` に分解する例です。

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

- `RawPower`: 非負の構成用基数と、約分前の指数
- `RawMonomial`: 明示的な零、または符号と構成用の因子列
- `NormalizationErrors`: 重複を除いて順序付けた正規化の失敗
- `MonomialErrorKind`: 単項式の演算と正規化が返す失敗
- `ProvenPrime`: 試し割りで素数と確定した値
- `Monomial`: 零、または符号と昇順の素数指数列を持つ正規形
- `RadicalBasis`: `0 < exponent < 1` の素数指数列

正規形のフィールドは private で、所有する値の複製には `try_clone` を使います。

補助的な操作は次のとおりです。

- `NormalizationErrors::from_one`: 一つの失敗から構成する
- `NormalizationErrors::from_errors`: 失敗の列を整列して構成する
- `NormalizationErrors::errors`: 先頭の失敗と追加の失敗を順に参照する
- `NormalizationErrors::into_parts`: 所有する失敗を先頭と追加に分けて取り出す
- `RadicalBasis::try_from_sorted_factors`: 昇順で相異なる素数と、真に 0 と 1 の間にある指数を検証して構築する

## 失敗

`MonomialErrorKind` は次の条件を区別します。

- `DivisionByZero`: 除数が零
- `ZeroToNegativePower`: 零の負指数冪
- `UndefinedZeroPower`: `0^0`
- `EvenRootOfNegative`: 負数の偶数根
- `InvalidRadicalBasis`: 基底列の検証に失敗
- `CapacityOverflow`: 検査可能な容量を超過
- `AllocationFailure { requested_elements }`: 確保に失敗。必要な総要素数を保持
- `Bigint(BigintError)`: 下位 crate の失敗

正規化は、入力に含まれる意味上の失敗を集めて `NormalizationErrors` として返します。容量の超過と確保の拒否は演算をその場で中断し、単一の失敗として返ります。

## ランタイム構成

既定の構成は標準ライブラリを使います。

```text
std
```

既定機能を無効にすると、動的メモリ確保だけを前提とする最小構成になります。

```text
core + alloc
```

値の振る舞いと失敗の種類は、どちらの構成でも同じです。

## ライセンス

MIT License です。
