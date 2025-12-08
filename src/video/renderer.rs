use crate::video::decoder::{DecodedFrame, PixelFormat};
use anyhow::{Context, Result};
use wgpu::{
    Backends, Device, DeviceDescriptor, Features, Instance, Limits, PowerPreference, Queue,
    RequestAdapterOptions, Surface, SurfaceConfiguration, TextureFormat, TextureUsages,
};
use winit::window::Window;

/// GPU-accelerated video renderer using wgpu
pub struct VideoRenderer<'a> {
    #[allow(dead_code)]
    instance: Instance,
    surface: Surface<'a>,
    device: Device,
    queue: Queue,
    config: SurfaceConfiguration,
    window: &'a Window,
    render_pipeline: wgpu::RenderPipeline,
    texture: Option<wgpu::Texture>,
    texture_bind_group: Option<wgpu::BindGroup>,
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    current_width: u32,
    current_height: u32,
}

impl<'a> VideoRenderer<'a> {
    /// Create a new video renderer
    pub fn new(window: &'a Window) -> Result<Self> {
        // Create wgpu instance
        let instance = Instance::new(wgpu::InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        // Create surface
        let surface = instance
            .create_surface(window)
            .context("Failed to create surface")?;

        // Request adapter
        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .context("Failed to find suitable GPU adapter")?;

        tracing::info!("Using GPU: {}", adapter.get_info().name);

        // Request device and queue
        let (device, queue) = pollster::block_on(adapter.request_device(
            &DeviceDescriptor {
                label: Some("Main Device"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .context("Failed to create device")?;

        // Configure surface
        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo, // VSync for now
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        // Create sampler for texture upscaling (bilinear interpolation)
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear, // Bilinear upscaling
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create bind group layout for texture
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Texture Bind Group Layout"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create render pipeline
        let render_pipeline = Self::create_render_pipeline(&device, &config, &bind_group_layout)?;

        Ok(Self {
            instance,
            surface,
            device,
            queue,
            config,
            window,
            render_pipeline,
            texture: None,
            texture_bind_group: None,
            sampler,
            bind_group_layout,
            current_width: 0,
            current_height: 0,
        })
    }

    /// Create the render pipeline with shaders
    fn create_render_pipeline(
        device: &Device,
        config: &SurfaceConfiguration,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Result<wgpu::RenderPipeline> {
        // Shader source (WGSL)
        let shader_source = include_str!("shaders/video.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Video Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Ok(pipeline)
    }

    /// Get current known video size
    pub fn current_video_size(&self) -> Option<(u32, u32)> {
        if self.current_width > 0 && self.current_height > 0 {
            Some((self.current_width, self.current_height))
        } else {
            None
        }
    }

    /// Render a decoded frame to the window
    pub fn render(&mut self, frame: &DecodedFrame) -> Result<()> {
        // Skip if window is minimized (0 size) to avoid swapchain errors
        if self.config.width == 0 || self.config.height == 0 {
            return Ok(());
        }

        // Update texture if frame size changed
        if frame.width != self.current_width || frame.height != self.current_height {
            self.update_texture(frame.width, frame.height)?;
        }

        // Upload frame data to GPU texture
        self.upload_frame_data(frame)?;

        // Render to screen
        self.render_to_screen()?;

        Ok(())
    }

    /// Create or update the video texture
    fn update_texture(&mut self, width: u32, height: u32) -> Result<()> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Video Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        self.texture = Some(texture);
        self.texture_bind_group = Some(bind_group);
        self.current_width = width;
        self.current_height = height;

        Ok(())
    }

    /// Upload frame data to GPU texture
    fn upload_frame_data(&mut self, frame: &DecodedFrame) -> Result<()> {
        let texture = self.texture.as_ref().context("Texture not initialized")?;

        // Convert frame data to RGBA if needed
        let rgba_data = match frame.format {
            PixelFormat::RGBA => frame.data.clone(),
            PixelFormat::YUV420P => Self::yuv420p_to_rgba(&frame.data, frame.width, frame.height),
            PixelFormat::NV12 => Self::nv12_to_rgba(&frame.data, frame.width, frame.height),
        };

        // Upload to GPU
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * frame.width),
                rows_per_image: Some(frame.height),
            },
            wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    /// Convert YUV420P to RGBA
    fn yuv420p_to_rgba(yuv_data: &[u8], width: u32, height: u32) -> Vec<u8> {
        let w = width as usize;
        let h = height as usize;
        let y_size = w * h;
        let uv_size = (w / 2) * (h / 2);

        let mut rgba = vec![0u8; w * h * 4];

        for y in 0..h {
            for x in 0..w {
                let y_index = y * w + x;
                let uv_index = (y / 2) * (w / 2) + (x / 2);

                let y_val = yuv_data[y_index] as f32;
                let u_val = yuv_data[y_size + uv_index] as f32 - 128.0;
                let v_val = yuv_data[y_size + uv_size + uv_index] as f32 - 128.0;

                // YUV to RGB conversion
                let r = (y_val + 1.402 * v_val).clamp(0.0, 255.0) as u8;
                let g = (y_val - 0.344 * u_val - 0.714 * v_val).clamp(0.0, 255.0) as u8;
                let b = (y_val + 1.772 * u_val).clamp(0.0, 255.0) as u8;

                let rgba_index = y_index * 4;
                rgba[rgba_index] = r;
                rgba[rgba_index + 1] = g;
                rgba[rgba_index + 2] = b;
                rgba[rgba_index + 3] = 255;
            }
        }

        rgba
    }

    /// Convert NV12 to RGBA
    fn nv12_to_rgba(nv12_data: &[u8], width: u32, height: u32) -> Vec<u8> {
        let w = width as usize;
        let h = height as usize;
        let y_size = w * h;

        let mut rgba = vec![0u8; w * h * 4];

        for y in 0..h {
            for x in 0..w {
                let y_index = y * w + x;
                let uv_index = (y / 2) * w + (x / 2) * 2;

                let y_val = nv12_data[y_index] as f32;
                let u_val = nv12_data[y_size + uv_index] as f32 - 128.0;
                let v_val = nv12_data[y_size + uv_index + 1] as f32 - 128.0;

                // YUV to RGB conversion
                let r = (y_val + 1.402 * v_val).clamp(0.0, 255.0) as u8;
                let g = (y_val - 0.344 * u_val - 0.714 * v_val).clamp(0.0, 255.0) as u8;
                let b = (y_val + 1.772 * u_val).clamp(0.0, 255.0) as u8;

                let rgba_index = y_index * 4;
                rgba[rgba_index] = r;
                rgba[rgba_index + 1] = g;
                rgba[rgba_index + 2] = b;
                rgba[rgba_index + 3] = 255;
            }
        }

        rgba
    }

    /// Render texture to screen with upscaling
    fn render_to_screen(&mut self) -> Result<()> {
        let output = match self.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost) => {
                tracing::warn!("Surface lost, reconfiguring...");
                self.reconfigure();
                return Ok(());
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                return Err(anyhow::anyhow!("Surface out of memory"));
            }
            // All other errors (Outdated, Timeout) should be resolved by the next frame
            Err(e) => {
                tracing::warn!("Skipping frame due to surface error: {:?}", e);
                return Ok(());
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
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);

            if let Some(bind_group) = &self.texture_bind_group {
                render_pass.set_bind_group(0, bind_group, &[]);
            }

            // Calculate Letterboxing (Fit inside window maintaining aspect ratio)
            if self.current_width > 0 && self.current_height > 0 {
                let win_w = self.config.width as f32;
                let win_h = self.config.height as f32;
                let vid_w = self.current_width as f32;
                let vid_h = self.current_height as f32;

                let win_aspect = win_w / win_h;
                let vid_aspect = vid_w / vid_h;

                let (viewport_w, viewport_h, x, y) = if vid_aspect > win_aspect {
                    // Video is wider than window: Fit width, adjust height (bars top/bottom)
                    let scale = win_w / vid_w;
                    let h = vid_h * scale;
                    (win_w, h, 0.0, (win_h - h) / 2.0)
                } else {
                    // Video is taller than window: Fit height, adjust width (bars left/right)
                    let scale = win_h / vid_h;
                    let w = vid_w * scale;
                    (w, win_h, (win_w - w) / 2.0, 0.0)
                };

                render_pass.set_viewport(x, y, viewport_w, viewport_h, 0.0, 1.0);
            }

            render_pass.draw(0..4, 0..1); // Full-screen quad
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    /// Reconfigure surface (e.g. on resize or lost)
    fn reconfigure(&mut self) {
        self.surface.configure(&self.device, &self.config);
    }

    /// Handle window resize
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
        Ok(())
    }

    /// Get window reference
    pub fn window(&self) -> &Window {
        self.window
    }
}
