# neco-bigint

[English](README.md)

`neco-bigint` は、厳密な計算のための任意精度演算を提供する crate です。
自然数・整数・既約有理数に加えて、二進有理数とその区間を扱えます。
記憶領域が増えうる演算は、桁あふれやメモリ確保の失敗を結果の値として呼び出し側へ返します。

## 機能

演算は四つの数の分類ごとに用意されています。

- 自然数: 加算、減算 ( 結果が負になる場合は失敗 )、乗算、シフト、除算、冪、最大公約数、最小公倍数
- 整数: 符号付きの四則演算、ユークリッド除法、冪、拡張ユークリッド互除法
- 有理数: 証拠付きの約分、四則演算、整数冪、床と天井、二進有理数への丸め
- 二進有理数: 正規形での四則演算、有限な `f64` との厳密な相互変換、最近接偶数丸め ( ties-to-even )、包含区間

数値を所有する型はいずれも内部表現を隠しており、検証を通った値だけを構築できます。自然数・整数・既約有理数・二進有理数は常に正規形で保持されます。唯一の例外が `RawRational` で、これは約分する前の入力をそのまま保存しておくための型です。

値の表現を数式で示すと次のとおりです。

- 有理数 $ -\tfrac{6}{8} $ は、約分により分母が正で既約な $ -\tfrac{3}{4} $ として保持されます
- 二進有理数は $ m \cdot 2^{e} $ の形を持ち、$ 0.625 = 5 \cdot 2^{-3} $ のように厳密に表します
- 区間 $ [\tfrac{1}{2}, \tfrac{3}{4}] $ は両端とも二進有理数で、厳密値の観測に使います

メモリ使用量が増えうる演算は、結果を次の形で返します。

```text
Result<_, BigintError>
```

## 使用例

有理数を約分してから、負の整数冪を計算する例です。

```rust
use neco_bigint::{BigInt, BigUint, RawRational};

fn main() -> Result<(), neco_bigint::BigintError> {
    let numerator = BigInt::try_from(-6_i32)?;
    let denominator = BigUint::try_from(8_u32)?;
    let raw = RawRational::new(numerator, denominator);
    let reduction = raw.reduce()?;
    let reduced = reduction.reduced();

    let expected_numerator = BigInt::try_from(-3_i32)?;
    assert_eq!(reduced.numerator(), &expected_numerator);
    let expected_denominator = BigUint::try_from(4_u32)?;
    assert_eq!(reduced.denominator(), &expected_denominator);

    let reciprocal_square = reduced.pow_i32(-2)?;
    let expected_numerator = BigInt::try_from(16_i32)?;
    assert_eq!(reciprocal_square.numerator(), &expected_numerator);
    let expected_denominator = BigUint::try_from(9_u32)?;
    assert_eq!(reciprocal_square.denominator(), &expected_denominator);
    Ok(())
}
```

二進有理数は次の厳密な形で値を持ちます。

```text
integer / 2^exponent
```

有限な `f64` からは損失なく変換できます。
浮動小数点数へ戻す時は最近接偶数丸めを使います。

```rust
use neco_bigint::{Dyadic, DyadicEnclosure};

fn main() -> Result<(), neco_bigint::BigintError> {
    let lower = Dyadic::from_f64_exact(0.5)?;
    let upper = Dyadic::from_f64_exact(0.75)?;
    let enclosure = DyadicEnclosure::new(lower, upper)?;

    let midpoint = enclosure.midpoint()?;
    let rounded = midpoint.round_to_f64_ties_even()?;
    assert_eq!(rounded, 0.625);
    let target = Dyadic::from_f64_exact(0.625)?;
    assert!(enclosure.contains_dyadic(&target));
    Ok(())
}
```

## 公開 API

公開型の役割は次のとおりです。

- `BigUint`: little-endian の `u32` limb 列で表す正規化済みの任意精度自然数
- `BigInt`: 符号と絶対値 ( `BigUint` ) の組で、零をただ一つの形で表す整数
- `Sign`: 整数の符号
  - `Negative`: 負
  - `Zero`: 零
  - `Positive`: 正
- `ExtendedGcd`: 最大公約数と二つの Bézout 係数
- `RawRational`: 約分前の分子と分母
- `RationalReduction`: 約分の入力と gcd、既約な結果
- `ReducedRational`: 分母が正で、分子と分母が互いに素な有理数
- `Dyadic`: 整数を二の冪で割った正規化済みの数
- `DyadicEnclosure`: 端点の順序を検証済みの、両端を含む区間
- `BigintError`: 検査付き演算が返す失敗

組み込み整数からの変換は次のとおりです。

- `BigUint`: 符号なしの組み込み整数から構築
- `BigInt`: 符号付き・符号なしの組み込み整数から構築
- `TryFrom`: 検査付きの構築

任意精度の値は複製にもメモリ確保を伴うため、値を所有する型は失敗を観測できる `try_clone` を提供します。

有理数の約分は次の二つで扱います。

- `RawRational::reduce`: 約分を実行する
- `RationalReduction`: 約分の入力と gcd を、既約な結果とともに保存する

## 失敗

`BigintError` は十種類の失敗を区別します。

- `CapacityOverflow`: limb 数、bit 数、シフト量、反復回数が検査可能な容量を超過
- `AllocationFailure { requested_limbs }`: 格納領域の確保に失敗。必要な総 limb 数を保持
- `UnsignedUnderflow`: 自然数の減算で結果が負
- `DivisionByZero`: 除数が零
- `NonExactDivision`: 厳密除算で余りが非零
- `ZeroDenominator`: 約分対象の分母が零
- `NonFiniteFloat`: 二進有理数への変換入力が無限大または NaN
- `FloatOutOfRange`: 絶対値が有限な `f64` の最大値を超過
- `InvalidInterval`: 区間の両端が逆順
- `ExponentOverflow`: 二進有理数の指数が上限を超過
  - `required`: 演算に必要な指数
  - `maximum`: 対応できる最大の `u32` 指数

メモリ使用量が増える演算は、まず必要な総 limb 数を検査付きの算術で見積もり、`BigUint::MAX_LIMBS` の上限を確かめてから、失敗を観測できる方法で領域を確保します。容量の超過も確保の失敗も、どちらの構成でも通常の `Result` として返ります。

## ランタイム構成

既定の構成は標準ライブラリを使います。

```text
std
```

既定機能を無効にすると、`alloc` を前提とする `no_std` 構成になります。

```text
core + alloc
```

```toml
[dependencies]
neco-bigint = { version = "0.1", default-features = false }
```

`std` 構成では、公開エラー型が `std::error::Error` を実装します。数値の振る舞いと失敗の種類は、どちらの構成でも同じです。

## ライセンス

MIT License です。
