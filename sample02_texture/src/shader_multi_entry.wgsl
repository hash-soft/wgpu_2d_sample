// 画面全体で共通のデータ（画面サイズなど）
struct GlobalUniforms {
    screen_size: vec2<f32>,
}

@group(1) @binding(0)
var<uniform> global_uniforms: GlobalUniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,   // (0.0 ~ 1.0 のローカル座標)
    @location(1) tex_coords: vec2<f32>,
}

// スプライトごとの個別データ（インスタンス入力）
struct SpriteInstanceInput {
    @location(2) display_position: vec2<f32>, // 画面上のピクセル位置 (x, y)
    @location(3) display_size: vec2<f32>,     // 画面上の表示サイズ (width, height)
    @location(4) uv_offset: vec2<f32>,       // 切り出し左上 (u0, v0)
    @location(5) uv_size: vec2<f32>,         // 切り出し幅・高さ (u_w, v_h)
    @location(6) texture_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) @interpolate(flat) texture_index: u32,
}

@vertex
fn vs_main(model: VertexInput, instance: SpriteInstanceInput) -> VertexOutput {
    var out: VertexOutput;
    // 0.0 ~ 1.0 のローカル UV を指定矩形の UV 範囲にスケーリング・シフト
    out.tex_coords = instance.uv_offset + model.tex_coords * instance.uv_size;
    //out.tex_coords = model.tex_coords;

    // ピクセル位置と画像サイズからスクリーン上のピクセル座標を計算
    let pixel_pos = instance.display_position + model.position * instance.display_size;

    // ピクセル座標 (0 ~ screen_size) を NDC 座標 (-1.0 ~ 1.0) に変換
    // X: 0 -> -1.0, width -> 1.0
    // Y: 0 -> 1.0 (上), height -> -1.0 (下)
    let ndc_x = (pixel_pos.x / global_uniforms.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (pixel_pos.y / global_uniforms.screen_size.y) * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.texture_index = instance.texture_index;
    return out;
}

// Fragment shader
@group(0) @binding(0) var t_texture0: texture_2d<f32>;
@group(0) @binding(1) var t_texture1: texture_2d<f32>;
@group(0) @binding(2) var t_texture2: texture_2d<f32>;
@group(0) @binding(3) var s_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    switch in.texture_index {
        case 1: {
            return textureSample(t_texture1, s_sampler, in.tex_coords);
        }
        case 2: {
            return textureSample(t_texture2, s_sampler, in.tex_coords);
        }
        default: {
            return textureSample(t_texture0, s_sampler, in.tex_coords);
        }
    }
}
