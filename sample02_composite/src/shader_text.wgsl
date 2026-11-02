// Vertex shader

struct GlobalUniforms {
    screen_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> global_uniforms: GlobalUniforms;

struct TextInstanceInput {
    @location(0) position: vec2<f32>, // 画面上のピクセル位置 (x, y)
    @location(1) size: vec2<f32>, // 画面上の表示サイズ (width, height)
    @location(2) uv_offset: vec2<f32>, // 切り出し左上 (u0, v0)
    @location(3) uv_size: vec2<f32>, // 切り出し幅・高さ (u_w, v_h)
    @location(4) texture_index: u32,
    @location(5) opacity: f32,
    @location(6) z: f32,
    @location(7) color_multiply: vec4<f32>,
    @location(8) color_add: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @interpolate(flat)
    @location(1) texture_index: u32,
    @interpolate(flat)
    @location(2) opacity: f32,
    @interpolate(flat)
    @location(3) color_multiply: vec4<f32>,
    @interpolate(flat)
    @location(4) color_add: vec4<f32>,
}

const VERTEX_POSITIONS = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
    instance: TextInstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    let local_uv = VERTEX_POSITIONS[in_vertex_index];
    out.tex_coords = instance.uv_offset + local_uv * instance.uv_size;

    let pixel_pos = instance.position + local_uv * instance.size;

    let ndc_x = (pixel_pos.x / global_uniforms.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (pixel_pos.y / global_uniforms.screen_size.y) * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.texture_index = instance.texture_index;
    out.opacity = instance.opacity;
    out.color_multiply = instance.color_multiply;
    out.color_add = instance.color_add;

    return out;
}

// Fragment shader

// サイズ同じ難で配列にできるな
@group(1) @binding(0)
var t_texture0: texture_2d<f32>;
@group(2) @binding(0)
var s_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var tex_color: vec4<f32>;

    switch in.texture_index {
        case 0 { tex_color = textureSample(t_texture0, s_sampler, in.tex_coords); }
        default {
            discard;
            break;
        }
    }
    // 全てのテクスチャをカラーフィルター対象にする
    // 状態による色替えや無効表現が必要なので
    tex_color = vec4<f32>(
            tex_color.rgb * in.color_multiply.rgb + in.color_add.rgb,
            tex_color.a * in.color_multiply.a + in.color_add.a,
        );

    return vec4<f32>(tex_color.rgb, tex_color.a * in.opacity);
}
