# neco-expr

[English](README.md)

`neco-expr` は、厳密値を式のグラフとして保持し、解決した値を受け取る利用箇所ごとに、証明付きの浮動小数点値へ解決する crate です。以下、この利用箇所を消費点 ( consumer ) と呼びます。
解決の結果には、有限な `f64` の値、要求した精度を満たす二進有理数の区間、厳密な絶対誤差の上限がそろっています。浮動小数点への近似は解決の最終段でだけ行い、途中の値は最後まで厳密なままです。

## 利用例

厳密値 1 を atom ( 式の末端に置く厳密値 ) として格納し、絶対精度 20 ビットで解決する例です。

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

`ExactValue` は、`ExactLayer` が区別する三つの層のいずれかの厳密値を保持します。

- `Monomial` ( 層 1 ) : 正規化済みの単項式
- `FormSum` ( 層 2 ) : 正規化済みの形式和
- `Algebraic` ( 層 3 ) : 最小多項式と実根の番号を持つ実代数的数

式のグラフは、`ExprGraph` に `ExprNode` の節を追加して組み立てます。

- `ExprGraph`: 節を追加した順に保持するアリーナ
- `ExprNode`: atom、負号、四則演算、既約な有理数の冪

演算の節は既に追加された節だけを参照できるため、グラフは常に有向非巡回です。

たとえば黄金比を表す式は、`Monomial` の atom $ 5^{1/2} $ と厳密な定数から組み立てます。加算の結果は `FormSum` です。

この式を解決すると、$ \varphi $ を含む要求した幅以下の区間と、その中から選んだ有限な `f64` が返ります。

$$ \varphi = \frac{1 + 5^{1/2}}{2} $$

結果の層は、入力の層と演算の種類から一意に決まります。

| 演算 | 入力 | 結果 |
|---|---|---|
| 負号 | 各層 | 入力と同じ層 |
| 加算・減算 | `Algebraic` を含む | `Algebraic` |
| 加算・減算 | その他の組み合わせ | `FormSum` |
| 乗算・除算 | `Monomial` 同士 | `Monomial` |
| 乗算・除算 | `Algebraic` を含む | `Algebraic` |
| 乗算・除算 | その他の組み合わせ | `FormSum` |
| 整数冪 | 各層 | 入力と同じ層 |
| 整数でない有理数冪 | `Monomial` | `Monomial` |
| 整数でない有理数冪 | `FormSum` | `Algebraic` |
| 整数でない有理数冪 | `Algebraic` | `Algebraic` |

## 入力と解決

`Resolver::resolve_all` は、解決の入力を、型の異なる四つの引数として受け取ります。

- `ExprGraph`: 式の節
- `AtomStore`: atom の ID と厳密値の対応
- `PrecisionRequirements`: 消費点の ID と絶対精度の対応
- `Assignments`: 消費点の ID と式の ID の対応

`resolve_all` は、一回の呼び出しで、すべての消費点を ID の順に解決します。

返り値は、評価のキャッシュ、分離区間のキャッシュ、解決した値をこの順に並べた組です。

- `EvaluationCache`: 到達した式の ID ごとの厳密値、または評価の失敗
- `IsolationCache`: 同じ式の ID と精度で再利用できる、代数的数の分離区間
- `ResolvedValues`: 消費点の ID ごとの `CertifiedF64`、または解決の失敗

未知の式の ID や atom の ID は評価キャッシュへ保存せず、要求した消費点の解決の失敗として記録します。一つの消費点が失敗しても、その失敗は結果の写像へ格納され、残りの消費点の解決は続きます。そのため、解決の全体は、キャッシュや結果の写像そのものを格納できない場合に限り、失敗として返ります。

## 証明付きの浮動小数点値

`CertifiedF64` は、三つの値をまとめて保持します。

- `value()`: 最近接偶数丸め ( ties-to-even ) で選んだ有限な `f64`
- `enclosure()`: 厳密値を含む二進有理数の区間
- `absolute_error()`: 選んだ値から区間の両端までの距離のうち、大きい方

`AbsoluteBits(bits)` で指定する絶対精度は、区間の幅に対する次の上限です。

```text
upper - lower <= 2^(-bits)
```

精度には、零を含むすべての `u32` の値を指定できます。浮動小数点値の選択は、精度の要求とは独立に行います。値の選択では、負の零を正の零へ正規化したうえで、非正規数、正の零、最大の有限値を含むすべての有限な `f64` の値と厳密値を比較します。

厳密値から、有限な `f64` の値と、厳密値を含む区間を求める操作を、ここでは射影と呼びます。式のグラフの外で利用者が所有する厳密値は、`project_exact_value_f64` で射影します。
この関数の引数は厳密な入力値と区間の精度の方針で、返り値は射影の結果です。

- `project_exact_value_f64`: 一つの厳密値を射影する
- `ExactValue`: 厳密な入力値
- `ProjectionPolicy`: 区間の精度を指定する
- `CertifiedScalarProjection`: 方針、選んだ値、区間、絶対誤差を保持する

`project_exact_value_f64` は、厳密値が有限な浮動小数点値の表現域を超える場合に、`ScalarProjectionError::FloatOutOfRange` を返します。

既に求めた区間の射影には `project_enclosure_f64` を使い、区間の中点を丸めて `f64` の値を選びます。丸めと絶対誤差の計算は、式の解決と同じ処理です。

ただし、区間の幅が、要求した `AbsoluteBits` の許す最大の幅を超える場合、この関数は `EnclosureProjectionError::InsufficientWidth` を返します。

## 失敗の型

- `GraphError`: 識別子の枯渇、未追加の節への参照、節の複製、グラフの格納の失敗
- `InsertError`: 識別子の重複、atom の値の複製、入力の写像の格納の失敗
- `EvalError`: 零除算、零の冪、偶数次の根、下位 crate の失敗
- `ResolveError`: 割り当ての不足 ( 精度を要求した消費点に、式が割り当てられていない ) 、未知の識別子、有限な表現域の超過、評価、下位の算術、結果の格納の失敗
- `ScalarProjectionError`: 単独の射影での表現域、下位 crate、格納の失敗
- `EnclosureProjectionError`: 区間の射影での幅の不足と、下位 crate の失敗
- `NecoFailure::ProvenanceMismatch`: 消費点、所有者、式、精度、実装元 ( `NecoImplementationSource` )、計算結果の識別子のいずれかの、要求との不一致
- `NecoFailure::MatrixInputMismatch`: 行列の形状、式の対応、浮動小数点値のビット列のいずれかの、証明付きの射影との不一致
- `NecoFailure::MissingMatrixInput`: 一般化固有値問題の入力の構築で、質量行列とその扱いを表す `MassNormalizationReference` が渡されなかった
- `NecoFailure::PrecisionExhaustion`: 要求した区間の幅を達成できない。要求精度と下位の原因を保持する
- `StorageError`: 容量の超過、または必要な総要素数を伴うメモリ確保の失敗

値を持つ `NecoFailure` の列挙子では、値の格納先は `Box` の領域です。

- 構築: 値を格納する `Box` の領域を確保する。確保に失敗した場合は、既定の確保失敗の処理に従う
- 値を参照するメソッド: 値を直接保持した場合と同じ
- `Debug` 出力: 値を直接保持した場合と同じ

区間計算の下位の失敗は、`ResolveError` の二つの列挙子として伝搬します。

- `ResolveError::Bigint`: 二進有理数と区間の処理に由来する失敗
- `ResolveError::Algnum`: 代数的数の区間の処理に由来する失敗

保持している下位の失敗は、`std::error::Error::source` で参照できます。
所有するデータは、`try_clone` で確保の失敗を検査しながら複製できます。
列を所有する型の複製手段は、`try_clone` だけです。

## 高水準の厳密計算

所有者 ( owner ) は、式の集合と要求を持ち込む利用側の単位で、利用側が与える `OwnerId` で区別します。高水準の処理は、所有者ごとに式の集合を受け取り、各要求に厳密計算と数値計算を割り当てます。式の集合は、下位の式グラフの API で作った式のグラフと atom から組み立てます。

- `OwnerId`: 所有者の識別子。利用側が `u32` の値を与える
- `OwnerExpressionSet`: 一つの所有者と、その所有者の式のグラフ、atom、要求
- `ExactAllocationInput`: 所有者ごとの式の集合の列、観測した能力、実装元
- `ExactExpressionRequirement`: 一つの消費点、指定した式、厳密な判定の指定、絶対精度。判定の指定は `ExactDecisionRequest` の値で、判定の種類 ( 零、等値、符号、縮退 ) と、判定する式を持つ。縮退の判定は、判定する式の値が零の場合に縮退、それ以外の場合に非縮退を返す
- `NecoObservedCapability`: 実装元で利用できることを観測した操作の一覧
- `NecoImplementationSource`: 実装元。結果が参照する版と公開 API の識別子。版は `neco-expr` のパッケージの版で、crates.io の同じ版に対応する

所有者の一致は、識別子の値だけで判定します。

- 来歴の所有者が要求の所有者と異なる場合: `ProvenanceMismatchField::Owner` の失敗を返す
- 同じ所有者が二度現れる場合: その所有者の識別子を重複した識別子として持つ `NecoFailure::StorageFailure` を返す

異なる所有者が同じ `ConsumerId` を使う場合も、各結果はその所有者の要求から作ります。所有者の列をどの順序で与えても、結果は同じです。

高水準の処理を構成する公開関数は、次の順に呼び出します。
三つ目以降の関数は、それぞれ前の関数が返した値の所有権を受け取るため、二つ目から六つ目まではこの順にだけ呼び出せます。

1. `read_exact_numeric_inputs`
2. `allocate_exact_numeric`
3. `normalize_exact_expressions`
4. `decide_exact_properties`
5. `resolve_certified_f64`
6. `assemble_exact_computation_product`

`ExactNumericAllocation` は、解決する所有者と消費点の組の集合を保持する値です。

`ExactComputationProduct` は、所有者と消費点の組ごとに `CertifiedF64Bundle` を返し、結果は所有者の識別子と消費点の識別子の組で引きます。結果の来歴は、次の値から読めます。

- `CertifiedF64Bundle`: 所有者、消費点、要求精度、実装元、計算結果の識別子
- `ComputationProductId`: 一回の計算の結果を識別する不透明な識別子
- `ConsumerId`: 消費点を指す識別子
- `AbsoluteBits`: 要求する二進有理数の区間の幅
- `NecoImplementationSource`: 結果が参照する版と公開 API
- `CertifiedF64Expression`: 式ごとの証明付きの結果
- `ExprId`: 式を指す識別子
- `CertifiedF64`: 有限な浮動小数点値、包含区間、絶対誤差の上限
- `CertifiedF64Bundle::try_clone`: 来歴と計算結果の識別子を保ったまま複製する

組み立ての段階では、要求の集合と結果の集合が完全に一対一に対応することを検査し、来歴の不一致や欠落を `NecoFailure` として返します。

`AbsoluteBits` は、厳密な二進有理数の区間の幅だけを制限します。一方、有限な浮動小数点値は、要求した幅とは独立に選びます。厳密値に最も近い値を選び、等距離の場合は最近接偶数丸め ( ties-to-even ) で決めます。高水準の結果は、要求した幅を満たす区間と、有限な丸め値を組み合わせた値です。

`direct_inspection` は、割り当て、所有者と消費点の組を持つ要求、判定の結果、証明付きの値、来歴、解決の件数を読み出します。解決の件数は、所有者ごとに同じ式と同じ精度の要求を一つにまとめ、実際に行った解決を数えた値です。射影の絶対誤差の上限は、`CertifiedF64::absolute_error()` が区間の端点から求めます。

数値誤差の予算と物理的な測定値の不確かさは、各消費点が扱います。
消費点は `OwnerId` と `ConsumerId` の組で識別します。
利用側はこの組で、自身の値と証明付きの結果を対応付けます。

## 構成と依存

実行時の依存は、このリポジトリの crate だけです。

- `neco-bigint`
- `neco-monomial`
- `neco-formsum`
- `neco-algnum`

既定の `std` 機能を有効にすると、`NecoFailure` を除く失敗の型を、標準のエラー型として使えます。
この機能は、依存する crate の同名の機能も有効にします。
既定機能を無効にすると、`core + alloc` の構成になります。

```bash
cargo check -p neco-expr --no-default-features
```

## ライセンス

MIT License です。
