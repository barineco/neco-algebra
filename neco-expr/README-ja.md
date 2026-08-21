# neco-expr

[English](README.md)

`neco-expr` は、厳密値を式グラフとして保持し、消費点ごとに保証付きの浮動小数点値へ解決する crate です。解決の結果には、有限な `f64` の値、要求した精度を満たす二進有理数の区間、厳密な絶対誤差の上限が揃います。浮動小数点への近似は解決の最終段だけで行い、途中の値は最後まで厳密なままです。

## 利用例

厳密な一を atom として格納し、絶対精度 20 ビットで解決する例です。

```rust
use neco_expr::{
    AbsoluteBits, Assignments, AtomId, AtomStore, ConsumerId, ExactValue, ExprGraph,
    ExprNode, PrecisionRequirements, Resolver,
};
use neco_monomial::Monomial;

let mut graph = ExprGraph::new();
let expression = graph
    .push(ExprNode::Atom(AtomId::new(0)))
    .expect("valid expression node");

let mut atoms = AtomStore::new();
let atom_value = ExactValue::Monomial(Monomial::one());
atoms
    .insert(AtomId::new(0), atom_value)
    .expect("unique atom ID");

let consumer = ConsumerId::new(0);
let mut requirements = PrecisionRequirements::new();
requirements
    .insert(consumer, AbsoluteBits::new(20))
    .expect("unique consumer ID");
let mut assignments = Assignments::new();
assignments
    .insert(consumer, expression)
    .expect("unique consumer ID");

let (_, _, resolved) = Resolver::new()
    .resolve_all(&graph, &atoms, &requirements, &assignments)
    .expect("sufficient storage");
let certified = resolved
    .get(consumer)
    .expect("requested consumer")
    .as_ref()
    .expect("successful resolution");
assert_eq!(certified.value(), 1.0);
```

## 式と厳密値

`ExactValue` は三つの層の厳密値を保持します。

- `Monomial`: 正規化済みの単項式
- `FormSum`: 正規化済みの形式和
- `Algebraic`: 最小多項式と実根番号を持つ実代数的数

式グラフは次の二つの型で構成します。

- `ExprGraph`: 節を追加順に保持するアリーナ
- `ExprNode`: atom、負号、四則演算、既約有理数の冪

演算の節はすでに追加された節だけを参照できるため、グラフは常に有向非巡回です。

たとえば黄金比を表す式は、atom $ 5^{1/2} $ ( 層 1 ) と厳密な定数から組み立てられます。加算の結果は層 2 の形式和になり、解決は $ \varphi $ を含む要求幅以下の区間と、その中から選んだ有限な `f64` を返します。

$$ \varphi = \frac{1 + 5^{1/2}}{2} $$

結果の層は、入力の層と演算の種類から一意に決まります。

| 演算 | 入力 | 結果 |
|---|---|---|
| 負号 | 各層 | 入力と同じ層 |
| 加算・減算 | 層 1 と層 2 | `FormSum` |
| 加算・減算 | 層 3 を含む | `Algebraic` |
| 乗算・除算 | `Monomial` 同士 | `Monomial` |
| 乗算・除算 | 層 2 まで | `FormSum` |
| 乗算・除算 | 層 3 を含む | `Algebraic` |
| 整数冪 | 各層 | 入力と同じ層 |
| 真の有理数冪 | `Monomial` | `Monomial` |
| 真の有理数冪 | `FormSum` | `Algebraic` |
| 真の有理数冪 | `Algebraic` | `Algebraic` |

## 宣言と解決

入力は互いに独立した値として構成します。

- `ExprGraph`: 式の節
- `AtomStore`: atom ID と厳密値の対応
- `PrecisionRequirements`: consumer ID と絶対精度の対応
- `Assignments`: consumer ID と式 ID の対応

解決は一回の呼び出しで行います。

- `Resolver::resolve_all`: すべての consumer を ID 順に解決する

返り値は次の三つです。

- `EvaluationCache`: 到達した式 ID ごとの厳密値、または評価の失敗
- `IsolationCache`: 同じ式 ID と精度で再利用できる、代数的数の分離区間
- `ResolvedValues`: consumer ID ごとの `CertifiedF64`、または解決の失敗

未知の式 ID や atom ID は評価キャッシュへ保存せず、要求した consumer の解決失敗として記録します。一つの consumer が失敗しても、その失敗は結果の写像へ格納され、残りの consumer の解決は続きます。解決の全体が失敗として返るのは、キャッシュや結果の写像そのものを格納できない場合です。

## 保証付きの浮動小数点値

`CertifiedF64` は三つの値をまとめて保持します。

- `value()`: 最近接偶数丸め ( ties-to-even ) で選んだ有限な `f64`
- `enclosure()`: 厳密値を含む二進有理数の区間
- `absolute_error()`: 選択した値から区間の両端までの距離の最大値

絶対精度の指定は次のとおりです。

- `AbsoluteBits(bits)`: 区間の幅を制限

```text
upper - lower <= 2^(-bits)
```

精度には零を含むすべての `u32` 値を指定できます。浮動小数点値の選択は精度の要求から独立しています。負の零は正の零へ正規化します。比較の対象は、非正規数、正の零、最大の有限値を含むすべての有限な `f64` 値です。

式グラフの外で利用者が所有する厳密値には、次の公開 API を適用します。

- `project_exact_value_f64`: 一つの厳密値を射影
- `ExactValue`: 厳密な入力値
- `ProjectionPolicy`: 区間精度の指定
- `CertifiedScalarProjection`: 方針、選択値、区間、絶対誤差

有限な `f64` の表現域を超える厳密値は、次の失敗として返ります。

- `ScalarProjectionError::FloatOutOfRange`: 厳密値が有限な表現域の外

## 公開失敗

失敗の型は処理の段階ごとに分かれます。

- `GraphError`: 識別子の枯渇、未追加の節への参照、節の複製、グラフの格納
- `InsertError`: 識別子の重複、atom 値の複製、入力写像の格納
- `EvalError`: 零除算、零の冪、偶数根、下位 crate の失敗
- `ResolveError`: 宣言の不足、未知の識別子、有限な表現域、評価、下位算術、結果の格納
- `ScalarProjectionError`: 単独射影の表現域、下位 crate、格納の失敗
- `StorageError`: 容量超過、必要な総要素数を伴うメモリ確保失敗

区間計算の下位失敗は次の列挙子として伝搬します。

- `ResolveError::Bigint`: 二進有理数と区間の処理に由来する失敗
- `ResolveError::Algnum`: 代数的数の区間処理に由来する失敗

下位の失敗と複製の入口は次のとおりです。

- `std::error::Error::source`: 保持している下位の失敗を参照
- `try_clone`: 所有するデータを検査付きで複製

## 高水準の厳密計算

高水準の処理は二つの所有者へ厳密計算と数値計算を割り当てます。下位の式グラフ API は、その入力として維持します。

- Modal Field Projection
- Wavesim

主な入力値は次のとおりです。

- `ReadExactNumericInput`: 二つの所有者から読み取った式グラフと atom
- `ExactAllocationInput`: 外部観測の能力、実装由来、厳密入力
- `ExactExpressionRequirement`: 一つの消費点、指定された式、型付き厳密判定、絶対精度
- `NecoObservedCapability`: 外部観測から渡される能力
- `NecoImplementationSource`: 外部観測から渡される実装由来

処理は所有権を移しながら、次の順で値を変換します。

1. `read_exact_numeric_inputs`: 二つの所有者の入力を読み取り結果へ変換
2. `allocate_exact_numeric`: 全候補を三つの有限集合へ分類
3. `normalize_exact_expressions`: 各所有者の式グラフを正規化
4. `decide_exact_properties`: 要求された厳密判定を実行
5. `resolve_certified_f64`: 要求された式を認証付き浮動小数点値へ解決
6. `assemble_exact_computation_product`: 割り当て、要求、判定、認証付きの値を集約

`ExactNumericAllocation` が保持する全量集合は次の三つです。

- 厳密入力
- 厳密判定
- 数値演算

`ExactComputationProduct` は各消費点へ認証付きの値を返します。同じ所有者の式グラフでは、同じ式と精度の要求が一回の解決を共有します。

観測入口は次のとおりです。

- `direct_inspection`: 割り当て、要求、型付き判定、認証付きの値、共有した解決の件数

高水準の失敗は閉じた `NecoFailure` を使います。操作、消費点、式、atom、判定の位置を保持し、格納と下位算術の失敗を対応付けます。数値誤差予算は所有者が管理し、認証付き浮動小数点値の絶対誤差には加算しません。

## 構成と依存

実行時の依存は次の四 crate です。

- `neco-bigint`
- `neco-monomial`
- `neco-formsum`
- `neco-algnum`

既定の `std` 機能は、標準エラー型との連携と、依存 crate の同名機能を有効にします。既定機能を無効にすると `core + alloc` 構成になります。

```bash
cargo check -p neco-expr --no-default-features
```

## ライセンス

MIT License です。
