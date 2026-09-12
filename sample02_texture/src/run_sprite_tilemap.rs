/// スプライトと同じ仕組みでタイルマップを実現する
/// 単にスプライトを並べるだけ
use std::{iter, sync::Arc};
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::{key::InputState, texture::Texture};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileInstance {
    pub position: [f32; 2],  // 表示ピクセル位置
    pub size: [f32; 2],      // 表示ピクセルサイズ
    pub uv_offset: [f32; 2], // 切り出しUVオフセット
    pub uv_size: [f32; 2],   // 切り出しUVサイズ
    pub texture_index: u32,
}

impl TileInstance {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TileInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance, // ★ここを Instance にする
            attributes: &[
                // location(0): sprite_position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // location(1): sprite_size
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
            ],
        }
    }
}

pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    textures: Vec<Texture>,
    #[allow(dead_code)]
    sampler: wgpu::Sampler,
    texture_bind_group: wgpu::BindGroup,
    uniform_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    map_data: Vec<u32>,
    tiles: Vec<TileInstance>,
    input: InputState,
    window: Arc<Window>,
}

impl State {
    async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: true,
            })
            .await
            .unwrap();

        let limits = adapter.limits();
        // サンプラー1枚+テクスチャ8枚
        // スプライトのほうが多いからスプライトのレイアウトを使いまわせばいいか
        let max_sampled_textures = Texture::request_max_sampled_textures(&limits, 9);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("tilemap_shader"),
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits {
                    max_sampled_textures_per_shader_stage: max_sampled_textures,
                    ..wgpu::Limits::default()
                },
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::default(),
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };

        // 複数テクスチャのロードと準備
        let empty_texture = Texture::create_empty_texture(&device, &queue);
        let world_bytes: &[u8] = include_bytes!("image/tilesets/world.png");
        let upper_bytes: &[u8] = include_bytes!("image/tilesets/upper.png");
        let array_bytes = vec![world_bytes, upper_bytes]; // テクスチャを増やす場合は要素を追加

        let mut textures = Vec::with_capacity(array_bytes.len());
        for bytes in array_bytes.iter() {
            let texture = Texture::from_bytes(&device, &queue, bytes, "Texture 2D")?;
            textures.push(texture);
        }
        let sampler = Texture::create_sampler(&device);

        let max_sampled_textures: u32 = 3;
        let texture_count = max_sampled_textures.saturating_sub(1);

        let mut layout_entries = Vec::with_capacity(max_sampled_textures as usize);
        for i in 0..texture_count {
            layout_entries.push(wgpu::BindGroupLayoutEntry {
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
        layout_entries.push(wgpu::BindGroupLayoutEntry {
            binding: texture_count,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
            count: None,
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &layout_entries,
                label: Some("texture_bind_group_layout"),
            });

        let mut group_entries = Vec::with_capacity(max_sampled_textures as usize);
        for (i, texture) in textures.iter().enumerate() {
            group_entries.push(wgpu::BindGroupEntry {
                binding: i as u32,
                resource: wgpu::BindingResource::TextureView(&texture.view),
            });
        }
        while group_entries.len() < texture_count as usize {
            group_entries.push(wgpu::BindGroupEntry {
                binding: group_entries.len() as u32,
                resource: wgpu::BindingResource::TextureView(&empty_texture.view),
            });
        }
        group_entries.push(wgpu::BindGroupEntry {
            binding: texture_count,
            resource: wgpu::BindingResource::Sampler(&sampler),
        });

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &group_entries,
            label: Some("texture_bind_group"),
        });

        let map_width = 100u32;
        let map_height = 100u32;
        let mut map_data = vec![0u32; (map_width * map_height) as usize];
        // サンプルとして格子模様のマップを作成
        for y in 0..map_height {
            for x in 0..map_width {
                let idx = (y * map_width + x) as usize;
                // テクスチャインデックスを上位16ビット、タイルIDを下位16ビットにパックする
                let (tex_index, tile_id) = if (x + y) % 2 == 0 {
                    // 市松模様の片方: テクスチャ0 の タイル0
                    (0, 0)
                } else {
                    // 市松模様のもう片方: テクスチャ1 の タイル9
                    (1, 9)
                };

                // もし複数のテクスチャをロードした場合、tex_index = 1 などと設定できる
                map_data[idx] = (tex_index << 16) | (tile_id & 0xFFFF);
            }
        }

        // ユニフォームグループレイアウト
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX, // 頂点シェーダーで参照するため
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("uniform_bind_group_layout"),
            });

        // ユニフォームデータ
        // シェーダーの共通データ
        let uniform: Uniforms = Uniforms {
            screen_size: [size.width as f32, size.height as f32],
        };

        // uniformバッファ
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });

        // WGSLシェーダーコードの動的構築
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Tilemap Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader_sprite_tilemap.wgsl").into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&texture_bind_group_layout),
                    Some(&uniform_bind_group_layout),
                ],
                immediate_size: 0,
            });

        let mut tiles = vec![];
        for x in 0..25 {
            for y in 0..20 {
                let tile = map_data[(y * map_width + x) as usize];
                let texture_index = tile >> 16;
                let texture_id = tile & 0xFFFF;
                let texture = &textures[texture_index as usize];
                let x_count = texture.texture.width() / 32;
                //println!("{} {} {}", x, y, texture_id);
                tiles.push(TileInstance {
                    position: [(x * 32) as f32, (y * 32) as f32],
                    size: [32.0, 32.0],
                    uv_offset: [
                        ((texture_id % x_count) * 32) as f32 / texture.texture.width() as f32,
                        ((texture_id / x_count) * 32) as f32 / texture.texture.height() as f32,
                    ],
                    uv_size: [
                        32.0 / texture.texture.width() as f32,
                        32.0 / texture.texture.height() as f32,
                    ],
                    texture_index,
                });
            }
        }

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&tiles),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(TileInstance::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            render_pipeline,
            textures,
            sampler,
            texture_bind_group,
            uniform_bind_group,
            instance_buffer,
            map_data,
            tiles,
            input: InputState::default(),
            window,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: KeyCode, pressed: bool) {
        match (key, pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            _ => {
                self.input.process_key(key, pressed);
            }
        }
    }

    fn update(&mut self) {
        //let speed = 5.0;
        let [dx, dy] = self.input.direction();

        if dx != 0.0 || dy != 0.0 {
            // self.camera_pos.x += dx * speed;
            // self.camera_pos.y += dy * speed;
        }
    }

    fn render(&mut self) -> anyhow::Result<()> {
        self.window.request_redraw();

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                anyhow::bail!("Lost device");
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.texture_bind_group, &[]);
            render_pass.set_bind_group(1, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.tiles.len() as u32);
        }

        self.queue.submit(iter::once(encoder.finish()));
        self.queue.present(output);

        Ok(())
    }
}

pub struct App {
    state: Option<State>,
}

impl App {
    pub fn new() -> Self {
        Self { state: None }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_inner_size(winit::dpi::PhysicalSize::new(800, 600))
            .with_visible(false);
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        window.set_visible(true);

        self.state = Some(pollster::block_on(State::new(window)).unwrap());
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(canvas) => canvas,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                state.update();
                match state.render() {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("{e}");
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => state.handle_key(event_loop, code, key_state.is_pressed()),
            _ => {}
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}
