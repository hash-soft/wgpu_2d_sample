// ノイズフィルターの最小構成シェーダー

struct GlobalUniforms {
    screen_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> global_uniforms: GlobalUniforms;

struct SpriteInstanceInput {
    @location(0) display_position: vec2<f32>,
    @location(1) display_size: vec2<f32>,
    @location(2) uv_offset: vec2<f32>,
    @location(3) uv_size: vec2<f32>,
    @location(4) texture_index: u32,
    @location(5) opacity: f32,
    @location(6) noise_strength: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) @interpolate(flat) texture_index: u32,
    @location(2) @interpolate(flat) opacity: f32,
    @location(3) @interpolate(flat) noise_strength: f32,
}

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
    out.tex_coords = instance.uv_offset + local_uv * instance.uv_size;

    let pixel_pos = instance.display_position + local_uv * instance.display_size;

    let ndc_x = (pixel_pos.x / global_uniforms.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (pixel_pos.y / global_uniforms.screen_size.y) * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.texture_index = instance.texture_index;
    out.opacity = instance.opacity;
    out.noise_strength = instance.noise_strength;

    return out;
}

// 疑似乱数生成関数（uvをシードにしてノイズを作る）
fn rand(co: vec2<f32>) -> f32 {
    return fract(sin(dot(co, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@group(1) @binding(0) var t_texture0: texture_2d<f32>;
@group(2) @binding(0) var s_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var tex_color: vec4<f32>;

    switch in.texture_index {
        case 0: { tex_color = textureSample(t_texture0, s_sampler, in.tex_coords); }
        default: { discard; break; }
    }

    // 現在のUV座標に依存した乱数を生成（0.0 ~ 1.0）
    // 乱数といっても同じ座標なら同じ値になる
    let noise_val = rand(in.tex_coords);
    // 強度に応じて元の色とブレンド
    tex_color = vec4<f32>(
        mix(tex_color.rgb, vec3<f32>(noise_val), in.noise_strength),
        tex_color.a
    );

    return vec4<f32>(tex_color.rgb, tex_color.a * in.opacity);
}
