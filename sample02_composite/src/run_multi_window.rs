/// マルチウィンドウシステム
/// todo
/// - 文字の表示をまとめた処理を作る
/// - 文字枠をまとめた処理を作る
/// - 上２つを統合してウィンドウにする
/// - 深度バッファをつける
/// - キーで５階層くらいのウィンドウを開く、閉じるをできるようにする
/// - 流れるメッセージウィンドウを作る
use std::{iter, sync::Arc, vec};
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::{
    load_map_static,
    sub::{
        frame_renderer::FrameRenderer,
        key::InputState,
        map_data::MapData,
        text_renderer::{TextPaint, TextRenderer},
        texture::Texture,
        tile::{TileDrawGroup, TileInstance},
        utils::{DeviceParams, get_monitor_max_resolution, max_texture_dimension_2d},
    },
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct AtlasInfo {
    atlas_size: [u32; 2],
    tile_uv_size: [f32; 2],
    tile_pixel_size: [f32; 2],
    _padding: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GlobalUniforms {
    view_proj: [[f32; 4]; 4],
    pettern: [u32; 2],
    _padding: [u32; 2],
    atlases: [AtlasInfo; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct FragmentUniforms {
    opacity: f32,
}

const WORLD_INITIAL_POS: [f32; 2] = [7712.0, 8064.0];

pub struct State {
    device_params: DeviceParams,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    depth_texture: Texture,
    #[allow(dead_code)]
    textures: Vec<Texture>,
    #[allow(dead_code)]
    sampler: wgpu::Sampler,
    texture_bind_group: wgpu::BindGroup,
    sampler_bind_group: wgpu::BindGroup,
    uniform_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    vertex_uniform_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    fragment_uniform_buffer: wgpu::Buffer,
    camera_pos: [f32; 2],
    scale: f32,
    degress: f32,
    map_data_list: Vec<MapData>,
    map_count: u32,
    tile_draw_groups: Vec<TileDrawGroup>,
    frame_renderer: FrameRenderer,
    text_renderer: TextRenderer,
    input: InputState,
    window: Arc<Window>,
}

impl State {
    async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let size = window.inner_size();
        let target_logical_size = (size.width as f32, size.height as f32);
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

        let monitor_size = get_monitor_max_resolution(&window).unwrap_or((1920, 1080));
        println!("monitor_size: {:?}", monitor_size);

        let adapter_limits = adapter.limits();
        let max_texture_dimension_2d = max_texture_dimension_2d(
            &window,
            wgpu::Limits::default().max_texture_dimension_2d,
            adapter_limits.max_texture_dimension_2d,
        );
        println!("max_texture_dimension_2d: {}", max_texture_dimension_2d);

        // テクスチャ8枚
        // スプライトのほうが多いからスプライトのレイアウトを使いまわせばいいか
        let max_textures = Texture::request_max_textures(&adapter_limits, 8);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("tilemap_shader"),
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits {
                    max_sampled_textures_per_shader_stage: max_textures,
                    max_texture_dimension_2d: max_texture_dimension_2d,
                    ..wgpu::Limits::default()
                },
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .unwrap();

        println!(
            "device min_uniform_buffer_offset_aligment: {}",
            device.limits().min_uniform_buffer_offset_alignment
        );

        let world = load_map_static!("../../json/map_world.json")?;
        let map_data_1 = MapData::from_tiled_map(&world).unwrap();
        let map_data_list = vec![map_data_1];

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
        let empty_texture = Texture::create_empty_texture(&device, &queue, config.format);
        let world_bytes: &[u8] = include_bytes!("../../image/tilesets/world.png");
        let array_bytes = vec![world_bytes]; // テクスチャを増やす場合は要素を追加

        let mut textures = Vec::with_capacity(array_bytes.len());
        for bytes in array_bytes.iter() {
            let texture = Texture::from_bytes(&device, &queue, bytes, config.format, "Texture 2D")?;
            textures.push(texture);
        }

        let max_textures: u32 = 3;
        let mut layout_entries = Vec::with_capacity(max_textures as usize);
        for i in 0..max_textures {
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

        let mut group_entries = Vec::with_capacity(max_textures as usize);
        for (i, texture) in textures.iter().enumerate() {
            group_entries.push(wgpu::BindGroupEntry {
                binding: i as u32,
                resource: wgpu::BindingResource::TextureView(&texture.view),
            });
        }
        // 残りの２個は空テクスチャで埋める
        while group_entries.len() < max_textures as usize {
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

        let mut sampler_layout_entries = Vec::with_capacity(1);
        sampler_layout_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
            count: None,
        });

        let sampler_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &sampler_layout_entries,
                label: Some("sampler_bind_group_layout"),
            });

        let mut sampler_group_entries = Vec::with_capacity(1);
        sampler_group_entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Sampler(&sampler),
        });

        let sampler_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &sampler_bind_group_layout,
            entries: &sampler_group_entries,
            label: Some("sampler_bind_group"),
        });

        // ユニフォームデータ
        // シェーダーの共通データ
        // 1番目はmap1、2番目以降はmap2
        let half_tile1_w = map_data_list[0].tile_width / 2;
        let half_tile1_h = map_data_list[0].tile_height / 2;
        let uniform: GlobalUniforms = GlobalUniforms {
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(), // 対角成分がすべて 1.0
            pettern: [0, 0],
            _padding: [0, 0],
            atlases: [
                AtlasInfo {
                    atlas_size: [
                        textures[0].texture.width() / half_tile1_w,
                        textures[0].texture.height() / half_tile1_h,
                    ],
                    tile_uv_size: [
                        16.0 / textures[0].texture.width() as f32,
                        16.0 / textures[0].texture.height() as f32,
                    ],
                    tile_pixel_size: [half_tile1_w as f32, half_tile1_h as f32],
                    _padding: [0.0, 0.0],
                }, // 使わないので適当に埋めておく
                AtlasInfo {
                    atlas_size: [0, 0],
                    tile_uv_size: [0.0, 0.0],
                    tile_pixel_size: [0.0, 0.0],
                    _padding: [0.0, 0.0],
                },
                AtlasInfo {
                    atlas_size: [0, 0],
                    tile_uv_size: [0.0, 0.0],
                    tile_pixel_size: [0.0, 0.0],
                    _padding: [0.0, 0.0],
                },
            ],
        };

        // デバイスのアライメント制限を取得
        let alignment = device.limits().min_uniform_buffer_offset_alignment as usize;
        let vertex_uniform_size = std::mem::size_of::<GlobalUniforms>();
        // アライメントの倍数にパディングを計算
        // ２の累乗の最上位以外を落とす
        let vertex_aligned_size = (vertex_uniform_size + alignment - 1) & !(alignment - 1);

        let fragment_uniform_size = std::mem::size_of::<FragmentUniforms>();
        let fragment_aligned_size = (fragment_uniform_size + alignment - 1) & !(alignment - 1);

        let instance_size_1 = (std::mem::size_of::<TileInstance>() * 368640) as u64;
        let mut tile_draw_groups = Vec::with_capacity(1);
        tile_draw_groups.push(TileDrawGroup {
            vertex_bind_offset: 0,
            fragment_bind_offset: 0,
            opacity: map_data_list[0].opacity,
            slice_range: 0..instance_size_1, // 最大で使うタイル数を決める必要がある
            draw_range: 0..0,
            tiles: vec![],
        });

        let uniform_count = tile_draw_groups.len();
        // uniformバッファ
        let vertex_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dynamic Uniform Buffer"),
            size: (vertex_aligned_size * uniform_count) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        {
            let mut buffer_view = vertex_uniform_buffer.get_mapped_range_mut(..)?;

            buffer_view
                .slice(0..vertex_uniform_size)
                .copy_from_slice(bytemuck::bytes_of(&uniform));
        }
        vertex_uniform_buffer.unmap();

        let fragment_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dynamic Uniform Buffer"),
            size: (fragment_aligned_size * uniform_count) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        {
            let mut buffer_view = fragment_uniform_buffer.get_mapped_range_mut(..)?;

            for (i, draw_group) in tile_draw_groups.iter().enumerate() {
                let fragment_uniforms = FragmentUniforms {
                    opacity: draw_group.opacity,
                };
                buffer_view
                    .slice(
                        fragment_aligned_size * i
                            ..fragment_aligned_size * i + fragment_uniform_size,
                    )
                    .copy_from_slice(bytemuck::bytes_of(&fragment_uniforms));
            }
        }
        fragment_uniform_buffer.unmap();

        // ユニフォームグループレイアウト
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX, // 頂点シェーダーで参照するため
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: Some(
                                wgpu::BufferSize::new(vertex_uniform_size as u64).unwrap(),
                            ),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: Some(
                                wgpu::BufferSize::new(fragment_uniform_size as u64).unwrap(),
                            ),
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
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &vertex_uniform_buffer, // バッファ全体
                        offset: 0,
                        size: wgpu::BufferSize::new(vertex_uniform_size as u64), // 1回で使えるサイズ
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &fragment_uniform_buffer, // バッファ全体
                        offset: 0,
                        size: wgpu::BufferSize::new(fragment_uniform_size as u64),
                    }),
                },
            ],
            label: Some("uniform_bind_group"),
        });

        // WGSLシェーダーコード
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Tilemap Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader_tilemap.wgsl").into()),
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

        // 全体のタイル頂点
        // これを層ごとに分配する
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: instance_size_1 as wgpu::BufferAddress, // 実際は縮小に制限を持たせて超えないようにする
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false, // ← ここを true にすると作成と同時に書き込みを行うことが可能、ゲームでは不要であろう
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false), // 書き込みは行わない
                depth_compare: None,              // 参照もしない
                stencil: wgpu::StencilState::default(), // ステンシルは使用しない
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let device_params =
            DeviceParams::new(monitor_size, max_texture_dimension_2d, target_logical_size);

        let mut text_renderer = TextRenderer::new(
            device.clone(),
            queue.clone(),
            device_params.target_logical_size,
            // 画面サイズをカバーできるサイズ
            device_params
                .monitor_size()
                .0
                .max(device_params.monitor_size().1),
            config.format,
        );

        text_renderer.add_font(include_bytes!("../../fonts/yosugaraver1_2.ttf"));
        text_renderer.add_font(include_bytes!("../../fonts/GenEiPOPlePw-Bk.ttf"));

        //let frame_bytes: &[u8] = include_bytes!("../../image/window/Window_vl.png");
        // まず実績のあるほうで試してみる
        let frame_bytes: &[u8] = include_bytes!("../../image/window/WindowBase_blue.png");
        let mut frame_renderer = FrameRenderer::new(
            device.clone(),
            queue.clone(),
            device_params.target_logical_size,
            // 画面サイズをカバーできるサイズ
            frame_bytes,
            config.format,
            10,
        )?;
        frame_renderer.push_window(&(150.0, 100.0), &(500.0, 200.0), 0.5);

        let depth_texture = Texture::create_depth_texture(&device, size.width, size.height);

        // サイズガードを追加した場合は有効にしないと最初のサイズ設定がスキップされる
        //text_renderer.resize(size.width, size.height, device_params.target_logical_size);

        Ok(Self {
            device_params,
            surface,
            device,
            queue,
            config,
            render_pipeline,
            depth_texture,
            textures,
            sampler,
            texture_bind_group,
            sampler_bind_group,
            uniform_bind_group,
            instance_buffer,
            vertex_uniform_buffer,
            fragment_uniform_buffer,
            camera_pos: WORLD_INITIAL_POS,
            scale: 1.0,
            degress: 0.0,
            map_data_list,
            map_count: 0,
            tile_draw_groups,
            text_renderer,
            frame_renderer,
            input: InputState::default(),
            window,
        })
    }

    pub fn setup(&mut self) {
        self.update_tile_data();
    }

    /// 作成時にconfigと同じサイズで２回くるのでそのときはconfigureだけを行うようにしたい
    /// resized発生時にsurfaceが破棄されているのでconfigureは必須
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            // 上限を超えていれば超えないように補正する
            let (width, height) = if width > self.device_params.max_texture_dimension_2d()
                || height > self.device_params.max_texture_dimension_2d()
            {
                // 縦横比を維持して補正
                let w = width as f32;
                let h = height as f32;
                // 縦と横の大きい方を計算
                let max_size = w.max(h);
                // 縦横比を維持したサイズにする
                let max_texture_dimension_2d = self.device_params.max_texture_dimension_2d() as f32;
                (
                    (w / max_size * max_texture_dimension_2d) as u32,
                    (h / max_size * max_texture_dimension_2d) as u32,
                )
            } else {
                (width, height)
            };
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);

            self.depth_texture = Texture::create_depth_texture(&self.device, width, height);
            self.text_renderer
                .resize(width, height, self.device_params.target_logical_size);
            self.text_renderer.clear();
            self.draw_text();
            self.text_renderer.submit(true);
        }
    }

    fn draw_text(&mut self) {
        let paint = TextPaint {
            font_index: 1,
            font_size: 36,
            font_weight: 500,
            line_width: 4,
            color: [255, 255, 255, 255],
            line_color: [0, 0, 0, 128],
        };
        self.text_renderer
            .draw_text(0.0, 0.0, 0.0, "hello world", &paint);

        self.text_renderer
            .draw_text(0.0, 40.0, 0.0, "はなす", &paint);

        self.text_renderer
            .draw_text(0.0, 80.0, 0.0, "じゅもん", &paint);
    }

    pub fn save_offscreen_png(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let width: u32 = self.text_renderer.texture_dimension();
        let height: u32 = self.text_renderer.texture_dimension();
        let row_bytes = width * 4; // RGBA8
        let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = row_bytes.div_ceil(alignment) * alignment;

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("offscreen readback buffer"),
            size: u64::from(padded_bytes_per_row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("offscreen readback encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.text_renderer.textures()[0].texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        let (tx, rx) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                tx.send(result).expect("map callback receiver dropped");
            });
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv()??;

        let mapped = buffer.slice(..).get_mapped_range()?;
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in mapped
            .chunks_exact(padded_bytes_per_row as usize)
            .take(height as usize)
        {
            pixels.extend_from_slice(&row[..row_bytes as usize]);
        }
        drop(mapped);
        buffer.unmap();

        image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)?;

        Ok(())
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: KeyCode, pressed: bool) {
        match (key, pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            (KeyCode::F2, true) => {
                self.save_offscreen_png(&std::path::Path::new("femtovg_offscreen.png"))
                    .unwrap();
            }
            (KeyCode::F11, true) => {
                let fs = match self.window.fullscreen() {
                    Some(_) => None,
                    None => Some(winit::window::Fullscreen::Borderless(None)),
                };
                self.window.set_fullscreen(fs);
            }
            _ => {
                self.input.process_key(key, pressed);
            }
        }
    }

    fn update(&mut self) {
        if self.update_input() {
            self.update_tile_data();
        }

        if self.tile_draw_groups.len() == 0 {
            return;
        }

        let target_logical_size = &self.device_params.target_logical_size;
        // 正射影行列を作成する、WGPUのNDCマッピング
        // 1draw中に変化することはないのでcpu側で行う
        let proj = glam::camera::rh::proj::directx::orthographic(
            0.0,
            target_logical_size.0,
            target_logical_size.1,
            0.0,
            -1.0,
            1.0,
        );

        let half_logical_w = target_logical_size.0 / 2.0;
        let half_logical_h = target_logical_size.1 / 2.0;

        let rotation = self.degress.to_radians();
        // 右側から適用
        // ・画面中央への配置
        // ・スケーリング
        // ・カメラ位置の逆オフセット
        let view =
            glam::Mat4::from_translation(glam::Vec3::new(half_logical_w, half_logical_h, 0.0))
                * glam::Mat4::from_rotation_z(rotation)
                * glam::Mat4::from_scale(glam::Vec3::new(self.scale, self.scale, 1.0))
                * glam::Mat4::from_translation(glam::Vec3::new(
                    -self.camera_pos[0],
                    -self.camera_pos[1],
                    0.0,
                ));
        let view_proj = proj * view;

        self.queue.write_buffer(
            &self.vertex_uniform_buffer,
            self.tile_draw_groups[0].vertex_bind_offset as wgpu::BufferAddress,
            bytemuck::bytes_of(&view_proj.to_cols_array_2d()),
        );

        let pattern: [u32; 2] = [self.map_count / 20, 0];
        self.map_count = (self.map_count + 1) % 60;
        if pattern[0] != self.map_count {
            for draw_group in self.tile_draw_groups.iter() {
                self.queue.write_buffer(
                    &self.vertex_uniform_buffer,
                    (std::mem::size_of::<[[f32; 4]; 4]>() + draw_group.vertex_bind_offset as usize)
                        as wgpu::BufferAddress,
                    bytemuck::bytes_of(&pattern),
                );
            }
        }
    }

    fn update_input(&mut self) -> bool {
        if self.input.reset {
            // マップの左上の中心に表示が[0.0, 0.0]となる
            self.camera_pos = WORLD_INITIAL_POS;
            self.scale = 1.0;
            self.degress = 0.0;
            return true;
        }

        let speed = 8.0;
        let [dx, dy] = self.input.direction();

        let mut dirty = false;

        if dx != 0.0 || dy != 0.0 {
            self.camera_pos[0] += dx * speed;
            self.camera_pos[1] += dy * speed;
            dirty = true;
        }

        // 値がずれないように２の負のべき乗単位で拡大縮小する
        if self.input.plus {
            self.scale += 0.0625;
            dirty = true;
        } else if self.input.minus {
            if self.scale > 0.125 {
                self.scale -= 0.0625;
                self.scale = self.scale.max(0.125);
                dirty = true;
            }
        }
        let prev_degress = self.degress;
        if self.input.rotation_l {
            self.degress = (self.degress + 360.0 - 4.0) % 360.0;
        } else if self.input.rotation_r {
            self.degress = (self.degress + 4.0) % 360.0;
        }
        if prev_degress != self.degress && (prev_degress == 0.0 || self.degress == 0.0) {
            dirty = true;
        }

        return dirty;
    }

    fn update_tile_data(&mut self) {
        for i in 0..self.tile_draw_groups.len() {
            self.set_tile_data(i);
        }
    }

    fn set_tile_data(&mut self, index: usize) {
        if index != 0 {
            return;
        }
        let map_data = &self.map_data_list[index];
        let draw_group = &mut self.tile_draw_groups[index];
        let data = &map_data.data;
        // layerのサイズは実際のタイルの数
        let map_width = map_data.width as i32;
        let map_height = map_data.height as i32;
        let tiles = &mut draw_group.tiles;
        // 1テクスチャしかないので他は1で埋める
        let counts: [u32; 3] = [self.textures[0].texture.width() / 32, 1, 1];
        tiles.clear();

        let target_logical_size = &self.device_params.target_logical_size;
        let half_logical_w = target_logical_size.0 / 2.0;
        let half_logical_h = target_logical_size.1 / 2.0;
        // 可視範囲の計算
        // index = 1 は拡大縮小回転をしない
        let [half_w, half_h] = if index == 1 {
            [half_logical_w, half_logical_h]
        } else {
            if self.degress != 0.0 {
                // 簡易的に最大領域を確保しているだけだが同じ位置なら更新がないのでメリットもある
                let half = ((half_logical_w * half_logical_w + half_logical_h * half_logical_h)
                    as f32)
                    .sqrt()
                    / self.scale;
                [half, half]
            } else {
                [half_logical_w / self.scale, half_logical_h / self.scale]
            }
        };

        let (camera_x, camera_y) = (self.camera_pos[0], self.camera_pos[1]);

        let view_left = camera_x - half_w;
        let view_right = camera_x + half_w;
        let view_top = camera_y - half_h;
        let view_bottom = camera_y + half_h;

        let tile_width_f = map_data.tile_width as f32;
        let tile_height_f = map_data.tile_height as f32;
        // 32.0px単位のタイルインデックス範囲
        // todo この計算だと余分に1チップ多くなることがあるのでうまくできないか見直す
        // 表示の最大範囲を求めておいて超えていたら補正するとなど
        let start_x = (view_left / tile_width_f).floor() as i32;
        let end_x = (view_right / tile_width_f).ceil() as i32;
        let start_y = (view_top / tile_height_f).floor() as i32;
        let end_y = (view_bottom / tile_height_f).ceil() as i32;

        for layer in &map_data.layers {
            for y in start_y..=end_y {
                for x in start_x..=end_x {
                    if x < 0 || x >= map_width || y < 0 || y >= map_height {
                        continue;
                    }
                    let tile = data[((y * map_width + x) as u32 + layer.start_index) as usize];
                    if tile & 0xFFFF == 0xFFFF {
                        continue;
                    }
                    TileInstance::push_tile(tiles, tile, x, y, counts);
                }
            }
        }

        let tile_count = tiles.len() as u32;
        draw_group.draw_range.end = tile_count;
        self.queue.write_buffer(
            &self.instance_buffer,
            draw_group.slice_range.start,
            bytemuck::cast_slice(&tiles),
        );
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
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(1, &self.texture_bind_group, &[]);
            render_pass.set_bind_group(2, &self.sampler_bind_group, &[]);

            // 描画回数分回す
            // 実際は間にスプライトが入るから関数にまとめて1こづつ処理することになる
            // たぶんテクスチャも切り替わる
            for tile_draw_group in self.tile_draw_groups.iter() {
                render_pass.set_bind_group(
                    0,
                    &self.uniform_bind_group,
                    &[
                        tile_draw_group.vertex_bind_offset,
                        tile_draw_group.fragment_bind_offset,
                    ],
                );
                render_pass.set_vertex_buffer(
                    0,
                    self.instance_buffer
                        .slice(tile_draw_group.slice_range.clone()),
                );
                render_pass.draw(0..4, tile_draw_group.draw_range.clone());
            }

            self.frame_renderer.render(&mut render_pass);
            self.text_renderer.render(&mut render_pass);
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

        self.state = Some(pollster::block_on(State::new(window)).unwrap());
        // State::newで設定しきれなかった設定を行う
        // 実際はゲームループ内で行う
        if let Some(state) = &mut self.state {
            state.setup()
        };
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
