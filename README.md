# [`wgpu_2d_sample`](Cargo.toml)

Rustのグラフィックスライブラリ [`wgpu`](Cargo.toml) を用いた、2Dグラフィックスおよびスプライト描画のサンプル集です。基礎的なポリゴン描画から、多様なテクスチャ管理手法、ビューポート制御まで段階的に学ぶことができます。

---

## 📁 ワークスペース構成

このリポジトリは [`Cargo.toml`](Cargo.toml:9) で管理されるCargoワークスペースです。以下のサンプルプロジェクトが含まれています。

### 1. [`sample01_polygon`](sample01_polygon/Cargo.toml)
- **概要**: [`wgpu`](Cargo.toml) を用いた基本的な多角形描画とシェーダーの基礎サンプルです。
- **主なファイル**: [`shader.wgsl`](sample01_polygon/src/shader.wgsl), [`shader_buffer.wgsl`](sample01_polygon/src/shader_buffer.wgsl), [`shader_dyn.wgsl`](sample01_polygon/src/shader_dyn.wgsl)
- **実行方法**:
  ```bash
  cargo run --package sample01_polygon
  ```

### 2. [`sample02_texture`](sample02_texture/Cargo.toml)
- **概要**: テクスチャ描画や各種バインドグループ構成（マルチバインドグループ、2Dアレイ、マルチエントリなど）の実装サンプルです。
- **実行バリエーション**: 引数によって異なる描画手法を選択できます。
  - [`introduction`](sample02_texture/src/run_introduction.rs:1): 固定位置への基本テクスチャ表示
    ```bash
    cargo run --package sample02_texture -- introduction
    ```
  - [`uniform`](sample02_texture/src/run_uniform.rs:1): ユニフォームを用いた同一テクスチャの複数回描画
    ```bash
    cargo run --package sample02_texture -- uniform
    ```
  - [`multi_bind_group`](sample02_texture/src/run_multi_bind_group.rs:1): 複数のBindGroupを登録し複数のテクスチャを表示
    ```bash
    cargo run --package sample02_texture -- multi_bind_group
    ```
  - [`2d_array`](sample02_texture/src/run_multi_2d_array.rs:1): テクスチャアレイを用いた1回描画での複数テクスチャ表示
    ```bash
    cargo run --package sample02_texture -- 2d_array
    ```
  - [`bing_group_multi_entry`](sample02_texture/src/run_bind_group_multi_entry.rs:1): 1つのBindGroup内で複数のテクスチャエントリを登録
    ```bash
    cargo run --package sample02_texture -- bing_group_multi_entry
    ```

### 3. [`sample03_viewport`](sample03_viewport/Cargo.toml)
- **概要**: ビューポートの制御や画面サイズ変更への対応、ストレッチ表示に関するサンプルです。
- **主なバリエーション**:
  - 通常表示 ([[`run.rs`](sample03_viewport/src/run.rs:1)])
    ```bash
    cargo run --package sample03_viewport -- normal
    ```
  - ストレッチ表示 ([[`run_stretch.rs`](sample03_viewport/src/run_stretch.rs:1)])
    ```bash
    cargo run --package sample03_viewport -- strech
    ```

---

## ⚙️ 必要環境

- **Rust**: [`edition = "2024"`](Cargo.toml:4) に対応した最新の Rust ツールチェーン（stable）
- **グラフィックスAPI**: [`wgpu`](Cargo.toml) がサポートするバックエンド（Vulkan, Metal, DirectX 12, WebGPU 等）

---

## 🚀 使い方

1. リポジトリをクローンします。
2. ルートディレクトリで各パッケージを指定して実行します。

```bash
cargo run --package <パッケージ名>
```