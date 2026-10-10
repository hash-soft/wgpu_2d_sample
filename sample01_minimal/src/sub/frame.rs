use crate::sub::sprite::SpriteInstance;

/// テクスチャがフレームより大きい場合はみ出す対応はしていない
pub fn make_frame_sprites(
    slice: &mut [SpriteInstance],
    position: &(f32, f32),
    frame_size: &(f32, f32),
    tex_size: &(u32, u32),
    tex_index: u32,
) {
    let uv_size = [1.0 / 3.0, 1.0 / 3.0];
    let tex_width = tex_size.0 / 3;
    let tex_height = tex_size.1 / 3;
    // 四隅
    let instance = &mut slice[0];
    instance.position = [position.0, position.1];
    instance.size = [tex_width as f32, tex_height as f32];
    instance.uv_offset = [0.0, 0.0];
    instance.uv_size = uv_size;
    instance.texture_index = tex_index;

    let instance = &mut slice[1];
    instance.position = [position.0 + frame_size.0 - tex_width as f32, position.1];
    instance.size = [tex_width as f32, tex_height as f32];
    instance.uv_offset = [2.0 / 3.0, 0.0];
    instance.uv_size = uv_size;
    instance.texture_index = tex_index;

    let instance = &mut slice[2];
    instance.position = [position.0, position.1 + frame_size.1 - tex_height as f32];
    instance.size = [tex_width as f32, tex_height as f32];
    instance.uv_offset = [0.0, 2.0 / 3.0];
    instance.uv_size = uv_size;
    instance.texture_index = tex_index;

    let instance = &mut slice[3];
    instance.position = [
        position.0 + frame_size.0 - tex_width as f32,
        position.1 + frame_size.1 - tex_height as f32,
    ];
    instance.size = [tex_width as f32, tex_height as f32];
    instance.uv_offset = [2.0 / 3.0, 2.0 / 3.0];
    instance.uv_size = uv_size;
    instance.texture_index = tex_index;

    // 四辺
    let instance = &mut slice[4];
    instance.position = [position.0 + tex_width as f32, position.1];
    instance.size = [frame_size.0 - (tex_width * 2) as f32, tex_height as f32];
    instance.uv_offset = [1.0 / 3.0, 0.0];
    instance.uv_size = uv_size;
    instance.texture_index = tex_index;

    let instance = &mut slice[5];
    instance.position = [position.0, position.1 + tex_height as f32];
    instance.size = [tex_width as f32, frame_size.1 - (tex_height * 2) as f32];
    instance.uv_offset = [0.0, 1.0 / 3.0];
    instance.uv_size = uv_size;
    instance.texture_index = tex_index;

    let instance = &mut slice[6];
    instance.position = [
        position.0 + tex_width as f32,
        position.1 + frame_size.1 - tex_height as f32,
    ];
    instance.size = [frame_size.0 - (tex_width * 2) as f32, tex_height as f32];
    instance.uv_offset = [1.0 / 3.0, 2.0 / 3.0];
    instance.uv_size = uv_size;
    instance.texture_index = tex_index;

    let instance = &mut slice[7];
    instance.position = [
        position.0 + frame_size.0 - tex_width as f32,
        position.1 + tex_height as f32,
    ];
    instance.size = [tex_width as f32, frame_size.1 - (tex_height * 2) as f32];
    instance.uv_offset = [2.0 / 3.0, 1.0 / 3.0];
    instance.uv_size = uv_size;
    instance.texture_index = tex_index;

    // 中央
    let instance = &mut slice[8];
    instance.position = [
        position.0 + tex_width as f32,
        position.1 + tex_height as f32,
    ];
    instance.size = [
        frame_size.0 - (tex_width * 2) as f32,
        frame_size.1 - (tex_height * 2) as f32,
    ];
    instance.uv_offset = [1.0 / 3.0, 1.0 / 3.0];
    instance.uv_size = uv_size;
    instance.texture_index = tex_index;
}
