use anyhow::*;
use image::GenericImageView;
use wgpu::Limits;

pub struct Texture {
    #[allow(unused)]
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl Texture {
    /// サンプラー1枚を含む最大のテクスチャ数
    pub fn request_max_sampled_textures(limits: &Limits, limit_texture_count: u32) -> u32 {
        std::cmp::min(
            limit_texture_count,
            limits.max_sampled_textures_per_shader_stage,
        )
    }

    /// テクスチャ1つに対して1つのサンプラーは不要なので使いまわせるようにする
    pub fn create_sampler(device: &wgpu::Device) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        })
    }

    pub fn create_empty_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 透明色 (0, 0, 0, 0) または黒などで初期化
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[0u8, 0u8, 0u8, 0u8],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            size,
        );

        Self { texture, view }
    }

    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
    ) -> Result<Self> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img, Some(label))
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>,
    ) -> Result<Self> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Ok(Self { texture, view })
    }

    pub fn from_array_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        array_bytes: Vec<&[u8]>,
        label: &str,
    ) -> Result<Self> {
        let array_img: Result<Vec<image::DynamicImage>> = array_bytes
            .into_iter()
            .map(|bytes| Ok(image::load_from_memory(bytes)?))
            .collect();
        // ループ版
        // let mut array_img = Vec::new();
        // for bytes in array_bytes {
        //     let img = image::load_from_memory(bytes)?;
        //     array_img.push(img);
        // }
        Self::from_array_image(device, queue, array_img?, Some(label))
    }
    pub fn from_array_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        array_img: Vec<image::DynamicImage>,
        label: Option<&str>,
    ) -> Result<Self> {
        // todo : 全サイズをチェックか外側からExtend3dを設定して渡す
        let dimensions = array_img[0].dimensions();
        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: array_img.len() as u32, // テクスチャの枚数（レイヤー数）
        };
        let array_texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2, // 3Dではなく2Dを指定
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // todo: 1回だけwrite_textureを呼び出す方法は後程
        for (i, img) in array_img.iter().enumerate() {
            let rgba = img.to_rgba8();
            let dimensions = img.dimensions();
            let size = wgpu::Extent3d {
                width: dimensions.0,
                height: dimensions.1,
                depth_or_array_layers: 1,
            };

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    aspect: wgpu::TextureAspect::All,
                    texture: &array_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: i as u32,
                    },
                },
                &rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * dimensions.0),
                    rows_per_image: Some(dimensions.1),
                },
                size,
            );
        }

        let array_texture_view = array_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Texture 2D Array View"),
            format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: Some(array_img.len() as u32), // imageの枚数
        });

        Ok(Self {
            texture: array_texture,
            view: array_texture_view,
        })
    }
}

pub struct DynamicShader {
    pub source: String,
}

impl DynamicShader {
    /// テクスチャ1つに対して1つのサンプラーは不要なので使いまわせるようにする
    pub fn create_multiple_texture(max_sampled_textures: u32) -> Self {
        // let max_allowed = (limits.max_sampled_textures_per_shader_stage as usize).saturating_sub(1); // サンプラー分を1つ引く例
        // // 最大32枚
        // let texture_count = std::cmp::min(32, max_allowed);

        let texture_count = max_sampled_textures.saturating_sub(1);
        println!(
            "max: {}, Dynamic Texture Count: {}",
            max_sampled_textures, texture_count
        );

        // WGSLシェーダーコードの動的構築
        let mut shader_source = String::new();

        // 共通ヘッダー・頂点シェーダー部分の追加
        shader_source.push_str(
            r#"
struct GlobalUniforms {
    screen_size: vec2<f32>,
}

@group(1) @binding(0)
var<uniform> global_uniforms: GlobalUniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct SpriteInstanceInput {
    @location(2) display_position: vec2<f32>,
    @location(3) display_size: vec2<f32>,
    @location(4) uv_offset: vec2<f32>,
    @location(5) uv_size: vec2<f32>,
    @location(6) texture_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) @interpolate(flat) texture_index: u32,
}

@vertex
fn vs_main(model: VertexInput, instance: SpriteInstanceInput) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = instance.uv_offset + model.tex_coords * instance.uv_size;
    let pixel_pos = instance.display_position + model.position * instance.display_size;
    let ndc_x = (pixel_pos.x / global_uniforms.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (pixel_pos.y / global_uniforms.screen_size.y) * 2.0;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.texture_index = instance.texture_index;
    return out;
}
"#,
        );

        // フラグメントシェーダー用のテクスチャ変数宣言を動的に追加
        for i in 0..texture_count {
            shader_source.push_str(&format!(
                "@group(0) @binding({}) var t_texture{}: texture_2d<f32>;\n",
                i, i
            ));
        }
        let sampler_binding_idx = texture_count;
        shader_source.push_str(&format!(
            "@group(0) @binding({}) var s_sampler: sampler;\n",
            sampler_binding_idx
        ));

        // フラグメントシェーダーのメイン関数とswitch文を動的に構築
        shader_source.push_str(
            r#"
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    switch in.texture_index {
"#,
        );

        for i in 1..texture_count {
            shader_source.push_str(&format!(
        "        case {}: {{\n            return textureSample(t_texture{}, s_sampler, in.tex_coords);\n        }}\n",
        i, i
    ));
        }

        shader_source.push_str(&format!(
    "        default: {{\n            return textureSample(t_texture0, s_sampler, in.tex_coords);\n        }}\n"
));

        shader_source.push_str(
            r#"
    }
}
"#,
        );

        DynamicShader {
            source: shader_source,
        }
    }
}
