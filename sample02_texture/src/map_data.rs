use std::collections::HashMap;

use crate::tiled_map::TiledMap;

#[derive(Debug)]
pub struct MapData {
    pub tile_width: u32,
    pub tile_height: u32,
    pub data: Vec<u32>,
    pub layers: Vec<MapLayer>,
}

#[derive(Debug)]
pub struct MapLayer {
    pub width: u32,
    pub height: u32,
    pub start_index: u32,
}

struct SliceLayer<'a> {
    pub width: u32,
    pub height: u32,
    pub data: &'a [u32],
}

impl MapData {
    pub fn from_tiled_map(tiled_map: &TiledMap) -> Option<Self> {
        let mut slice_layers: Vec<SliceLayer> = Vec::new();
        let mut data_count: u32 = 0;
        // 最初にlayerと総データ数を確定させる
        for layer in &tiled_map.layers {
            if layer.layer_type != "tilelayer" {
                continue;
            }
            let width = layer.width.unwrap_or(0);
            let height = layer.height.unwrap_or(0);
            let data = layer.data.as_deref().unwrap_or(&[]);
            // 無効なデータの場合、スキップ
            if width == 0 || height == 0 || data.is_empty() {
                continue;
            }
            slice_layers.push(SliceLayer {
                width,
                height,
                data,
            });
            data_count += width * height;
        }

        if data_count == 0 {
            return None;
        }

        let mut texture_map = HashMap::new();
        texture_map.insert("image/tilesets/world.png", 0);
        texture_map.insert("image/tilesets/ground.png", 1);
        texture_map.insert("image/tilesets/upper.png", 2);

        let texture_indices: Vec<i32> = tiled_map
            .tilesets
            .iter()
            .map(|tileset| match texture_map.get(&tileset.source.as_str()) {
                Some(&index) => index,
                None => -1,
            })
            .collect();

        let mut data = vec![0; data_count as usize];
        let mut layers = Vec::with_capacity(slice_layers.len());
        let mut start_index = 0;
        for layer in &slice_layers {
            for y in 0..layer.height {
                for x in 0..layer.width {
                    // tilesetsのfirstgidを見てテクスチャを決める必要がある
                    // 実際のゲームではtiledは使わないので決め打ちでいい
                    let src_index = (y * layer.width + x) as usize;
                    let dest_index = src_index + start_index as usize;
                    data[dest_index] =
                        Self::to_tile_data(tiled_map, layer.data[src_index], &texture_indices);
                }
            }
            layers.push(MapLayer {
                width: layer.width,
                height: layer.height,
                start_index,
            });
            start_index += layer.width * layer.height;
        }

        Some(MapData {
            tile_width: tiled_map.tilewidth,
            tile_height: tiled_map.tileheight,
            data,
            layers,
        })
    }

    fn to_tile_data(tiled_map: &TiledMap, chip_id: u32, texture_indices: &Vec<i32>) -> u32 {
        if chip_id == 0 {
            return 1 << 21 | 0xFFFF;
        }

        for i in (0..tiled_map.tilesets.len()).rev() {
            let tileset = &tiled_map.tilesets[i];
            // マイナスにならなければ確定
            // インデックスはテクスチャIdになる
            let mut real_chip_id = match chip_id.checked_sub(tileset.firstgid) {
                Some(result) => result,
                None => continue,
            };
            let mut texture_id = texture_indices[i];
            if texture_id < 0 {
                texture_id = 0;
                real_chip_id = 0xFFFF; // テクスチャがみつからなければ描画しないに倒す
            }
            let texture_id = texture_id as u32;
            // 情報を持ってないのでとりあえず指定テクスチャごとに一定以上の番号ならアニメーションにしておく
            let anim_index = match texture_id {
                0 => {
                    if real_chip_id > 60 {
                        4
                    } else {
                        0
                    }
                }
                1 => {
                    if real_chip_id > 216 {
                        4
                    } else {
                        0
                    }
                }
                2 => 0,
                _ => 0,
            };
            return (texture_id << 28) | (anim_index << 24) | (3 << 21) | (real_chip_id & 0xFFFF);
        }

        // 見つからない場合
        return 1 << 21 | 0xFFFF;
    }
}
