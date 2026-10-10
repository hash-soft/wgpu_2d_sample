/// とりあえずSpriteInstanceを使う
use std::{iter, sync::Arc};

use glyphon::{
    Attrs, Cache, Family, FontSystem, Resolution, SwashCache, TextArea, TextAtlas, TextBounds,
    TextRenderer, Viewport,
};
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::sub::{font::CustomFont, frame, sprite::SpriteInstance};
use crate::{sub::key::InputState, sub::texture::Texture};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct VertexUniforms {
    screen_size: [f32; 2],
}

pub struct State {
    target_logical_size: (f32, f32),
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    textures: Vec<Texture>,
    texture_bind_group: wgpu::BindGroup,
    sampler_bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    sprites: Vec<SpriteInstance>,
    font_system: FontSystem,
    text_renderer: TextRenderer,
    text_buffer: glyphon::Buffer,
    text_viewport: Viewport,
    text_atlas: TextAtlas,
    swash_cache: SwashCache,
    input: InputState,
    window: Arc<Window>,
}

impl State {
    async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let size = window.inner_size();
        let target_logical_size = (size.width as f32, size.height as f32);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::default(),
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: Default::default(),
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .unwrap();

        let limits = adapter.limits();
        let max_sampled_textures = Texture::request_max_textures(&limits, 32);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("sample_filter"),
                required_limits: wgpu::Limits {
                    max_sampled_textures_per_shader_stage: max_sampled_textures,
                    ..wgpu::Limits::default()
                },
                ..Default::default()
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

        let empty_texture = Texture::create_empty_texture(&device, &queue);
        let dragon_bytes: &[u8] = include_bytes!("image/window/rainbow.png");
        let array_bytes = vec![dragon_bytes];

        let mut textures = Vec::with_capacity(array_bytes.len());
        for bytes in array_bytes.iter() {
            let texture = Texture::from_bytes(&device, &queue, bytes, "Texture 2D")?;
            textures.push(texture);
        }

        let texture_count = array_bytes.len() as u32;
        let mut layout_entries = Vec::with_capacity(texture_count as usize);
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

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &layout_entries,
                label: Some("texture_bind_group_layout"),
            });

        let mut group_entries = Vec::with_capacity(texture_count as usize);
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

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &group_entries,
            label: Some("texture_bind_group"),
        });

        let sampler = Texture::create_sampler(&device);
        let sampler_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                }],
                label: Some("sampler_bind_group_layout"),
            });

        let sampler_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &sampler_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&sampler),
            }],
            label: Some("sampler_bind_group"),
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
                label: Some("uniform_bind_group_layout"),
            });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&uniform_bind_group_layout),
                    Some(&texture_bind_group_layout),
                    Some(&sampler_bind_group_layout),
                ],
                immediate_size: 0,
            });

        let uniform_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Filter Uniform Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader_texture_uniform.wgsl").into()),
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Filter Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &uniform_shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(SpriteInstance::desc())],
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
                topology: wgpu::PrimitiveTopology::TriangleStrip,
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

        let uniform: VertexUniforms = VertexUniforms {
            screen_size: [target_logical_size.0, target_logical_size.1],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Filter Uniform Buffer"),
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

        let mut sprites = vec![
            SpriteInstance {
                position: [0.0, 0.0],
                size: [0.0, 0.0],
                uv_offset: [0.0, 0.0],
                uv_size: [0.0, 0.0],
                texture_index: 0,
            };
            9
        ];
        // 中央に半分のサイズで出す
        frame::make_frame_sprites(
            &mut sprites[0..9],
            &(target_logical_size.0 / 4.0, target_logical_size.1 / 4.0),
            &(target_logical_size.0 / 2.0, target_logical_size.1 / 2.0),
            &(textures[0].texture.width(), textures[0].texture.height()),
            0,
        );

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Filter Instance Buffer"),
            contents: bytemuck::cast_slice(&sprites),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let mut selectable_fonts = Vec::new();
        let mut font_system = FontSystem::new();
        // メッセージに適したフォントを使う
        let font_data_gen = include_bytes!("fonts/GenEiPOPlePw-Bk.ttf");
        let font_gen = CustomFont::load_font(&mut font_system, font_data_gen.to_vec()).unwrap();
        selectable_fonts.push(font_gen);

        let swash_cache = SwashCache::new();
        let mut cache = Cache::new(&device);
        let mut text_atlas = TextAtlas::new(&device, &queue, &mut cache, config.format);
        let text_renderer = TextRenderer::new(
            &mut text_atlas,
            &device,
            wgpu::MultisampleState::default(),
            None,
        );

        let mut text_buffer =
            glyphon::Buffer::new(&mut font_system, glyphon::Metrics::new(30.0, 42.0));
        text_buffer.set_size(Some(size.width as f32), Some(size.height as f32));
        text_buffer.set_text(
            "こんにちは, glyphon!\nText inside a window.\nCreated with wgpu!\nThis is a sample program.",
            // Attrsを返せばいいので不要ならcolorを設定する必要はない
            // rgbaも使える
            &Attrs::new()
                .family(Family::Name(&selectable_fonts[0].family))
                .weight(selectable_fonts[0].get_closest_weight(glyphon::Weight::default()))
                .color(glyphon::Color::rgb(255, 255, 255)),
            glyphon::Shaping::Advanced,
            None,
        );
        // text_bufferを更新したら行う必要がある
        text_buffer.shape_until_scroll(&mut font_system, false);

        let mut text_viewport = Viewport::new(&device, &cache);
        text_viewport.update(
            &queue,
            Resolution {
                width: config.width,
                height: config.height,
            },
        );

        Ok(Self {
            target_logical_size,
            surface,
            device,
            queue,
            config,
            render_pipeline,
            textures,
            texture_bind_group,
            sampler_bind_group,
            uniform_buffer,
            uniform_bind_group,
            instance_buffer,
            sprites,
            font_system,
            swash_cache,
            text_atlas,
            text_renderer,
            text_buffer,
            text_viewport,
            input: InputState::default(),
            window,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.resize_text();
        }
    }

    pub fn text_renderer_prepare(&mut self) {
        // 実際は縦横の小さいほうに合わせてサイズをそろえるため
        // 縦横でスケールが異なることはない
        let scale_x = self.config.width as f32 / self.target_logical_size.0;
        let scale_y = self.config.height as f32 / self.target_logical_size.1;
        let uniform_scale = scale_x.min(scale_y);
        let left = self.target_logical_size.0 / 4.0 + (self.textures[0].texture.width() / 3) as f32;
        let top = self.target_logical_size.1 / 4.0 + (self.textures[0].texture.height() / 3) as f32;
        // 描画テキストの準備 (Prepare)
        self.text_renderer
            .prepare(
                &self.device,
                &self.queue,
                &mut self.font_system,
                &mut self.text_atlas,
                &self.text_viewport,
                [TextArea {
                    buffer: &self.text_buffer,
                    left: left * scale_x,
                    top: top * scale_y,
                    scale: uniform_scale,
                    // 現在の設定は画面全体なので問題ないが
                    // １部に設定したければscaleをかける必要がある
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: self.config.width as i32,
                        bottom: self.config.height as i32,
                    },
                    default_color: glyphon::Color::rgb(0, 255, 255),
                    custom_glyphs: &[],
                }],
                &mut self.swash_cache,
            )
            .unwrap();
    }

    fn resize_text(&mut self) {
        self.text_buffer.set_size(
            Some(self.config.width as f32),
            Some(self.config.height as f32),
        );
        self.text_buffer
            .shape_until_scroll(&mut self.font_system, false);

        self.text_viewport.update(
            &self.queue,
            Resolution {
                width: self.config.width,
                height: self.config.height,
            },
        );
        self.text_renderer_prepare();
    }

    fn update(&mut self) {
        // キーの上下でノイズの強さを変更する
        self.update_input();
        self.queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.sprites),
        );
    }

    fn update_input(&mut self) {}

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: KeyCode, pressed: bool) {
        match (key, pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            _ => {
                self.input.process_key(key, pressed);
            }
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
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &self.texture_bind_group, &[]);
            render_pass.set_bind_group(2, &self.sampler_bind_group, &[]);

            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.sprites.len() as u32);

            self.text_renderer
                .render(&self.text_atlas, &self.text_viewport, &mut render_pass)
                .unwrap();
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
            .with_inner_size(winit::dpi::PhysicalSize::new(960, 540))
            .with_visible(false);
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        window.set_visible(true);

        let mut state = pollster::block_on(State::new(window)).unwrap();
        state.text_renderer_prepare();
        self.state = Some(state);
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
            } => {
                state.handle_key(event_loop, code, key_state.is_pressed());
            }
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
