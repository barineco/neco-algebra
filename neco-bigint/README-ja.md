# neco-bigint

[English](README.md)

`neco-bigint` は、厳密な計算に使う任意精度演算を提供する crate です。自然数・整数・既約有理数に加えて、二進有理数と検証済みの包含区間を扱えます。記憶領域が増えうる演算は、容量超過やメモリ確保失敗を `Result` の値として呼び出し側へ返します。

## 機能

演算は、次の四種類の数に分かれています。

- 自然数: 加算、減算、乗算、シフト、除算、冪、最大公約数、最小公倍数
- 整数: 符号付きの四則演算、ユークリッド除法、冪、拡張ユークリッド互除法
- 有理数: 証明値を伴う約分、四則演算、整数冪、床と天井、二進有理数への丸め
- 二進有理数: 正規形での四則演算、有限な浮動小数点値との厳密な相互変換、最近接偶数丸め、包含区間

### 検証済みの値

自然数・整数・既約有理数・二進有理数は内部表現を隠し、検証済みの正規形だけを構築できます。有理数の構成では、約分前の入力と検証済みの結果を次の型で分けます。

- `RawRational`: 零分母を含む、約分前の分子と分母を保持
- `RawRational::reduce`: 零分母を拒否し、成功時には正規形の有理数と約分の証明値を生成
- `BigintError::ZeroDenominator`: 約分時の零分母を報告

### 値の保持形式

保持形式は次のとおりです。

- 有理数: `-6/8` を、正の分母と互いに素な分子・分母を持つ `-3/4` へ約分
- 二進有理数: `m * 2^e` の形で保持し、`0.625 = 5 * 2^(-3)` などを厳密に表現
- 包含区間: `[1/2, 3/4]` のように、順序を検証した二進有理数の端点を保持

### 失敗の返し方

記憶領域が増えうる演算は、結果を次の型で返します。

```text
Result<_, BigintError>
```

## 使用例

有理数を約分してから、負の整数冪を計算します。

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

二進有理数は、整数と非負の二進指数を保持します。有限な浮動小数点値からは損失なく変換でき、浮動小数点値へ戻す時には最近接偶数丸めを使います。

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

### 公開型

公開型の役割は次のとおりです。

- `BigUint`: リトルエンディアンの `u32` の桁列で表す、正規化済みの任意精度自然数
- `BigInt`: 符号と絶対値の組で零を一つの形に表す整数
- `Sign`: 整数の符号
  - `Negative`: 負
  - `Zero`: 零
  - `Positive`: 正
- `ExtendedGcd`: 最大公約数と二つの Bézout 係数
- `RawRational`: 約分前の分子と分母
- `RationalReduction`: 約分の入力、最大公約数、既約な結果
- `ReducedRational`: 分母が正で、分子と分母が互いに素な有理数
- `Dyadic`: 整数を二の冪で割った正規化済みの数
- `DyadicEnclosure`: 端点の順序を検証した、両端を含む区間
- `BigintError`: 検査付き演算が返す失敗

### 組み込み整数からの変換

変換には次の経路を使います。

- `BigUint`: 符号なしの組み込み整数から構築
- `BigInt`: 符号付き・符号なしの組み込み整数から構築
- `TryFrom`: 検査付きの構築

### 複製

任意精度の値は複製にもメモリ確保を伴います。値を所有する各型は、メモリ確保失敗を返す `try_clone` を提供します。

### 有理数の約分

約分には次の公開入口があります。

- `RawRational::reduce`: 検証と約分を実行
- `RationalReduction`: 約分の入力と最大公約数を、既約な結果とともに保存

## 失敗

`BigintError` は十種類の失敗を区別します。

- `CapacityOverflow`: 桁数、ビット数、シフト量、反復回数が、検査で定めた容量上限を超過
- `AllocationFailure { requested_limbs }`: 格納領域の確保に失敗し、必要な総桁数を保持
- `UnsignedUnderflow`: 自然数の減算で結果が負
- `DivisionByZero`: 除数が零
- `NonExactDivision`: 厳密除算で余りが非零
- `ZeroDenominator`: 約分対象の分母が零
- `NonFiniteFloat`: 二進有理数への変換入力が無限大または NaN
- `FloatOutOfRange`: 絶対値が有限な `f64` の最大値を超過
- `InvalidInterval`: 包含区間の両端が逆順
- `ExponentOverflow`: 二進有理数の指数が上限を超過
  - `required`: 演算に必要な指数
  - `maximum`: 対応できる最大の `u32` 指数

記憶領域が増える演算は、必要な総桁数を算術演算で検査して見積もり、`BigUint::MAX_LIMBS` の上限を確認します。その後、失敗を返せる経路で記憶領域を確保します。容量超過とメモリ確保失敗は、どちらの構成でも通常の `Result` として返ります。

## ランタイム構成

### 標準ライブラリ

既定では標準ライブラリを使います。

```text
std
```

### 既定機能を無効にした構成

`alloc` を前提とする `no_std` 構成になります。

```text
core + alloc
```

```toml
[dependencies]
neco-bigint = { version = "0.1", default-features = false }
```

### 公開エラー型

`std` を使う構成では、公開エラー型が `std::error::Error` を実装します。数値の振る舞いと失敗の種類は、どちらの構成でも同じです。

## ライセンス

MIT License です。
