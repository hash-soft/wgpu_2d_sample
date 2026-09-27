// shader_multipass_pass2.wgsl
// マルチパスブラーサンプル: Pass 2
// オフスクリーンテクスチャ全体を画面に描画しつつ、簡易的なブラーをかける

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

// 画面全体を覆う2つの三角形（頂点バッファなしで生成）
@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // 0: (-1, -1), 1: (3, -1), 2: (-1, 3) 
    // これで画面全体を覆う大きな三角形を1つ描画するテクニック
    let x = f32((in_vertex_index << 1u) & 2u);
    let y = f32(in_vertex_index & 2u);
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, y * -2.0 + 1.0, 0.0, 1.0);
    out.tex_coords = vec2<f32>(x, y);
    return out;
}

@group(0) @binding(0) var t_color: texture_2d<f32>;
@group(0) @binding(1) var s_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 簡易的な5タップ十字ブラー
    let tex_dim = vec2<f32>(textureDimensions(t_color));
    let texel_size = 1.0 / tex_dim;
    var offset = texel_size * 32.0; // ブラーの広がり具合
    // コメントアウトをはずせばブラーなしになる
    //offset.x = 0.0;
    //offset.y = 0.0;

    var color = textureSample(t_color, s_sampler, in.tex_coords) * 0.2;
    color += textureSample(t_color, s_sampler, in.tex_coords + vec2<f32>(offset.x, 0.0)) * 0.2;
    color += textureSample(t_color, s_sampler, in.tex_coords + vec2<f32>(-offset.x, 0.0)) * 0.2;
    color += textureSample(t_color, s_sampler, in.tex_coords + vec2<f32>(0.0, offset.y)) * 0.2;
    color += textureSample(t_color, s_sampler, in.tex_coords + vec2<f32>(0.0, -offset.y)) * 0.2;

    return color;
}
