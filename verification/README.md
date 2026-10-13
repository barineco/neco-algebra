# 代数演算の検証

この文書の手順で、代数演算の公開 API が返した値を、Lean で書いた検査器で確かめることができます。対象の演算は、有理数の既約化、拡張ユークリッド互除法、代数的数の根の分離、浮動小数点への射影、有理数の連立一次方程式の一意解とアフィン解です。演算ごとに検査器が確かめる性質と、観測バッチと判定の形式は、`protocol/README.md` に定めています。

手順では、まず生成器が公開 API を呼び、その観測値を観測バッチへ書き出します。
次に検査器が、観測バッチの各観測を、入力と観測値から計算し直した条件で判定します。
最後に観測器が、保存した観測バッチと検査器の結果を照合し、判定の件数を出力します。
根の分離では、検査器が確かめる条件が成り立つと、区間にただ一つの実根があります。条件は、両端点で符号が反転することと、導関数の値の範囲が正か負の一方に収まることです。この性質は、Lean の健全性定理 ( soundness ) で証明しています。

## 必要なもの

- Lean: `lean/lean-toolchain` が指す版
- Rust: stable のツールチェイン
- `git`
- `neco-lean/neco-lean-main`: Lean の補助ライブラリ
- `mathlib`: Lake が取得する

## リポジトリの配置

Lean の検査器は、補助ライブラリ `neco-lean` を `lean/lakefile.toml` の相対パスで参照します。
そのため、二つのリポジトリを `git clone` で複製し、同じ `<root>` の下に次の形で配置します。

- `neco-lean/neco-lean-main`
- `neco-algebra/neco-algebra-main`

各リポジトリは、`<repo>/<repo>-main` に main ブランチの作業ツリーを置きます。`mathlib` は Lake が取得するため、配置は不要です。Rust の生成器と観測器は、このリポジトリの中のパスだけを参照します。

## Lean の検査器

`lean` ディレクトリで、mathlib の構築済みの成果物を取得してから、検査器を構築します。

```sh
cd neco-algebra/neco-algebra-main/verification/lean
lake exe cache get
lake build
```

構築した検査器は、観測バッチを読み取り、結果を標準出力へ書き出します。

```sh
.lake/build/bin/neco-algebra-checker check <観測バッチ>
```

## Rust の生成器

`rust` ディレクトリで生成器を構築し、実行します。

```sh
cd neco-algebra/neco-algebra-main/verification/rust
cargo build --locked
cargo run --locked -p neco-algebra-producer -- describe
cargo run --locked -p neco-algebra-producer -- generate --request <request>
cargo run --locked -p neco-algebra-producer -- self-check --request <request> --batch <観測バッチ>
```

`<request>` は、正規形の JSON ( canonical JSON ) で書いた生成要求です。生成要求も、観測バッチと同じく `protocol/README.md` の共通規約に従います。
`<request>` に `-` を渡すと、標準入力から読み取ります。

## 観測器

`observer` ディレクトリの観測器は、保存した観測バッチと検査器の結果を照合し、判定の件数を出力します。

```sh
cd neco-algebra/neco-algebra-main/verification/observer
cargo run --locked -- --batch <観測バッチ> --checker-result <検査器の結果>
```

同梱の検証ベクターの検査と、簡易的な動作確認 ( smoke ) では、`artifacts/` に配置した検査器と観測バッチを使います。配置の手順は `artifacts/README.md` にあります。

```sh
cargo run --locked -- --vectors <verification/vectors/known|tampered|invalid>
cargo run --locked -- --smoke <case-count>
```
