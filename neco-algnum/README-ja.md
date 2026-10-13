# neco-algnum

[English](README.md)

`neco-algnum` は、実代数的数を厳密に扱う crate です。値は、整数係数の最小多項式と、実根の番号 ( 以下、根番号 ) の組で同定します。最小多項式は、原始的かつ既約で最高次係数が正という正規形を持ち、根番号は 0 から数えます。たとえば $ \sqrt{2} + \sqrt{3} $ は次の組で表されます。

$$ m(x) = x^{4} - 10x^{2} + 1, \qquad k = 3 $$

$ m $ の実根は $ \pm\sqrt{3} \pm \sqrt{2} $ の四つで、それらを昇順に数えた番号 $ k = 3 $ が最大の根 $ \sqrt{2} + \sqrt{3} $ を指します。二進有理数を端点とする証明付きの区間は、値の構成と観測に使い、値そのものの同一性は最小多項式と根番号が保ちます。

## 多項式と検証済みの型

整数係数と有理係数の多項式は、係数を次数の低い順に格納します。構成の途中の値と検証済みの値は、型で分かれています。

- `Polynomial`: 末尾の零を除いた整数係数多項式
- `RationalPolynomial`: 末尾の零を除いた有理係数多項式
- `CandidatePolynomial`: 次数が 1 以上の候補多項式
- `SquareFreePolynomial`: 原始的で最高次係数が正の、平方因子を持たない多項式
- `IrreduciblePolynomial`: 有理数体の上で既約な多項式
- `MinimalPolynomial`: 実代数的数を同定する既約多項式
- `PolynomialQuotient`: 剰余環 `Q[x] / (m)` での演算
- `GeneratorRepresentative`: 同じ法に対する生成元 `x mod m`
- `RootIndex`: 同じ既約多項式の実根を昇順に数えた番号
- `RealAlgebraic`: 最小多項式と根番号からなる厳密値
- `IsolatingInterval`: 厳密値を含む、観測に使う二進有理数の区間
- `CertifiedAlgebraic`: 根の分離で得た厳密値と証明付きの区間の組

`Polynomial` は加減乗算、微分、整数と有理数の点での評価、合成を提供します。`CandidatePolynomial::square_free` は、多項式の内容 ( 係数の最大公約数 ) と重複因子を取り除きます。因数分解では、Kronecker 法で因子の候補をすべて列挙します。
列挙したどの候補でも割り切れない多項式を、既約と判定します。

### 多項式商

`PolynomialQuotient` は、演算の結果を最小多項式を法として剰余化し、法より次数の低い `RationalPolynomial` を返します。

- `reduce`: 一つの多項式を剰余化する
- `add`: 加算の結果を剰余化する
- `sub`: 減算の結果を剰余化する
- `mul`: 乗算の結果を剰余化する

`generator` は、同じ法で剰余化した生成元を返します。
また、`RationalPolynomial::to_real_algebraic_coefficients` は有理係数を実代数的数へ変換し、結果を `RationalCoefficientConversion` として返します。

## 証明付きの実根

Sturm 列を用いて、すべての実根を、異なる根をちょうど一つずつ含む区間へ分離し、同じ既約多項式の実根に、値の昇順で根番号を与えます。隣り合う区間が端点を共有する場合もあります。
根は、すべての実根の分離か、利用者が与えた区間の検証によって構成し、その結果から厳密値を取り出します。

- `isolate_real_roots`: すべての実根を、証明付きの区間とともに返す
- `certify_root`: 利用者が与えた二つの二進有理数の端点を検証する
- `into_value`: 証明付きの構成結果から厳密値を取り出す

### 区間の精密化

`RealAlgebraic::enclose` と `IsolatingInterval::refine` は、同じ最小多項式と根番号を保ったまま、要求した幅以下の区間へ精密化します。

### 厳密な観測

比較、符号、零と 1 の判定は、厳密な結果を返します。
同定に使う最小多項式と根番号も読み出せます。

- `compare`: 二つの実代数的数を比較する
- `compare_dyadic`: 二進有理数の値と比較する
- `sign`: 符号を判定する
- `is_zero`: 最小多項式と根番号から、零かを判定する
- `is_one`: 最小多項式と根番号から、乗法単位元 1 かを判定する
- `minimal_polynomial`: 同定に使う最小多項式を読む
- `root_index`: 同定に使う根番号を読む

## 厳密な代数演算

`RealAlgebraic` は、四則演算、冪、冪根を提供します。
形式和からの変換と、形式和との等しさの判定も行えます。

- `add`: 二つの実代数的数を加える
- `sub`: 二つの実代数的数の差をとる
- `mul`: 二つの実代数的数を掛ける
- `div`: 二つの実代数的数で割る
- `pow_integer`: 整数の指数で冪をとる
- `pow_rational`: 既約な有理数の指数で、実数値の冪をとる
- `nth_root`: 正の次数の実根をとる
- `from_form_sum`: 厳密な `FormSum` を実代数的数へ昇格する
- `equals_form_sum`: 形式和との等しさを判定する。形式和を `FormSum` の演算で最小多項式へ代入した値が零になり、形式和の値を含む区間が、根番号が指す根の区間とだけ交われば、等しい

加算、減算、乗算、除算、冪、根の構成は、いずれも終結式の構成、平方因子の除去、完全な因数分解、証明付きの根の選択という段階を経て、結果も最小多項式と根番号の組 ( `RealAlgebraic` ) で返します。除算では、最初に除数が零かを検査し、零なら拒否します。負数の偶数根は実数の値を持たないため失敗として返り、負数の奇数根は一意な実根を返します。

## 失敗

`AlgnumError` は入力、演算、表現、格納、下位 crate の失敗を区別します。

- `ZeroPolynomial`: 次数が 1 以上の候補が存在しない
- `InvalidIsolation`: 端点が逆順か等しい、または端点そのものが多項式の根である
- `NoTargetRoot`: 指定した区間に対象の根がない
- `MultipleTargetRoots`: 指定した区間に対象の根が複数ある
- `DivisionByZero`: 厳密に零である値で割ろうとした
- `UndefinedZeroPower`: `0^0` を求めた
- `ZeroToNegativePower`: 零を負の指数で冪乗した
- `ZeroRootDegree`: 次数が 0 の根を求めた
- `EvenRootOfNegative`: 負数の偶数次の根を求めた
- `RepresentationLimit`: 次数、係数の数、Sylvester 行列の大きさが表現の上限を超えた
- `AllocationLimit`: 必要な総要素数を厳密に見積もった値が `usize` の上限を超えた
- `AllocationFailure`: アロケータが確保を拒否した。対象の資源と要求した要素数を保持する
- `Bigint`: `neco-bigint` の失敗を、列挙子と内容ごと保持する
- `FormSum`: `neco-formsum` の失敗を、列挙子と内容ごと保持する

### 表現上限の対象

`RepresentationResource` は、上限を超えた対象を区別します。

- 根の次数
- 多項式の次数
- 係数の数
- Sylvester 行列の辺長
- Sylvester 行列の要素数

### メモリ確保の対象

`AllocationResource` は、確保に失敗した対象を十三種類に区別します。

- 多項式係数
- 有理係数
- Kronecker 法の評価点
- 評価値の約数
- 約数の組を列挙する添字
- 因子候補
- 因子
- Sturm 列
- 根の区間
- Sylvester 行列の要素
- 行列式の展開に使う置換
- 終結式の係数
- 根の候補

格納に関する失敗は、総要素数がプラットフォームの上限を超える場合と、上限内の要求をアロケータが拒否する場合の二種類です。

可変長の値を所有する公開型は `try_clone` を備え、複製時の確保の失敗を `Result` で返します。

## 機能と依存

既定の `std` 機能を有効にすると、`AlgnumError` を標準のエラー型として使え、下位の crate の失敗もエラーの原因から辿れます。
この機能は、依存する二つの crate の同名の機能も有効にします。
既定機能を無効にすると、同じ厳密値と同じ失敗を扱う `core + alloc` の構成になります。

```console
cargo check -p neco-algnum --no-default-features
```

実行時には、`neco-bigint` と `neco-formsum` に依存します。

## ライセンス

MIT License です。
