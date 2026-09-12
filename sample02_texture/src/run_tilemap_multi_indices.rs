use cgmath::{Deg, Matrix3, Vector2};
use std::{iter, sync::Arc};
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::{
    key::InputState,
    texture::{DynamicShader, Texture},
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileVertex {
    pub position: [f32; 2],
    pub screen_uv: [f32; 2],
    pub map_base_index: u32,
}

impl TileVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TileVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // screen_uv
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // map_base_index: 四角形ごとに参照するmap_dataの開始インデックス
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 2]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct AtlasInfo {
    atlas_size: [u32; 2],
    tile_uv_size: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 3],
    map_size: [u32; 2],
    _padding: [u32; 2], // uniformバッファの配列開始オフセットを16の倍数にするためのパディング
    atlases: [AtlasInfo; 32],
}

impl CameraUniform {
    fn new(
        screen_size: Vector2<f32>,
        camera_pos: Vector2<f32>,
        zoom: f32,
        rotation_deg: f32,
        map_size: [u32; 2],
    ) -> Self {
        // スクリーンUVをピクセル単位へ
        let scale_screen = Matrix3::from_nonuniform_scale(screen_size.x, screen_size.y);
        let center_offset =
            Matrix3::from_translation(Vector2::new(-screen_size.x / 2.0, -screen_size.y / 2.0));

        let rot = Matrix3::from_angle_z(Deg(rotation_deg));
        let scale_zoom = Matrix3::from_scale(1.0 / zoom);

        // タイル1枚を32x32ピクセルとする
        let tile_pixel_size = 32.0;
        let trans = Matrix3::from_translation(camera_pos / tile_pixel_size);

        // 最終的な行列 (画面中心を原点として回転ズームし、カメラ位置へ平行移動)
        // さらに全体をタイルのサイズで割る（タイル空間へ変換）
        let tile_scale = Matrix3::from_scale(1.0 / tile_pixel_size);
        let matrix = trans * tile_scale * scale_zoom * rot * center_offset * scale_screen;

        let view_proj = [
            [matrix.x.x, matrix.x.y, matrix.x.z, 0.0],
            [matrix.y.x, matrix.y.y, matrix.y.z, 0.0],
            [matrix.z.x, matrix.z.y, matrix.z.z, 0.0],
        ];

        let mut atlases = [AtlasInfo {
            atlas_size: [1, 1],
            tile_uv_size: [1.0, 1.0],
        }; 32];

        // テクスチャ0: world.png の設定 (12x19)
        atlases[0] = AtlasInfo {
            atlas_size: [12, 19],
            tile_uv_size: [1.0 / 12.0, 1.0 / 19.0],
        };

        // テクスチャ1: upper.png の設定 (16x32)
        atlases[1] = AtlasInfo {
            atlas_size: [16, 32],
            tile_uv_size: [1.0 / 16.0, 1.0 / 32.0],
        };

        Self {
            view_proj,
            map_size,
            _padding: [0, 0],
            atlases,
        }
    }
}

pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,

    texture_bind_group: wgpu::BindGroup,
    uniform_bind_group: wgpu::BindGroup,

    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,

    camera_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    map_buffer: wgpu::Buffer,

    camera_pos: Vector2<f32>,
    zoom: f32,
    rotation: f32,

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
        let max_sampled_textures = Texture::request_max_sampled_textures(&limits, 9);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("tilemap_indexed_shader"),
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
            present_mode: surface_caps.present_modes[1],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };

        // 複数テクスチャのロードと準備
        let empty_texture = Texture::create_empty_texture(&device, &queue);
        let world_bytes: &[u8] = include_bytes!("image/tilesets/world.png");
        let upper_bytes: &[u8] = include_bytes!("image/tilesets/upper.png");
        let array_bytes = vec![world_bytes, upper_bytes];

        let mut textures = Vec::with_capacity(array_bytes.len());
        for bytes in array_bytes.iter() {
            let texture = Texture::from_bytes(&device, &queue, bytes, "Texture 2D")?;
            textures.push(texture);
        }
        let sampler = Texture::create_sampler(&device);

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

        // 3つのマップレイヤー/領域のデータを単一のStorage Bufferに作成
        let map_width = 100u32;
        let map_height = 100u32;
        let map_layer_size = (map_width * map_height) as usize;
        let mut map_data = vec![0u32; map_layer_size * 3];

        // レイヤー0 (ベースインデックス 0): 背景マップ（市松模様）
        for y in 0..map_height {
            for x in 0..map_width {
                let idx = (y * map_width + x) as usize;
                let (tex_index, tile_id) = if (x + y) % 2 == 0 { (0, 0) } else { (1, 9) };
                map_data[idx] = (tex_index << 16) | (tile_id & 0xFFFF);
            }
        }

        // レイヤー1 (ベースインデックス 10,000): 追加の四角形A用（テクスチャ0の水・草原模様）
        for y in 0..map_height {
            for x in 0..map_width {
                let idx = map_layer_size + (y * map_width + x) as usize;
                let tile_id = ((x / 2 + y / 2) % 12) as u32;
                map_data[idx] = (0 << 16) | (tile_id & 0xFFFF);
            }
        }

        // レイヤー2 (ベースインデックス 20,000): 追加の四角形B用（テクスチャ1の装飾マップ）
        for y in 0..map_height {
            for x in 0..map_width {
                let idx = (map_layer_size * 2) + (y * map_width + x) as usize;
                let tile_id = ((x * 3 + y) % 30) as u32;
                map_data[idx] = (1 << 16) | (tile_id & 0xFFFF);
            }
        }

        let map_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Multi Map Storage Buffer"),
            contents: bytemuck::cast_slice(&map_data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // 頂点バッファとインデックスバッファの構築
        // 画面全体の描画に加え、指定の頂点（サブウィンドウ/追加矩形）を複数定義
        let vertices: &[TileVertex] = &[
            // --- 四角形0: 画面全体の背景マップ (map_base_index: 0) ---
            TileVertex {
                position: [-1.0, 1.0],
                screen_uv: [0.0, 0.0],
                map_base_index: 0,
            },
            TileVertex {
                position: [-1.0, -1.0],
                screen_uv: [0.0, 1.0],
                map_base_index: 0,
            },
            TileVertex {
                position: [1.0, 1.0],
                screen_uv: [1.0, 0.0],
                map_base_index: 0,
            },
            TileVertex {
                position: [1.0, -1.0],
                screen_uv: [1.0, 1.0],
                map_base_index: 0,
            },
            // --- 四角形1: 右上の指定頂点矩形 (map_base_index: 10,000) ---
            TileVertex {
                position: [0.3, 0.9],
                screen_uv: [0.0, 0.0],
                map_base_index: 10000,
            },
            TileVertex {
                position: [0.3, 0.3],
                screen_uv: [0.0, 1.0],
                map_base_index: 10000,
            },
            TileVertex {
                position: [0.9, 0.9],
                screen_uv: [1.0, 0.0],
                map_base_index: 10000,
            },
            TileVertex {
                position: [0.9, 0.3],
                screen_uv: [1.0, 1.0],
                map_base_index: 10000,
            },
            // --- 四角形2: 左下の指定頂点矩形 (map_base_index: 20,000) ---
            TileVertex {
                position: [-0.9, -0.3],
                screen_uv: [0.0, 0.0],
                map_base_index: 20000,
            },
            TileVertex {
                position: [-0.9, -0.9],
                screen_uv: [0.0, 1.0],
                map_base_index: 20000,
            },
            TileVertex {
                position: [-0.3, -0.3],
                screen_uv: [1.0, 0.0],
                map_base_index: 20000,
            },
            TileVertex {
                position: [-0.3, -0.9],
                screen_uv: [1.0, 1.0],
                map_base_index: 20000,
            },
        ];

        let indices: &[u16] = &[
            // 四角形0 (全画面背景)
            0, 1, 2, 2, 1, 3, // 四角形1 (右上矩形)
            4, 5, 6, 6, 5, 7, // 四角形2 (左下矩形)
            8, 9, 10, 10, 9, 11,
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tilemap Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tilemap Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let initial_camera = CameraUniform::new(
            Vector2::new(size.width as f32, size.height as f32),
            Vector2::new(50.0 * 32.0, 50.0 * 32.0),
            1.0,
            0.0,
            [map_width, map_height],
        );

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[initial_camera]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: Some("uniform_bind_group_layout"),
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: map_buffer.as_entire_binding(),
                },
            ],
            label: Some("uniform_bind_group"),
        });

        // WGSLシェーダーコードの動的構築 (draw_indexed対応)
        let dynamic_shader = DynamicShader::create_dynamic_tilemap_indexed_shader(texture_count);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Tilemap Indexed Shader"),
            source: wgpu::ShaderSource::Wgsl(dynamic_shader.source.into()),
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

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(TileVertex::desc())],
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
                topology: wgpu::PrimitiveTopology::TriangleList, // 複数四角形を扱いやすい TriangleList
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
            texture_bind_group,
            uniform_bind_group,
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
            camera_buffer,
            map_buffer,
            camera_pos: Vector2::new(50.0 * 32.0, 50.0 * 32.0),
            zoom: 1.0,
            rotation: 0.0,
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
            // Z, X キーでズーム
            (KeyCode::KeyZ, true) => self.zoom += 0.1,
            (KeyCode::KeyX, true) => self.zoom = (self.zoom - 0.1).max(0.1),
            // Q, E キーで回転
            (KeyCode::KeyQ, true) => self.rotation -= 5.0,
            (KeyCode::KeyE, true) => self.rotation += 5.0,
            _ => {
                self.input.process_key(key, pressed);
            }
        }
    }

    fn update(&mut self) {
        let speed = 5.0;
        let [dx, dy] = self.input.direction();

        if dx != 0.0 || dy != 0.0 {
            self.camera_pos.x += dx * speed;
            self.camera_pos.y += dy * speed;
        }

        let camera = CameraUniform::new(
            Vector2::new(800.0, 600.0),
            self.camera_pos,
            self.zoom.max(0.1),
            self.rotation,
            [100, 100],
        );
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[camera]));
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

            // 頂点バッファとインデックスバッファを設定
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            // 全ての四角形（全画面＋追加指定頂点）を一括で draw_indexed 描画
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);

            // ※個別に描画範囲を制御したい場合は以下のようにインデックス範囲を分割指定も可能:
            // render_pass.draw_indexed(0..6, 0, 0..1);   // 全画面背景のみ
            // render_pass.draw_indexed(6..12, 0, 0..1);  // 右上矩形のみ
            // render_pass.draw_indexed(12..18, 0, 0..1); // 左下矩形のみ
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
