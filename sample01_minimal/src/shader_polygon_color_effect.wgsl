// vec3fの下にf32をはさんでいるのは、vec3fは16バイト境界に揃える必要があるため
struct FillStyle {
    base_color: vec3f, // ベースの薄い色
    line_count: f32, // 画面内の横線の本数（可変）
    line_color: vec3f, // 線の濃い色
    line_thickness: f32, // 線の太さ比率（0.0 〜 1.0、可変）
}

// 画面全体で共通のデータ（画面サイズなど）
struct GlobalUniforms {
    screen_size: vec2<f32>,
    _padding: vec2<f32>, // 16バイト境界に揃えるためのダミー
    fill_style: array<FillStyle, 2>, // 塗りつぶしスタイル
}

@group(0) @binding(0)
var<uniform> global_uniforms: GlobalUniforms;

// スプライトごとの個別データ（インスタンス入力）
struct SpriteInstanceInput {
    @location(0) display_position: vec2<f32>, // 画面上のピクセル位置 (x, y)
    @location(1) display_size: vec2<f32>, // 画面上の表示サイズ (width, height)
    @location(2) fill_style_index: u32, // 塗りつぶしスタイルのインデックス
    @location(3) scroll_offset: f32, // スクロール位置
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @interpolate(flat)
    @location(1) fill_style_index: u32,
    @interpolate(flat)
    @location(2) scroll_offset: f32,
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
    instance: SpriteInstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    let local_uv = VERTEX_POSITIONS[in_vertex_index];

    // ピクセル位置と画像サイズからスクリーン上のピクセル座標を計算
    let pixel_pos = instance.display_position + local_uv * instance.display_size;

    // ピクセル座標 (0 ~ screen_size) を NDC 座標 (-1.0 ~ 1.0) に変換
    // X: 0 -> -1.0, width -> 1.0
    // Y: 0 -> 1.0 (上), height -> -1.0 (下)
    let ndc_x = (pixel_pos.x / global_uniforms.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (pixel_pos.y / global_uniforms.screen_size.y) * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = local_uv;
    out.uv.y = pixel_pos.y / global_uniforms.screen_size.y;
    out.fill_style_index = instance.fill_style_index;
    out.scroll_offset = instance.scroll_offset;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let style = global_uniforms.fill_style[in.fill_style_index];
    let moving_y = in.uv.y - in.scroll_offset;
    // ぼかし幅（0.01 だとクッキリ、0.1 だとフワッとしたソフトエッジ）
    let blur = 0.1;
    let half_thickness = style.line_thickness * 0.5;
    // 線の開始位置の調整
    // start_offset = 0 なら中心
    //let start_offset = 0.5 - half_thickness; // 上部
    let start_offset = 0.5; // 上部に半分はみ出し
    // 線の本数を掛けて周期化（0.0 〜 1.0 を繰り返す）
    let cell_y = fract(moving_y * style.line_count + start_offset) - 0.5;
    // 線の中心からの距離（0.0 が中心、0.5 が外側）
    let dist = abs(cell_y);
    let line_factor = 1.0 - smoothstep(half_thickness - blur, half_thickness + blur, dist);
    // 4. ベース色と線の色を合成
    let final_color = mix(style.base_color, style.line_color, line_factor);
    return vec4<f32>(final_color, 1.0);
}
