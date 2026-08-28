// Vertex shader

// 頂点シェーダーの出力を保存する構造体
// vertex > out, fragment > in
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    // 左上　1/4 スペースに表示
    var out: VertexOutput;
    let x = f32(-i32(in_vertex_index)) * 0.5;
    let y = f32(i32(in_vertex_index & 1u));
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

// Fragment shader

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 色変換 ((srgb_color / 255 + 0.055) / 1.055) ^ 2.4
    let c = pow((0.5 + 0.055) / 1.055, 2.4);
    return vec4<f32>(c, 0.0, c, 1.0);
}