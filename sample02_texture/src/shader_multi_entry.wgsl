// sample02_texture\src\run_bind_group_multi_entry.rs
// で動的作成しているシェーダの元となったファイル
// サンプルはこちらを使う

// 画面全体で共通のデータ（画面サイズなど）
struct GlobalUniforms {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> global_uniforms: GlobalUniforms;

// スプライトごとの個別データ（インスタンス入力）
struct SpriteInstanceInput {
    @location(0) display_position: vec2<f32>, // 画面上のピクセル位置 (x, y)
    @location(1) display_size: vec2<f32>, // 画面上の表示サイズ (width, height)
    @location(2) uv_offset: vec2<f32>, // 切り出し左上 (u0, v0)
    @location(3) uv_size: vec2<f32>, // 切り出し幅・高さ (u_w, v_h)
    @location(4) texture_index: u32,
    @location(5) opacity: f32,
    @location(6) scale: vec2<f32>, // 拡大縮小倍率 (sx, sy) / 1.0 = 等倍
    @location(7) rotation: f32,       // 回転角度（ラジアン, 時計回り正）
    @location(8) pivot: vec2<f32>, // 回転軸（ローカル比率 0.0〜1.0 / [0.5,0.5] = 中心）
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) @interpolate(flat) texture_index: u32,
    @location(2) @interpolate(flat) opacity: f32,
}

// 描画先と元どちらも全体を対象とするので同じ値を使う
const VERTEX_POSITIONS = array<vec2<f32>, 4>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0)
);

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32, instance: SpriteInstanceInput) -> VertexOutput {
    var out: VertexOutput;

    let local_uv = VERTEX_POSITIONS[in_vertex_index];

    // ── UV 座標（変換なし）──────────────────────────────────────
    out.tex_coords = instance.uv_offset + local_uv * instance.uv_size;

    // ── ローカル頂点座標（スケール × サイズ）────────────────────
    let scaled_size = instance.display_size * instance.scale;
    let local = local_uv * scaled_size;

    // ── ピボット（回転軸）をピクセル単位へ変換 ──────────────────
    // pivot は [0,1] の比率なので scaled_size を乗じてピクセルオフセットへ
    let pivot_px = instance.pivot * scaled_size;

    // ── ピボットを原点として回転 ──────────────────────────────────
    let centered = local - pivot_px;
    let cos_r = cos(instance.rotation);
    let sin_r = sin(instance.rotation);
    let rotated = vec2<f32>(
        centered.x * cos_r - centered.y * sin_r, // 2D 回転行列の X 成分
        centered.x * sin_r + centered.y * cos_r, // 2D 回転行列の Y 成分
    );

    // memo
    // X: 0 -> -1.0, width -> 1.0
    // Y: 0 ->  1.0（上）, height -> -1.0（下）

    // ── ピボットを元の位置に戻して表示位置を加算 ────────────────
    let pixel_pos = instance.display_position + rotated + pivot_px;
    // ── 変更：行列を使ってカメラのスクロール・拡縮・回転と NDC 変換をまとめて適用 ──
    out.clip_position = global_uniforms.view_proj * vec4<f32>(pixel_pos, 0.0, 1.0);

    out.texture_index = instance.texture_index;
    out.opacity = instance.opacity;
    return out;
}

// Fragment shader
@group(1) @binding(0) var t_texture0: texture_2d<f32>;
@group(1) @binding(1) var t_texture1: texture_2d<f32>;
@group(1) @binding(2) var t_texture2: texture_2d<f32>;
@group(1) @binding(3) var t_texture3: texture_2d<f32>;
@group(2) @binding(0) var s_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {

    var tex_color: vec4<f32>;
    switch in.texture_index {
        case 0: {
            tex_color = textureSample(t_texture0, s_sampler, in.tex_coords);
        }
        case 1: {
            tex_color = textureSample(t_texture1, s_sampler, in.tex_coords);
        }
        case 2: {
            tex_color = textureSample(t_texture2, s_sampler, in.tex_coords);
        }
        case 3: {
            tex_color = textureSample(t_texture3, s_sampler, in.tex_coords);
        }
        default: {
            discard;
            break;
        }
    }
    return vec4<f32>(tex_color.rgb, tex_color.a * in.opacity);
}
