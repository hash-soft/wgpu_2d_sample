
struct AtlasInfo {
    atlas_size: vec2<u32>,
    tile_uv_size: vec2<f32>,
}

// 画面全体で共通のデータ（画面サイズなど）
struct GlobalUniforms {
    view_proj: mat4x4<f32>,
    tile_pixel_size: vec2<f32>,
    pettern: vec2<u32>,
    atlases: array<AtlasInfo, 2>,
}

@group(0) @binding(0) var<uniform> global_uniforms: GlobalUniforms;

// タイルごとの個別データ（インスタンス入力）
struct TileInstanceInput {
    @location(0) world_pos: vec2<i32>, // ワールド座標 (x, y)
    @location(1) tile_data: u32,       // タイルデータ
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,    // 頂点座標
    @location(0) tex_coords: vec2<f32>,             // テクスチャ座標
    @location(1) @interpolate(flat) texture_index: u32,
}

// もう一つ引数増やしてオフセットやアニメーションを別にできるようにするのもいいかもしれない
@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32, instance: TileInstanceInput) -> VertexOutput {
    // PrimitiveTopology::TriangleStrip を指定しているので4頂点で2つの三角形に構成される
    // テクスチャの切り出し位置と表示座標のもとになる
    var pos = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0)
    );

    let tex_index = instance.tile_data >> 28u;
    let anim_index = (instance.tile_data >> 24) & 0xF;
    let pattern = (instance.tile_data >> 21) & 0x7;
    let tile_id = instance.tile_data & 0xFFFFu;
    let atlas = global_uniforms.atlases[tex_index];

    let atlas_x = tile_id % atlas.atlas_size.x + anim_index * (global_uniforms.pettern[0] % pattern);
    let atlas_y = tile_id / atlas.atlas_size.x;
    let uv_offset = vec2<f32>(f32(atlas_x), f32(atlas_y)) * atlas.tile_uv_size;

    var out: VertexOutput;
    // 0.0 ~ 1.0 のローカル UV を指定矩形の UV 範囲にスケーリング・シフト
    out.tex_coords = uv_offset + pos[in_vertex_index] * atlas.tile_uv_size;

    // ピクセル位置と画像サイズからスクリーン上のピクセル座標を計算
    // 拡大縮小前の基本サイズでピクセル位置を計算
    let tile_size = global_uniforms.tile_pixel_size;
    let local_pixel_pos = vec2<f32>(instance.world_pos) * tile_size + pos[in_vertex_index] * tile_size;

    // View-Projection行列でクリップ座標系（NDC）へ一発変換
    out.clip_position = global_uniforms.view_proj * vec4<f32>(local_pixel_pos, 0.0, 1.0);
    out.texture_index = tex_index;

    return out;
}

// Fragment shader
@group(1) @binding(0) var t_texture0: texture_2d<f32>;
@group(1) @binding(1) var t_texture1: texture_2d<f32>;
@group(1) @binding(2) var s_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {

    switch in.texture_index {
        case 0: {
            return textureSample(t_texture0, s_sampler, in.tex_coords);
        }
        case 1: {
            // texture1だけ透明度を50%にしている
            let tex_color = textureSample(t_texture1, s_sampler, in.tex_coords);
            return vec4<f32>(tex_color.rgb, tex_color.a * 0.8);
        }
        default: {
            discard;
        }
    }
}