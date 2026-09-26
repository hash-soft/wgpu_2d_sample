// Vertex shader

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) position: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    // 右下 1/4 スペースに表示
    var out: VertexOutput;
    // 反時計回りなので右⇒中央⇒左になるように計算
    let x = f32(-i32(in_vertex_index)) * 0.5 + 1.0;
    let y = f32(i32(in_vertex_index & 1u) - 1);
    out.position = vec2<f32>(x, y);
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

// Fragment shader

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // yがマイナスになるので補正
    return vec4<f32>(in.position.x, in.position.y + 1.0, 0.5, 1.0);
}