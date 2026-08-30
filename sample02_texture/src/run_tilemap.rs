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

use crate::{key::InputState, texture::Texture};

// --- WGSL シェーダーの動的生成 ---
fn create_dynamic_tilemap_shader(texture_count: u32) -> String {
    let mut shader_source = String::new();

    shader_source.push_str(
        r#"
struct AtlasInfo {
    atlas_size: vec2<u32>,
    tile_uv_size: vec2<f32>,
}

struct CameraUniform {
    view_proj: mat3x3<f32>, 
    map_size: vec2<u32>,
    _padding: vec2<u32>,
    atlases: array<AtlasInfo, 32>,
}
"#,
    );

    // テクスチャバインディングの動的生成
    for i in 0..texture_count {
        shader_source.push_str(&format!(
            "@group(0) @binding({}) var t_texture{}: texture_2d<f32>;\n",
            i, i
        ));
    }

    let sampler_binding = texture_count;
    shader_source.push_str(&format!(
        "@group(0) @binding({}) var s_sampler: sampler;\n",
        sampler_binding
    ));

    // 残りの共通部分
    shader_source.push_str(
        r#"
@group(1) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(1) var<storage, read> map_data: array<u32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) screen_uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var pos = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, -1.0)
    );
    
    var uvs = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0)
    );

    var out: VertexOutput;
    out.clip_position = vec4<f32>(pos[in_vertex_index], 0.0, 1.0);
    out.screen_uv = uvs[in_vertex_index];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos_h = camera.view_proj * vec3<f32>(in.screen_uv, 1.0);
    let world_pos = world_pos_h.xy; 

    let tile_pos = vec2<u32>(floor(world_pos));

    if (world_pos.x < 0.0 || world_pos.y < 0.0 || 
        tile_pos.x >= camera.map_size.x || tile_pos.y >= camera.map_size.y) {
        discard;
    }

    let map_index = tile_pos.y * camera.map_size.x + tile_pos.x;
    let map_data_val = map_data[map_index];
    
    // 上位16ビットをテクスチャインデックス、下位16ビットをタイルIDとして解釈
    let tex_index = map_data_val >> 16u;
    let tile_id   = map_data_val & 0xFFFFu;

    // 安全のため範囲内に収める
    let safe_tex_index = min(tex_index, 31u);
    let atlas = camera.atlases[safe_tex_index];

    let local_uv = fract(world_pos);

    let atlas_x = f32(tile_id % atlas.atlas_size.x);
    let atlas_y = f32(tile_id / atlas.atlas_size.x);
    
    let base_uv = vec2<f32>(atlas_x, atlas_y) * atlas.tile_uv_size;
    let final_uv = base_uv + local_uv * atlas.tile_uv_size;

    switch tex_index {
"#,
    );

    // 指定のタイルがない場合は描画しないようにする
    for i in 0..texture_count {
        shader_source.push_str(&format!(
            "        case {}u: {{\n            return textureSample(t_texture{}, s_sampler, final_uv);\n        }}\n",
            i, i
        ));
    }

    shader_source.push_str(
        r#"
        default: {
            //return textureSample(t_texture0, s_sampler, final_uv);
            discard;
        }
    }
}
"#,
    );

    shader_source
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

        // 別のテクスチャを1番に登録する場合はここにサイズを設定する
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
        let max_sampled_textures = Texture::request_max_sampled_textures(&limits, 32);

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
            present_mode: surface_caps.present_modes[0],
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

        let map_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Map Storage Buffer"),
            contents: bytemuck::cast_slice(&map_data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let initial_camera = CameraUniform::new(
            Vector2::new(size.width as f32, size.height as f32),
            Vector2::new(50.0 * 32.0, 50.0 * 32.0), // マップ中央付近
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

        // WGSLシェーダーコードの動的構築
        let shader_source = create_dynamic_tilemap_shader(texture_count);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Tilemap Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
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
                buffers: &[],
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
            texture_bind_group,
            uniform_bind_group,
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
            //Vector2::new(self.config.width as f32, self.config.height as f32),    // 画面サイズに連動しない
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
            render_pass.draw(0..4, 0..1); // 4頂点で1インスタンス（1画面分）を描画
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
