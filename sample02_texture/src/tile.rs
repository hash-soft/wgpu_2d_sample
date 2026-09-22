use std::ops::Range;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileInstance {
    pub grid_pos: [i16; 2], // ワールド座標 (論理タイルインデックス) (x, y) インデックス単位なので符号付16bitで十分
    pub tile_data: u32,     // 上位16bit: texture_index, 下位16bit: tile_id
}

impl TileInstance {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TileInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance, // ★ここを Instance にする
            attributes: &[
                // location(0): grid_pos
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Sint16x2,
                },
                // location(1): tile_data
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[i16; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }

    pub fn push_tile(
        tiles: &mut Vec<TileInstance>,
        tile_data: u32,
        x: i32,
        y: i32,
        counts: [u32; 3],
    ) {
        let x = (x * 2) as i16;
        let y = (y * 2) as i16;
        let tex_index = tile_data >> 28;
        let anim_index = (tile_data >> 24) & 0xF;
        let tile_id = tile_data & 0xFFFF;
        let count_x = counts[tex_index as usize];
        let tile_x = (tile_id % count_x) * 2;
        let tile_y = (tile_id / count_x) * 2;
        let count_x2 = count_x * 2;
        let upper = (tile_data & 0xF0FF0000) | ((anim_index * 2) << 24);

        tiles.push(TileInstance {
            grid_pos: [x, y],
            tile_data: upper | (tile_x + tile_y * count_x2),
        });
        tiles.push(TileInstance {
            grid_pos: [x + 1, y],
            tile_data: upper | (tile_x + 1 + tile_y * count_x2),
        });
        tiles.push(TileInstance {
            grid_pos: [x, y + 1],
            tile_data: upper | (tile_x + (tile_y + 1) * count_x2),
        });
        tiles.push(TileInstance {
            grid_pos: [x + 1, y + 1],
            tile_data: upper | (tile_x + 1 + (tile_y + 1) * count_x2),
        });
    }
}

pub struct TileDrawGroup {
    pub vertex_bind_offset: u32, // 頂点bind_groupのoffset
    pub fragment_bind_offset: u32,
    pub opacity: f32,
    pub slice_range: Range<u64>,  // instanceをsliceする範囲
    pub draw_range: Range<u32>,   // drawで指定するinstanceの範囲
    pub tiles: Vec<TileInstance>, // タイルの頂点のもとになるインスタンス
}
