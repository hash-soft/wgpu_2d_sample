// sample02_texture/src/run_tilemap_indexed.rs の静的参照用シェーダー
// 頂点バッファとインデックスバッファによる複数四角形描画および四角形ごとの map_base_index 切り替えに対応

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) screen_uv: vec2<f32>,
    @location(2) map_base_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) screen_uv: vec2<f32>,
    @location(1) @interpolate(flat) map_base_index: u32,
}

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(model.position, 0.0, 1.0);
    out.screen_uv = model.screen_uv;
    out.map_base_index = model.map_base_index;
    return out;
}

// Fragment shader
@group(0) @binding(0) var t_texture0: texture_2d<f32>;
@group(0) @binding(1) var t_texture1: texture_2d<f32>;
@group(0) @binding(2) var t_texture2: texture_2d<f32>;
@group(0) @binding(3) var s_sampler: sampler;

struct AtlasInfo {
    atlas_size: vec2<u32>,
    tile_uv_size: vec2<f32>,
}

struct CameraUniform {
    view_proj: mat3x3<f32>,
    map_size: vec2<u32>,
    _padding: vec2<u32>,
    atlases: array<AtlasInfo, 8>,
}

@group(1) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(1) var<storage, read> map_data: array<u32>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos_h = camera.view_proj * vec3<f32>(in.screen_uv, 1.0);
    let world_pos = world_pos_h.xy;

    let tile_pos = vec2<u32>(floor(world_pos));

    if world_pos.x < 0.0 || world_pos.y < 0.0 || 
        tile_pos.x >= camera.map_size.x || tile_pos.y >= camera.map_size.y {
        discard;
    }

    let local_map_index = tile_pos.y * camera.map_size.x + tile_pos.x;
    // 四角形ごとに指定された map_base_index を加算
    let final_map_index = in.map_base_index + local_map_index;
    let map_data_val = map_data[final_map_index];

    // 上位16ビットをテクスチャインデックス、下位16ビットをタイルIDとして解釈
    let tex_index = (map_data_val >> 16u) & 0x7u;
    let tile_id = map_data_val & 0xFFFFu;

    let atlas = camera.atlases[tex_index];

    let local_uv = fract(world_pos);

    let atlas_x = f32(tile_id % atlas.atlas_size.x);
    let atlas_y = f32(tile_id / atlas.atlas_size.x);

    let base_uv = vec2<f32>(atlas_x, atlas_y) * atlas.tile_uv_size;
    let final_uv = base_uv + local_uv * atlas.tile_uv_size;

    switch tex_index {
        case 0u: {
            return textureSample(t_texture0, s_sampler, final_uv);
        }
        case 1u: {
            return textureSample(t_texture1, s_sampler, final_uv);
        }
        case 2u: {
            return textureSample(t_texture2, s_sampler, final_uv);
        }
        default: {
            discard;
        }
    }
}
