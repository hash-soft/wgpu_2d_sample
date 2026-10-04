use std::{iter, sync::Arc};

use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::sub::texture::Texture;

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

// 中央に配置
const VERTICES: &[Vertex] = &[
    Vertex {
        position: [0.0, 1.0, 0.0],
        color: [1.0, 0.0, 0.0],
    },
    Vertex {
        position: [-1.0, -1.0, 0.0],
        color: [0.0, 1.0, 0.0],
    },
    Vertex {
        position: [1.0, -1.0, 0.0],
        color: [0.0, 0.0, 1.0],
    },
];

pub struct State {
    #[allow(dead_code)]
    target_logical_size: (f32, f32),
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    buffer_render_pipeline: wgpu::RenderPipeline,
    blit_render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    render_target_view: wgpu::TextureView,
    #[allow(dead_code)]
    sampler: wgpu::Sampler,
    blit_bind_group: wgpu::BindGroup,
    window: Arc<Window>,
}

impl State {
    async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let size = window.inner_size();
        // 初期サイズを論理サイズとして設定する
        let target_logical_size = (size.width as f32, size.height as f32);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::default(),
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
            present_mode: wgpu::PresentMode::default(),    // 垂直同期
            alpha_mode: surface_caps.alpha_modes[0], // ウィンドウ背後との合成モード（利用可能な最初のモード）
            view_formats: vec![],                    // ビューフォーマットの追加設定（空）
            desired_maximum_frame_latency: 2,        // 最大フレームレイテンシ
            color_space: wgpu::SurfaceColorSpace::Auto, // カラー空間の自動設定
        };
        // SurfaceConfigの出力
        println!("Surface Config:\n {:#?}", config);
        println!("=======================");

        // パイプラインの作成
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });

        let shader_buffer = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Dyn Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shader_polygon_index_buffer.wgsl").into(),
            ),
        });

        // Vertexを使用したパイプライン
        let buffer_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
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

        // 頂点バッファ
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let render_target_size = wgpu::Extent3d {
            width: target_logical_size.0 as u32,
            height: target_logical_size.1 as u32,
            depth_or_array_layers: 1,
        };

        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Fixed Resolution Render Target"),
            size: render_target_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format, // シーン用のフォーマット
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let render_target_view = render_target.create_view(&wgpu::TextureViewDescriptor::default());

        // オフスクリーンから転送するパイプライン
        let blit_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Blit Bind Group Layout"),
                entries: &[
                    // Binding 0: テクスチャ
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Binding 1: サンプラー
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blit Pipeline Layout"),
            bind_group_layouts: &[Some(&blit_bind_group_layout)],
            immediate_size: 0,
        });

        let shader_blit = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Dyn Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader_blit.wgsl").into()),
        });

        let blit_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blit Pipeline"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_blit,
                entry_point: Some("vs_main"),
                buffers: &[], // 頂点バッファは空でOK
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_blit,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format, // Surface（画面）のフォーマットを指定
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = Texture::create_sampler(&device);
        let blit_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &blit_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&render_target_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("blit_bind_group"),
        });

        Ok(Self {
            target_logical_size,
            surface,
            device,
            queue,
            config,
            buffer_render_pipeline,
            blit_render_pipeline,
            vertex_buffer,
            render_target_view,
            sampler,
            blit_bind_group,
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
        }
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: KeyCode, pressed: bool) {
        match (key, pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            _ => {}
        }
    }

    fn update(&mut self) {}

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
            // オフスクリーンレンダーパス開始
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Offscreen Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.render_target_view, // 固定解像度のViewを指定
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
                ..Default::default()
            });

            render_pass.set_pipeline(&self.buffer_render_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..3, 0..1);
        }

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

            render_pass.set_pipeline(&self.blit_render_pipeline);
            render_pass.set_bind_group(0, &self.blit_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
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
            .with_inner_size(winit::dpi::PhysicalSize::new(960, 540))
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
