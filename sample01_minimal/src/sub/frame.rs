use std::ops::Deref;

use crate::sub::sprite::SpriteInstance;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FrameInstance {
    pub base: SpriteInstance,
    pub z: f32,
}

impl Deref for FrameInstance {
    type Target = SpriteInstance;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl FrameInstance {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<FrameInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                }, // pos
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                }, // size
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                }, // uv_off
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                }, // uv_size
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Uint32,
                }, // tex
                wgpu::VertexAttribute {
                    offset: 36,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32,
                }, // opacity
                wgpu::VertexAttribute {
                    offset: 40,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32,
                }, // z
            ],
        }
    }
}

/// テクスチャがフレームより大きい場合はみ出す対応はしていない
pub fn make_frame_sprites(
    slice: &mut [FrameInstance],
    position: &(f32, f32),
    frame_size: &(f32, f32),
    tex_size: &(u32, u32),
    tex_index: u32,
    z: f32,
) {
    let uv_size = [1.0 / 3.0, 1.0 / 3.0];
    let tex_width = tex_size.0 / 3;
    let tex_height = tex_size.1 / 3;
    // 四隅
    let instance = &mut slice[0];
    let base = &mut instance.base;
    base.position = [position.0, position.1];
    base.size = [tex_width as f32, tex_height as f32];
    base.uv_offset = [0.0, 0.0];
    base.uv_size = uv_size;
    base.texture_index = tex_index;
    base.opacity = 1.0;
    instance.z = z;

    let instance = &mut slice[1];
    let base = &mut instance.base;
    base.position = [position.0 + frame_size.0 - tex_width as f32, position.1];
    base.size = [tex_width as f32, tex_height as f32];
    base.uv_offset = [2.0 / 3.0, 0.0];
    base.uv_size = uv_size;
    base.texture_index = tex_index;
    base.opacity = 1.0;
    instance.z = z;

    let instance = &mut slice[2];
    let base = &mut instance.base;
    base.position = [position.0, position.1 + frame_size.1 - tex_height as f32];
    base.size = [tex_width as f32, tex_height as f32];
    base.uv_offset = [0.0, 2.0 / 3.0];
    base.uv_size = uv_size;
    base.texture_index = tex_index;
    base.opacity = 1.0;
    instance.z = z;

    let instance = &mut slice[3];
    let base = &mut instance.base;
    base.position = [
        position.0 + frame_size.0 - tex_width as f32,
        position.1 + frame_size.1 - tex_height as f32,
    ];
    base.size = [tex_width as f32, tex_height as f32];
    base.uv_offset = [2.0 / 3.0, 2.0 / 3.0];
    base.uv_size = uv_size;
    base.texture_index = tex_index;
    base.opacity = 1.0;
    instance.z = z;

    // 四辺
    let instance = &mut slice[4];
    let base = &mut instance.base;
    base.position = [position.0 + tex_width as f32, position.1];
    base.size = [frame_size.0 - (tex_width * 2) as f32, tex_height as f32];
    base.uv_offset = [1.0 / 3.0, 0.0];
    base.uv_size = uv_size;
    base.texture_index = tex_index;
    base.opacity = 1.0;
    instance.z = z;

    let instance = &mut slice[5];
    let base = &mut instance.base;
    base.position = [position.0, position.1 + tex_height as f32];
    base.size = [tex_width as f32, frame_size.1 - (tex_height * 2) as f32];
    base.uv_offset = [0.0, 1.0 / 3.0];
    base.uv_size = uv_size;
    base.texture_index = tex_index;
    base.opacity = 1.0;
    instance.z = z;

    let instance = &mut slice[6];
    let base = &mut instance.base;
    base.position = [
        position.0 + tex_width as f32,
        position.1 + frame_size.1 - tex_height as f32,
    ];
    base.size = [frame_size.0 - (tex_width * 2) as f32, tex_height as f32];
    base.uv_offset = [1.0 / 3.0, 2.0 / 3.0];
    base.uv_size = uv_size;
    base.texture_index = tex_index;
    base.opacity = 1.0;
    instance.z = z;

    let instance = &mut slice[7];
    let base = &mut instance.base;
    base.position = [
        position.0 + frame_size.0 - tex_width as f32,
        position.1 + tex_height as f32,
    ];
    base.size = [tex_width as f32, frame_size.1 - (tex_height * 2) as f32];
    base.uv_offset = [2.0 / 3.0, 1.0 / 3.0];
    base.uv_size = uv_size;
    base.texture_index = tex_index;
    base.opacity = 1.0;
    instance.z = z;

    // 中央
    let instance = &mut slice[8];
    let base = &mut instance.base;
    base.position = [
        position.0 + tex_width as f32,
        position.1 + tex_height as f32,
    ];
    base.size = [
        frame_size.0 - (tex_width * 2) as f32,
        frame_size.1 - (tex_height * 2) as f32,
    ];
    base.uv_offset = [1.0 / 3.0, 1.0 / 3.0];
    base.uv_size = uv_size;
    base.texture_index = tex_index;
    base.opacity = 1.0;
    instance.z = z;
}
