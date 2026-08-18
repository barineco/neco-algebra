# neco-algnum

[English](README.md)

`neco-algnum` は、実代数的数を厳密に扱う crate です。値は、整数係数の最小多項式と実根の番号の組で同定します。多項式は原始的かつ既約で最高次係数が正という正規形を持ち、根番号は零から数えます。たとえば $ \sqrt{2} + \sqrt{3} $ は次の組で表されます。

$$ m(x) = x^{4} - 10x^{2} + 1, \qquad k = 3 $$

$ m $ の実根は $ \pm\sqrt{3} \pm \sqrt{2} $ の四つで、それらを昇順に数えた番号 $ k = 3 $ が最大の根 $ \sqrt{2} + \sqrt{3} $ を指します。証明付きの二進有理数区間は値の構成と観測に使い、値そのものの同一性は最小多項式と根番号が保ちます。

## 多項式と検証済みの型

整数係数と有理係数の多項式は、係数を次数の低い順に格納します。構成途中の値と検証済みの値は型で分かれています。

- `Polynomial`: 末尾の零を除いた整数係数多項式
- `RationalPolynomial`: 末尾の零を除いた有理係数多項式
- `CandidatePolynomial`: 次数が一以上の候補多項式
- `SquareFreePolynomial`: 原始的で最高次係数が正の、平方因子を持たない多項式
- `IrreduciblePolynomial`: 有理数体の上で既約な多項式
- `MinimalPolynomial`: 実代数的数を同定する既約多項式
- `PolynomialQuotient`: 剰余環 `Q[x] / (m)` での演算
- `GeneratorRepresentative`: 同じ法に対する生成元 `x mod m`
- `RootIndex`: 同じ既約多項式の実根を昇順に数えた番号
- `RealAlgebraic`: 最小多項式と実根番号からなる厳密値
- `IsolatingInterval`: 厳密値を含む観測用の二進有理数区間
- `CertifiedAlgebraic`: 根の分離が産出した厳密値と証明付き区間の組

`Polynomial` は加減乗算、微分、整数と有理数の点での評価、合成を提供します。`CandidatePolynomial::square_free` は内容と重複因子を取り除きます。因数分解は Kronecker 法で候補を完全に列挙し、既約性はその列挙だけを証拠とします。

### 多項式商

`PolynomialQuotient` は演算の結果を法で剰余化し、法より次数の低い `RationalPolynomial` を返却値とします。

- `reduce`: 一つの多項式を剰余化する
- `add`: 加算の結果を剰余化する
- `sub`: 減算の結果を剰余化する
- `mul`: 乗算の結果を剰余化する

`generator` は同じ法で剰余化した生成元を返します。
`RationalPolynomial::to_real_algebraic_coefficients` は有理係数を実代数的数へ変換し、`RationalCoefficientConversion` を返します。

## 証明付きの実根

Sturm 列はすべての実根を、それぞれ異なる根を一つずつ含む区間へ分離し、一つの既約多項式の中で値の昇順に根番号を与えます。隣接する区間は端点を共有する場合があります。根の構成には次の操作を使います。

- `isolate_real_roots`: すべての実根を証明付き区間とともに返す
- `certify_root`: 利用者が与えた二つの二進有理数の端点を検証する
- `into_value`: 証明付きの構成結果から厳密値を取り出す

### 区間の精密化

`RealAlgebraic::enclose` と `IsolatingInterval::refine` は、同じ最小多項式と根番号を保ったまま、要求した幅以下の区間へ精密化します。

### 厳密な観測

厳密な観測には次の操作を使います。

- `compare`: 二つの実代数的数の比較
- `compare_dyadic`: 二進有理数の値との比較
- `sign`: 符号の判定
- `is_zero`: 最小多項式と根番号による零の判定
- `is_one`: 最小多項式と根番号による乗法単位元の判定
- `minimal_polynomial`: 同定に使う最小多項式の観測
- `root_index`: 同定に使う根番号の観測

零と乗法単位元は、最小多項式と根番号だけから判定できます。

## 厳密な代数演算

`RealAlgebraic` は次の演算を提供します。

- `add`: 二つの実代数的数の加算
- `sub`: 二つの実代数的数の減算
- `mul`: 二つの実代数的数の乗算
- `div`: 二つの実代数的数の除算
- `pow_integer`: 整数指数による冪
- `pow_rational`: 既約な有理数指数による実数値の冪
- `nth_root`: 正の次数の実根
- `from_form_sum`: 厳密な `FormSum` からの昇格
- `equals_form_sum`: 最小多項式への代入と根番号で、層をまたいだ同値を判定

加算・減算・乗算・除算・冪・根の構成は、終結式の構成、平方因子の除去、完全な因数分解、証明付きの根の選択という段階を経て、どの結果も同じ値表現で返します。除算は零の除数を最初に検査して拒否します。負数の偶数根は実数の値を持たないため失敗として返り、負数の奇数根は一意な実根を返します。

## 失敗

`AlgnumError` は入力、演算、表現、格納、下位 crate の失敗を区別します。

- `ZeroPolynomial`: 次数一以上の候補が存在しない
- `InvalidIsolation`: 端点が逆順か同値、または端点自体が多項式の根
- `NoTargetRoot`: 指定区間に対象の根がない
- `MultipleTargetRoots`: 指定区間に対象の根が複数ある
- `DivisionByZero`: 厳密値の零除算
- `UndefinedZeroPower`: `0^0`
- `ZeroToNegativePower`: 零の負指数冪
- `ZeroRootDegree`: 次数零の根
- `EvenRootOfNegative`: 負数の偶数根
- `RepresentationLimit`: 次数、係数の数、Sylvester 行列が表現上限を超過
- `AllocationLimit`: 必要な総要素数の厳密な見積もりが `usize` の上限を超過
- `AllocationFailure`: アロケーター による確保の拒否。対象の資源と要求要素数を保持
- `Bigint`: `neco-bigint` の失敗を variant と内容ごと保持
- `FormSum`: `neco-formsum` の失敗を variant と内容ごと保持

補助の型は失敗の対象を細かく区別します。

- `RepresentationResource`: 根の次数、多項式の次数、係数の数、Sylvester 行列の辺長と要素数
- `AllocationResource`: 係数、因子、Sturm 列、根の区間、行列、置換、終結式などの格納対象

格納に関する失敗は二つの場合に分かれます。必要な総要素数の厳密な見積もりがプラットフォームの上限を超える場合と、上限の内側でも アロケーター が確保を拒否する場合です。

可変長の値を所有する公開型は `try_clone` を提供し、確保の失敗を `Result` で返します。

`std` 機能では `AlgnumError` を標準のエラー型として使えます。
下位の失敗はエラーの原因から辿れます。

## 機能と依存

既定の `std` 機能は、標準エラー型との連携と、両依存 crate の同名の機能を有効にします。既定機能を無効にすると、同じ厳密値と同じ失敗を扱う `core + alloc` 構成になります。

```console
cargo check -p neco-algnum --no-default-features
```

実行時の依存は次のとおりです。

- `neco-bigint`
- `neco-formsum`

## ライセンス

MIT License です。
