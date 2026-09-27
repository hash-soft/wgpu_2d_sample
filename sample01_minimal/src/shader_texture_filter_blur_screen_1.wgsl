// shader_multipass_pass1.wgsl
// マルチパスブラーサンプル: Pass 1
// スプライトをオフスクリーンテクスチャへ描画する
// shader_multi_entry.wgsl の簡略版（テクスチャ1枚固定・texture_index不使用）

struct GlobalUniforms {
    screen_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> global_uniforms: GlobalUniforms;

// SpriteCharacterInstance のレイアウトと合わせる（Pass1では texture_index は使わない）
struct SpriteInstanceInput {
    @location(0) display_position: vec2<f32>,
    @location(1) display_size: vec2<f32>,
    @location(2) uv_offset: vec2<f32>,
    @location(3) uv_size: vec2<f32>,
    @location(4) texture_index: u32,   // レイアウト合わせのため宣言するが未使用
    @location(5) opacity: f32,
    @location(6) scale: vec2<f32>,
    @location(7) rotation: f32,
    @location(8) pivot: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) @interpolate(flat) opacity: f32,
}

// 左上→左下→右上→右下 (TriangleStrip)
const VERTEX_POSITIONS = array<vec2<f32>, 4>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
);

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
    instance: SpriteInstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    let local_uv = VERTEX_POSITIONS[in_vertex_index];

    // UV 座標
    out.tex_coords = instance.uv_offset + local_uv * instance.uv_size;

    // スケール済みローカル座標
    let scaled_size = instance.display_size * instance.scale;
    let local = local_uv * scaled_size;

    // ピボット（ピクセル単位）
    let pivot_px = instance.pivot * scaled_size;

    // ピボットを原点として回転
    let centered = local - pivot_px;
    let cos_r = cos(instance.rotation);
    let sin_r = sin(instance.rotation);
    let rotated = vec2<f32>(
        centered.x * cos_r - centered.y * sin_r,
        centered.x * sin_r + centered.y * cos_r,
    );

    // ピクセル座標 → NDC
    let pixel_pos = instance.display_position + rotated + pivot_px;
    let ndc_x = (pixel_pos.x / global_uniforms.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (pixel_pos.y / global_uniforms.screen_size.y) * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.opacity = instance.opacity;
    return out;
}

// スプライトテクスチャ（1枚固定）
@group(1) @binding(0) var t_sprite: texture_2d<f32>;
@group(2) @binding(0) var s_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_sprite, s_sampler, in.tex_coords);
    return vec4<f32>(color.rgb, color.a * in.opacity);
}
