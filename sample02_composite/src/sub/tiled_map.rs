use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct TiledMap {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
    pub tilewidth: u32,
    pub tileheight: u32,
    pub tilesets: Vec<TileSets>,
}

#[derive(Deserialize, Debug)]
pub struct Layer {
    pub data: Option<Vec<u32>>,
    pub x: u32,
    pub y: u32,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub opacity: f32,
    #[serde(rename = "type")]
    pub layer_type: String, // tilelayerがマップ
}

#[derive(Deserialize, Debug)]
pub struct TileSets {
    pub firstgid: u32,
    pub source: String,
}
