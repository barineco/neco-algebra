# neco-bigint

[English](README.md)

`neco-bigint` は、厳密な計算に使う任意精度演算を提供する crate です。
自然数・整数・既約有理数に加えて、二進有理数 ( dyadic rational ) と、その検証済みの包含区間 ( enclosure ) を扱えます。
メモリ使用量が増えうる演算は、容量の超過やメモリ確保の失敗を `Result` の値として呼び出し側へ返します。

## 提供する演算

演算は四つの数の分類ごとに用意されています。

- 自然数: 加算、減算、乗算、シフト、除算、冪、最大公約数、最小公倍数
- 整数: 符号付きの四則演算、ユークリッド除法、冪、拡張ユークリッド互除法
- 有理数: 証拠付きの約分、四則演算、整数冪、床と天井、二進有理数への丸め
- 二進有理数: 正規形での四則演算、有限な浮動小数点値との厳密な相互変換、最近接偶数丸め ( ties-to-even )、包含区間

### 検証済みの値

自然数・整数・既約有理数・二進有理数の型は内部表現を隠しており、検証を通った正規形の値だけを構築できます。有理数については、約分する前の入力と検証済みの結果を、別の型と操作で表します。

- `RawRational`: 約分する前の分子と分母を保持する。分母が零の値も保持できる
- `RawRational::reduce`: 分母が零なら拒否し、成功すると `RationalReduction` を生成する。この値は、正規形の有理数を持ち、約分が正しいことの証拠として入力と最大公約数も持つ
- `BigintError::ZeroDenominator`: 約分の際に分母が零だったことを報告する

### 値の保持形式

- 有理数: $ -\tfrac{6}{8} $ は、分母が正で分子と分母が互いに素な $ -\tfrac{3}{4} $ へ約分して保持する
- 二進有理数: 整数 $ m $ と非負の指数 $ k $ で $ m / 2^{k} $ の形に保持し、$ 0.625 = 5 / 2^{3} $ のような値を厳密に表す
- 包含区間: $ [\tfrac{1}{2}, \tfrac{3}{4}] $ のように、順序を検証した二進有理数の端点を保持する

### 失敗の返し方

メモリ使用量が増えうる演算は、結果を次の型で返します。

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

二進有理数は、整数 $ m $ と、分母 $ 2^{k} $ の非負の指数 $ k $ を保持し、値 $ m / 2^{k} $ を表します。有限な浮動小数点値からは損失なく変換でき、浮動小数点値へ戻す時には最近接偶数丸めを使います。

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

- `BigUint`: リトルエンディアンの `u32` の桁列で表す、正規化済みの任意精度自然数
- `BigInt`: 符号と絶対値の組で表し、零の表現が一つに定まる整数
- `Sign`: 整数の符号
  - `Negative`: 負
  - `Zero`: 零
  - `Positive`: 正
- `ExtendedGcd`: 最大公約数と二つの Bézout 係数
- `RawRational`: 約分前の分子と分母
- `RationalReduction`: 約分の入力、最大公約数、既約な結果
- `ReducedRational`: 分母が正で、分子と分母が互いに素な有理数
- `Dyadic`: 整数を 2 の冪で割った形の、正規化済みの数
- `DyadicEnclosure`: 端点の順序を検証した、両端を含む区間
- `BigintError`: 検査付き演算が返す失敗

### 組み込み整数からの変換

- `BigUint`: 符号なしの組み込み整数から構築する
- `BigInt`: 符号付きと符号なしの組み込み整数から構築する
- `TryFrom`: 検査付きで構築する

### 複製

任意精度の値は、複製する際にもメモリを確保します。そのため、値を所有する各型は、確保の失敗を返す `try_clone` を提供します。

### 有理数の約分

- `RawRational::reduce`: 入力を検証して約分する
- `RationalReduction`: 約分の入力と最大公約数を、既約な結果とともに保持する

## 失敗

`BigintError` は十種類の失敗を区別します。

- `CapacityOverflow`: 桁数、ビット数、シフト量、反復回数が、演算の前に検査する容量の上限を超えた
- `AllocationFailure { requested_limbs }`: 格納領域の確保に失敗した。必要な総桁数を保持する
- `UnsignedUnderflow`: 自然数の減算で結果が負になる
- `DivisionByZero`: 除数が零
- `NonExactDivision`: 厳密な除算で余りが零でない
- `ZeroDenominator`: 約分する有理数の分母が零
- `NonFiniteFloat`: 二進有理数へ変換する入力が無限大または NaN
- `FloatOutOfRange`: 二進有理数の絶対値が、有限な `f64` の最大値を超えた
- `InvalidInterval`: 包含区間の両端が逆順
- `ExponentOverflow`: 二進有理数の指数が上限を超えた
  - `required`: 演算に必要な指数
  - `maximum`: 対応できる最大の `u32` 指数

メモリ使用量が増える演算は、必要な総桁数を、桁あふれを検査しながら見積もり、上限 `BigUint::MAX_LIMBS` を超えないことを確かめます。その後、確保の失敗を値として受け取れる方法で領域を確保します。容量の超過とメモリ確保の失敗は、標準ライブラリを使う構成と使わない構成のどちらでも、通常の `Result` の値として返ります。

## ランタイム構成

既定では標準ライブラリを使います。既定機能を無効にすると、動的メモリ確保だけを前提とする `core + alloc` の構成になります。

```toml
[dependencies]
neco-bigint = { version = "0.1", default-features = false }
```

標準ライブラリを使う構成では、`BigintError` が `std::error::Error` を実装します。数値の振る舞いと失敗の種類は、どちらの構成でも同じです。

## ライセンス

MIT License です。
