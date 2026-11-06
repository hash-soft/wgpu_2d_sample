use wgpu::util::DeviceExt;

use crate::sub::texture::Texture;

/// # どういう使い方をするか
/// - インスタンスをウィンドウごとに割り当てる
/// - 深度バッファを書き込むのでインスタンスの前方が全面になる
/// - なので後ろからインスタンスを割り当てる必要がある
/// - ウィンドウスキンのタイプにより描画方法を変えることができるようにする
/// - ウィンドウスキンは1テクスチャなのでカーソルも含めておく必要がある
///
/// # ウィンドウ枠
/// - ４隅４辺でウィンドウ枠を作る
/// - 区切り線を持たせることもできる
/// - どの位置から線が必要かわからないので１線3インスタンスが無難
///
/// # ウィンドウ背景
/// - 下地とその上のタイリングを持つ
/// - 下地は拡大縮小の１インスタンス
/// - タイリングは等倍で敷き詰めるがシェーダ側でなんとかして1インスタンスにしたい
///
/// # カーソル
/// - 基本なしか1つだが複数カーソルにも対応しておきたい
/// - そうするとその分のインスタンスが増えることになる
///

/// こっちは背景のタイリングを持つのでtextと統一できない
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FrameInstance {
    pub position: [f32; 2],  // 表示ピクセル位置
    pub size: [f32; 2],      // 表示ピクセルサイズ
    pub uv_offset: [f32; 2], // 切り出しUVオフセット
    pub uv_size: [f32; 2],   // 切り出しUVサイズ
    pub texture_index: u32,
    pub opacity: f32, // 透過度
    pub z: f32,
    pub color_multiply: [f32; 4],
    pub color_add: [f32; 4],
}

impl FrameInstance {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<FrameInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // location(0): position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // location(1): size
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // location(2): uv_offset
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // location(3): uv_size
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // location(4): texture_index
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Uint32,
                },
                // location(5): opacity
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 9]>() as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32,
                },
                // location(6): z
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 10]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32,
                },
                // location(7): color_multiply
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 11]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // location(8): color_add
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 15]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniforms {
    logical_size: [f32; 2],
}

pub struct FrameRenderer {
    #[allow(dead_code)]
    device: wgpu::Device,
    #[allow(dead_code)]
    queue: wgpu::Queue,
    render_pipeline: wgpu::RenderPipeline, // 転送用パイプライン
    uniform_bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    texture: Texture,
    texture_bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    sampler: wgpu::Sampler,
    sampler_bind_group: wgpu::BindGroup,
    frames: Vec<FrameInstance>,
    frame_instance_buffer: wgpu::Buffer,
    start_points: Vec<u32>,
}

impl FrameRenderer {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        logical_size: (f32, f32),
        frame_bytes: &[u8],
        format: wgpu::TextureFormat,
        initial_window_count: usize,
    ) -> anyhow::Result<Self> {
        let logical_size = [logical_size.0, logical_size.1];

        // グローバル
        let uniform_bind_group_layout = Self::create_uniform_bind_group_layout(&device);
        let uniform_buffer = Self::create_uniform_buffer(&device, &logical_size);
        let uniform_bind_group =
            Self::create_uniform_bind_group(&device, &uniform_bind_group_layout, &uniform_buffer);

        // テクスチャー
        let texture_bind_group_layout = Self::create_texture_bind_group_layout(&device);
        // ウィンドウスキンは１枚だけ
        let texture = Texture::from_bytes(&device, &queue, frame_bytes, format, "Texture 2D")?;
        let texture_bind_group =
            Self::create_texture_bind_group(&device, &texture_bind_group_layout, &texture);

        // サンプラー
        let sampler_bind_group_layout = Self::create_sampler_bind_group_layout(&device);
        let sampler = Texture::create_sampler(&device);
        let sampler_bind_group =
            Self::create_sampler_bind_group(&device, &sampler_bind_group_layout, &sampler);

        // パイプライン
        let render_pipeline = Self::create_pipeline(
            &device,
            format,
            &texture_bind_group_layout,
            &sampler_bind_group_layout,
        );

        // フレームのインスタンス
        let initial_frame_count = initial_window_count * 10; // カーソルの数が可変だが1個で試算
        let frames = vec![FrameInstance::default(); initial_frame_count];
        let frame_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Text Instance Buffer"),
            size: (std::mem::size_of::<FrameInstance>() * initial_frame_count)
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false, // 作成時には書き込まない
        });
        let start_points: Vec<u32> = Vec::with_capacity(initial_window_count);

        Ok(Self {
            device: device,
            queue: queue,
            render_pipeline,
            uniform_bind_group,
            texture,
            texture_bind_group,
            sampler,
            sampler_bind_group,
            frames,
            frame_instance_buffer,
            start_points,
        })
    }

    pub fn render(&mut self, render_pass: &mut wgpu::RenderPass<'_>) {
        if self.frames.is_empty() {
            return;
        }
        let start_index = if let Some(last) = self.start_points.last() {
            *last
        } else {
            return;
        };
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        render_pass.set_bind_group(1, &self.texture_bind_group, &[]);
        render_pass.set_bind_group(2, &self.sampler_bind_group, &[]);

        // ここがやっかい
        // 後ろから追加していくので start = end - 描画frame数, end = self.frames.len() となる
        // vertexは全部セットしてdrawで範囲を調整のほうがわかりやすそうなのでそうする
        render_pass.set_vertex_buffer(0, self.frame_instance_buffer.slice(..));
        render_pass.draw(0..4, start_index..self.frames.len() as u32);
    }

    fn create_uniform_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("frame_renderer_uniform_bind_group_layout"),
        })
    }

    /// テクスチャがフレームより大きい場合はみ出す対応はしていない
    pub fn push_window(&mut self, position: &(f32, f32), frame_size: &(f32, f32), z: f32) {
        let start_index = if let Some(last) = self.start_points.last() {
            *last as usize
        } else {
            self.frames.len() - 9
        };
        let slice = &mut self.frames[start_index as usize..start_index + 9];
        let tex_size = (self.texture.texture.width(), self.texture.texture.height());
        let tex_index = 0;
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
        instance.opacity = 1.0;
        instance.z = z;
        instance.color_multiply = [1.0; 4];

        let instance = &mut slice[1];
        instance.position = [position.0 + frame_size.0 - tex_width as f32, position.1];
        instance.size = [tex_width as f32, tex_height as f32];
        instance.uv_offset = [2.0 / 3.0, 0.0];
        instance.uv_size = uv_size;
        instance.texture_index = tex_index;
        instance.opacity = 1.0;
        instance.z = z;
        instance.color_multiply = [1.0; 4];

        let instance = &mut slice[2];
        instance.position = [position.0, position.1 + frame_size.1 - tex_height as f32];
        instance.size = [tex_width as f32, tex_height as f32];
        instance.uv_offset = [0.0, 2.0 / 3.0];
        instance.uv_size = uv_size;
        instance.texture_index = tex_index;
        instance.opacity = 1.0;
        instance.z = z;
        instance.color_multiply = [1.0; 4];

        let instance = &mut slice[3];
        instance.position = [
            position.0 + frame_size.0 - tex_width as f32,
            position.1 + frame_size.1 - tex_height as f32,
        ];
        instance.size = [tex_width as f32, tex_height as f32];
        instance.uv_offset = [2.0 / 3.0, 2.0 / 3.0];
        instance.uv_size = uv_size;
        instance.texture_index = tex_index;
        instance.opacity = 1.0;
        instance.z = z;
        instance.color_multiply = [1.0; 4];

        // 四辺
        let instance = &mut slice[4];
        instance.position = [position.0 + tex_width as f32, position.1];
        instance.size = [frame_size.0 - (tex_width * 2) as f32, tex_height as f32];
        instance.uv_offset = [1.0 / 3.0, 0.0];
        instance.uv_size = uv_size;
        instance.texture_index = tex_index;
        instance.opacity = 1.0;
        instance.z = z;
        instance.color_multiply = [1.0; 4];

        let instance = &mut slice[5];
        instance.position = [position.0, position.1 + tex_height as f32];
        instance.size = [tex_width as f32, frame_size.1 - (tex_height * 2) as f32];
        instance.uv_offset = [0.0, 1.0 / 3.0];
        instance.uv_size = uv_size;
        instance.texture_index = tex_index;
        instance.opacity = 1.0;
        instance.z = z;
        instance.color_multiply = [1.0; 4];

        let instance = &mut slice[6];
        instance.position = [
            position.0 + tex_width as f32,
            position.1 + frame_size.1 - tex_height as f32,
        ];
        instance.size = [frame_size.0 - (tex_width * 2) as f32, tex_height as f32];
        instance.uv_offset = [1.0 / 3.0, 2.0 / 3.0];
        instance.uv_size = uv_size;
        instance.texture_index = tex_index;
        instance.opacity = 1.0;
        instance.z = z;
        instance.color_multiply = [1.0; 4];

        let instance = &mut slice[7];
        instance.position = [
            position.0 + frame_size.0 - tex_width as f32,
            position.1 + tex_height as f32,
        ];
        instance.size = [tex_width as f32, frame_size.1 - (tex_height * 2) as f32];
        instance.uv_offset = [2.0 / 3.0, 1.0 / 3.0];
        instance.uv_size = uv_size;
        instance.texture_index = tex_index;
        instance.opacity = 1.0;
        instance.z = z;
        instance.color_multiply = [1.0; 4];

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
        instance.opacity = 1.0;
        instance.z = z;
        instance.color_multiply = [1.0; 4];

        self.start_points.push(start_index as u32);
        self.queue.write_buffer(
            &self.frame_instance_buffer,
            (std::mem::size_of::<FrameInstance>() * start_index) as wgpu::BufferAddress,
            bytemuck::cast_slice(slice),
        )
    }

    fn create_uniform_buffer(device: &wgpu::Device, logical_size: &[f32; 2]) -> wgpu::Buffer {
        let uniform = FrameUniforms {
            logical_size: *logical_size,
        };
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Filter Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        })
    }

    fn create_uniform_bind_group(
        device: &wgpu::Device,
        uniform_bind_group_layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        })
    }

    fn create_texture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        let mut texture_entries = Vec::with_capacity(1);
        for i in 0..1 {
            texture_entries.push(wgpu::BindGroupLayoutEntry {
                binding: i as u32,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                },
                count: None,
            });
        }

        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &texture_entries,
            label: Some("frame_renderer_texture_bind_group_layout"),
        })
    }

    fn create_texture_bind_group(
        device: &wgpu::Device,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        texture: &Texture,
    ) -> wgpu::BindGroup {
        let mut group_entries = Vec::with_capacity(1);
        group_entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&texture.view),
        });

        // 共通で持っている空テクスチャを入れる
        // while group_entries.len() < 1 {
        //     group_entries.push(wgpu::BindGroupEntry {
        //         binding: group_entries.len() as u32,
        //         resource: wgpu::BindingResource::TextureView(&empty_texture.view),
        //     });
        // }

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &group_entries,
            label: Some("texture_bind_group"),
        })
    }

    fn create_sampler_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering), // Filteringにしてけばsamplerのほうを使ってくれるらしいが未確認
                count: None,
            }],
            label: Some("sampler_bind_group_layout"),
        })
    }

    fn create_sampler_bind_group(
        device: &wgpu::Device,
        sampler_bind_group_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: sampler_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(sampler),
            }],
            label: Some("sampler_bind_group"),
        })
    }

    /// フレームを転送するだけものもの
    /// シェーダーバッファは外から渡すようにする
    fn create_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        sampler_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        // textと同じシェーダーを使っている
        let shader_buffer = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Frame Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shader_text.wgsl").into()),
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("text_renderer_uniform_bind_group_layout"),
            });

        let rende_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Frame Render Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&uniform_bind_group_layout),
                    Some(texture_bind_group_layout),
                    Some(sampler_bind_group_layout),
                ],
                immediate_size: 0,
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Render Pipeline"),
            layout: Some(&rende_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_buffer,
                entry_point: Some("vs_main"),
                buffers: &[Some(FrameInstance::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_buffer,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true), // 書き込みを行う
                depth_compare: Some(wgpu::CompareFunction::Less), // 手前（小さい値）にあるピクセルを描画
                stencil: wgpu::StencilState::default(),           // ステンシルは使用しない
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });
        render_pipeline
    }
}
