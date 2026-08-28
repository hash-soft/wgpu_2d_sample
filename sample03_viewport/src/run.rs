use std::{iter, sync::Arc};

use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::{key::InputState, sprite::SpriteInstance, texture};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TexVertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}

impl TexVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TexVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0, // @location(0) にあたる
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress, // 前までのサイズ分進む
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
}

// 頂点データは 0.0 ~ 1.0 の矩形にする
const VERTICES_LOCAL: &[TexVertex] = &[
    TexVertex {
        position: [0.0, 0.0, 0.0],
        tex_coords: [0.0, 0.0],
    }, // 左上
    TexVertex {
        position: [0.0, 1.0, 0.0],
        tex_coords: [0.0, 1.0],
    }, // 左下
    TexVertex {
        position: [1.0, 1.0, 0.0],
        tex_coords: [1.0, 1.0],
    }, // 右下
    TexVertex {
        position: [1.0, 0.0, 0.0],
        tex_coords: [1.0, 0.0],
    }, // 右上
];

const INDICES: &[u16] = &[0, 1, 3, 1, 2, 3];

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

// 画面全体に配置 左上から下に進み1週する
const VERTICES4: &[Vertex] = &[
    Vertex {
        position: [-1.0, 1.0, 0.0],
        color: [1.0, 0.0, 0.0],
    }, // A
    Vertex {
        position: [-1.0, -1.0, 0.0],
        color: [0.0, 1.0, 0.0],
    }, // B
    Vertex {
        position: [1.0, -1.0, 0.0],
        color: [1.0, 0.0, 0.0],
    }, // C
    Vertex {
        position: [1.0, 1.0, 0.0],
        color: [0.0, 0.0, 1.0],
    }, // D
];

pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    index4_buffer: wgpu::Buffer,
    num_indices: u32,
    #[allow(dead_code)]
    diffuse_texture: texture::Texture, // 現在は使っていない
    diffuse_bind_group: wgpu::BindGroup,
    uniform_render_pipeline: wgpu::RenderPipeline,
    vertex_local_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    uniform_buffer: wgpu::Buffer, // 保持しているだけ
    uniform_bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    uniform: Uniforms,
    instance_buffer: wgpu::Buffer,
    sprites: Vec<SpriteInstance>,
    buffer_render_pipeline: wgpu::RenderPipeline,
    vertex4_buffer: wgpu::Buffer,
    input: InputState,
    window: Arc<Window>,
}

impl State {
    async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: Default::default(),
        });
        // すべてデフォルトだがbackendsもデフォルトになってしまう
        //let instance = wgpu::Instance::default();

        let surface = instance.create_surface(window.clone()).unwrap();

        // Adapterを探す
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(), // 電力志向
                compatible_surface: Some(&surface), // Surfaceと互換性のあるAdapterを探す
                force_fallback_adapter: false,      // trueなら強制ソフトウェア
                apply_limit_buckets: true,          // ハードウェア固有の制限値を丸める
            })
            .await
            .unwrap();

        // Adapterの情報を出力
        let adapter_info = adapter.get_info();
        println!("=== WGPU Debug Info ===");
        println!("Adapter Info:\n {:#?}", adapter_info);
        let limits = adapter.limits();
        println!("Max 2D texture size: {}", limits.max_texture_dimension_2d);

        // 探したAdapterからDeviceとQueueを作る
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("sample01"),                    // デバイス特定のためつけておく
                required_features: wgpu::Features::empty(), // 拡張機能不要なのでempty
                experimental_features: wgpu::ExperimentalFeatures::disabled(), // 実験的機能は不要
                required_limits: wgpu::Limits::default(),   // 限界値は標準の制限値
                memory_hints: Default::default(),           // メモリ割り当て方法は標準
                trace: wgpu::Trace::Off,                    // 通常はOff
            })
            .await
            .unwrap();

        // 利用可能モードを取得
        let surface_caps = surface.get_capabilities(&adapter);
        println!("Surface Capabilities:\n {:#?}", surface_caps);
        // ピクセルフォーマットの決定
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        println!("Surface Format:\n {:#?}", surface_format);
        // サーフェス情報
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, // 描画先（レンダリングターゲット）として使用
            format: surface_format,                        // surface_capsから決定したフォーマット
            width: size.width,                             // ウィンドウ内部の幅
            height: size.height,                           // ウィンドウ内部の高さ
            present_mode: surface_caps.present_modes[0], // 垂直同期などの表示モード（利用可能な最初のモード）
            alpha_mode: surface_caps.alpha_modes[0], // ウィンドウ背後との合成モード（利用可能な最初のモード）
            view_formats: vec![],                    // ビューフォーマットの追加設定（空）
            desired_maximum_frame_latency: 2,        // 最大フレームレイテンシ
            color_space: wgpu::SurfaceColorSpace::Auto, // カラー空間の自動設定
        };
        // SurfaceConfigの出力
        println!("Surface Config:\n {:#?}", config);
        println!("=======================");

        let diffuse_bytes = include_bytes!("pipo-enemy021.png");
        let diffuse_texture =
            texture::Texture::from_bytes(&device, &queue, diffuse_bytes, "enemy021").unwrap();

        // テクスチャグループレイアウト
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT, // フラグメントシェーダーで使う
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        // テクスチャバインドグループ
        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

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

        // テクスチャパイプラインレイアウトの作成
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&texture_bind_group_layout),
                    Some(&uniform_bind_group_layout),
                ], // グループレイアウトをバインド
                immediate_size: 0,
            });

        let uniform_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Uniform Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader_uniform.wgsl").into()),
        });

        // Uniformを使用したパイプライン
        let uniform_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &uniform_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(TexVertex::desc()), Some(SpriteInstance::desc())],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &uniform_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            });

        let index4_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        // ユニフォームデータ
        let min_size = size.width.min(size.height);
        let uniform: Uniforms = Uniforms {
            screen_size: [min_size as f32, min_size as f32],
        };

        // uniformバッファ
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // uniformバインドグループ
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });

        let vertex_local_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Local Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES_LOCAL),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let sprites = vec![
            // スプライト 1（全体表示）
            SpriteInstance {
                position: [400.0, 300.0],
                size: [
                    diffuse_texture.texture.width() as f32,
                    diffuse_texture.texture.height() as f32,
                ],
                uv_offset: [0.0, 0.0],
                uv_size: [1.0, 1.0],
            },
            // スプライト 2（切り抜き表示）
            SpriteInstance {
                position: [100.0, 100.0],
                size: [
                    (diffuse_texture.texture.width() / 2) as f32,
                    (diffuse_texture.texture.height() / 2) as f32,
                ],
                uv_offset: [0.2, 0.2],
                uv_size: [0.6, 0.6],
            },
        ];

        // instanceバッファ
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&sprites),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let num_indices = INDICES.len() as u32;

        // 矩形バッファ
        let shader_buffer = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Dyn Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader_buffer.wgsl").into()),
        });

        let render_buffer_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });
        // Vertexを使用したパイプライン
        let buffer_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_buffer_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader_buffer,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(Vertex::desc())],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader_buffer,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent::REPLACE,
                            alpha: wgpu::BlendComponent::REPLACE,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            });

        // 4角形頂点バッファ
        let vertex4_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES4),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            index4_buffer,
            num_indices,
            diffuse_texture,
            diffuse_bind_group,
            uniform_render_pipeline,
            vertex_local_buffer,
            uniform_buffer,
            uniform_bind_group,
            uniform,
            instance_buffer,
            sprites,
            buffer_render_pipeline,
            vertex4_buffer,
            input: InputState::default(),
            window,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        // サーフェスは固定で画面サイズに合わせて伸縮したい場合はサイズ変更をやめるだけでOK
        // 縦横比率はviewportで制御する
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);

            // 以下のコメントアウト部分を有効にすると位置とサイズが追従しない
            // let uniform = Uniforms {
            //     screen_size: [width as f32, height as f32], // ここを新しい幅・高さに更新
            //     position: [400.0, 300.0],                   // 任意の固定位置 (x, y)
            //     image_size: [
            //         self.diffuse_texture.texture.width() as f32,
            //         self.diffuse_texture.texture.height() as f32,
            //     ],
            // };

            // // 2. queue.write_buffer で GPU 上のバッファデータを書き換える
            // self.queue
            //     .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));
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
        let speed = 2.0;
        let [dx, dy] = self.input.direction();

        if dx != 0.0 || dy != 0.0 {
            self.sprites[0].position[0] += dx * speed;
            self.sprites[0].position[1] += dy * speed;

            self.queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&self.sprites),
            );
        }
    }

    fn render(&mut self) -> anyhow::Result<()> {
        // 次フレームのRedrawRequested要求
        // この処理が一定間隔のループを実現している
        self.window.request_redraw();

        // 画面に出力するsurfaceを取得
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                // SurfaceConfigの再構築
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                anyhow::bail!("Lost device");
            }
        };
        // アクセスするためのビューを作成
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // 描画コマンドを記録するためのコマンドエンコーダを作成
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            // レンダーパス開始
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // パスの開始時と終了時の操作
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None, // テクスチャにレンダリングを行うための設定
                })],
                depth_stencil_attachment: None, // 深度バッファやステンシルバッファの設定
                occlusion_query_set: None,      // 見えないオブジェクトをスキップするためのクエリ
                timestamp_writes: None,         // 処理計測のタイムスタンプの書き込み先
                multiview_mask: None,           // マルチビュー機能のためのマスク
            });

            // これで描画範囲を指定する
            // 縦と横小さいほうに合わせて中央に配置
            let size = self.window.inner_size();
            let w = size.width as f32;
            let h = size.height as f32;
            // 縦と横の小さい方（正方形のビューポートサイズ）を計算
            let min_size = w.min(h);

            // 中央に配置するためのオフセット（x, y）を計算
            let x = (w - min_size) / 2.0;
            let y = (h - min_size) / 2.0;

            render_pass.set_viewport(x, y, min_size, min_size, 0.0, 1.0);

            // パイプライン設定
            render_pass.set_pipeline(&self.buffer_render_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex4_buffer.slice(..));
            render_pass.set_index_buffer(self.index4_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);

            render_pass.set_pipeline(&self.uniform_render_pipeline);
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            render_pass.set_bind_group(1, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_local_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.set_index_buffer(self.index4_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..self.sprites.len() as u32);
        }

        // コマンドを実行
        self.queue.submit(iter::once(encoder.finish()));
        // サーフェスに反映
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
            WindowEvent::MouseInput { state, button, .. } => match (button, state.is_pressed()) {
                (MouseButton::Left, true) => {}
                (MouseButton::Left, false) => {}
                _ => {}
            },
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
