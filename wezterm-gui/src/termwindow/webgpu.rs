use crate::quad::Vertex;
use crate::termwindow::metal_present::MetalPresenter;
use anyhow::anyhow;
use config::{ConfigHandle, GpuInfo, WebGpuPowerPreference};
use std::cell::{Cell, Ref, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use window::bitmaps::Texture2d;
use window::raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};
use window::{BitmapImage, Rect, Window};

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShaderUniform {
    pub foreground_text_hsb: [f32; 3],
    pub milliseconds: u32,
    pub projection: [[f32; 4]; 4],
    // sampler2D atlas_nearest_sampler;
    // sampler2D atlas_linear_sampler;
}

pub struct WebGpuState {
    pub adapter_info: wgpu::AdapterInfo,
    pub downlevel_caps: wgpu::DownlevelCapabilities,
    pub device: wgpu::Device,
    pub queue: Arc<wgpu::Queue>,
    /// Presents into the view's CAMetalLayer; replaces wgpu::Surface.
    pub presenter: MetalPresenter,
    pub render_pipeline: wgpu::RenderPipeline,
    /// One uniform buffer, rewritten each frame by write_uniform.
    uniform_buffer: wgpu::Buffer,
    pub uniform_bind_group: wgpu::BindGroup,
    atlas_bind_groups: RefCell<Option<AtlasBindGroups>>,
    /// Number of the frame being built; goes up on each note_submission.
    frame: Cell<u64>,
    /// The last few frames and their submissions, oldest first.
    submissions: RefCell<VecDeque<(u64, wgpu::SubmissionIndex)>>,
    /// The newest frame known to have finished on the GPU.
    finished_frame: Cell<Option<u64>>,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    texture_nearest_sampler: wgpu::Sampler,
    texture_linear_sampler: wgpu::Sampler,
    pub handle: RawHandlePair,
}

/// Bind groups for one glyph atlas texture; see atlas_bind_groups.
pub struct AtlasBindGroups {
    texture: Rc<dyn Texture2d>,
    pub linear: wgpu::BindGroup,
    pub nearest: wgpu::BindGroup,
}

pub struct RawHandlePair {
    window: RawWindowHandle,
    display: RawDisplayHandle,
}

impl RawHandlePair {
    fn new(window: &Window) -> Self {
        Self {
            window: window.window_handle().expect("window handle").as_raw(),
            display: window.display_handle().expect("display handle").as_raw(),
        }
    }
}

impl HasWindowHandle for RawHandlePair {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        unsafe { Ok(WindowHandle::borrow_raw(self.window)) }
    }
}

impl HasDisplayHandle for RawHandlePair {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        unsafe { Ok(DisplayHandle::borrow_raw(self.display)) }
    }
}

pub struct WebGpuTexture {
    texture: wgpu::Texture,
    width: u32,
    height: u32,
    queue: Arc<wgpu::Queue>,
}

impl std::ops::Deref for WebGpuTexture {
    type Target = wgpu::Texture;
    fn deref(&self) -> &Self::Target {
        &self.texture
    }
}

impl Texture2d for WebGpuTexture {
    fn write(&self, rect: Rect, im: &dyn BitmapImage) {
        let (im_width, im_height) = im.image_dimensions();

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: rect.min_x() as u32,
                    y: rect.min_y() as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            im.pixel_data_slice(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(im_width as u32 * 4),
                rows_per_image: Some(im_height as u32),
            },
            wgpu::Extent3d {
                width: im_width as u32,
                height: im_height as u32,
                depth_or_array_layers: 1,
            },
        );
    }

    fn read(&self, _rect: Rect, _im: &mut dyn BitmapImage) {
        unimplemented!();
    }

    fn width(&self) -> usize {
        self.width as usize
    }

    fn height(&self) -> usize {
        self.height as usize
    }
}

impl WebGpuTexture {
    pub fn new(width: u32, height: u32, state: &WebGpuState) -> anyhow::Result<Self> {
        let limit = state.device.limits().max_texture_dimension_2d;

        if width > limit || height > limit {
            // Ideally, wgpu would have a fallible create_texture method,
            // but it doesn't: instead it will panic if the requested
            // dimension is too large.
            // So we check the limit ourselves here.
            // <https://github.com/wezterm/wezterm/issues/3713>
            anyhow::bail!(
                "texture dimensions {width}x{height} exceeed the \
                 max dimension {limit} supported by your GPU"
            );
        }

        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let view_formats = if state
            .downlevel_caps
            .flags
            .contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS)
        {
            vec![format, format.remove_srgb_suffix()]
        } else {
            vec![]
        };
        let texture = state.device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("Texture Atlas"),
            view_formats: &view_formats,
        });
        Ok(Self {
            texture,
            width,
            height,
            queue: Arc::clone(&state.queue),
        })
    }
}

pub fn adapter_info_to_gpu_info(info: wgpu::AdapterInfo) -> GpuInfo {
    GpuInfo {
        name: info.name,
        vendor: Some(info.vendor),
        device: Some(info.device),
        device_type: format!("{:?}", info.device_type),
        driver: if info.driver.is_empty() {
            None
        } else {
            Some(info.driver)
        },
        driver_info: if info.driver_info.is_empty() {
            None
        } else {
            Some(info.driver_info)
        },
        backend: format!("{:?}", info.backend),
    }
}

fn compute_adapter_list(instance: &wgpu::Instance, backends: wgpu::Backends) -> Vec<String> {
    instance
        .enumerate_adapters(backends)
        .into_iter()
        .map(|a| adapter_info_to_gpu_info(a.get_info()).to_string())
        .collect()
}

/// The CAMetalLayer backing the window's view. The window crate makes it
/// (make_backing_layer) and keeps its contentsScale in step with the
/// window's backing scale.
#[allow(unexpected_cfgs)] // <https://github.com/SSheldon/rust-objc/issues/125>
fn view_layer(handle: &RawHandlePair) -> anyhow::Result<*mut objc::runtime::Object> {
    use objc::{msg_send, sel, sel_impl};
    match handle.window {
        RawWindowHandle::AppKit(h) => {
            let view = h.ns_view.as_ptr() as *mut objc::runtime::Object;
            let layer: *mut objc::runtime::Object = unsafe { msg_send![view, layer] };
            anyhow::ensure!(!layer.is_null(), "view has no backing layer");
            Ok(layer)
        }
        other => anyhow::bail!("unsupported window handle {other:?}"),
    }
}

impl WebGpuState {
    pub async fn new(window: &Window, config: &ConfigHandle) -> anyhow::Result<Self> {
        let handle = RawHandlePair::new(window);
        Self::new_impl(handle, config).await
    }

    pub async fn new_impl(handle: RawHandlePair, config: &ConfigHandle) -> anyhow::Result<Self> {
        // We present through our own CAMetalLayer (MetalPresenter), so only
        // Metal adapters can work.
        let backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });
        let layer = view_layer(&handle)?;

        let mut adapter: Option<wgpu::Adapter> = None;

        if let Some(preference) = &config.webgpu_preferred_adapter {
            for a in instance.enumerate_adapters(backends) {
                let info = a.get_info();

                if preference.name != info.name {
                    continue;
                }

                if preference.device_type != format!("{:?}", info.device_type) {
                    continue;
                }

                if preference.backend != format!("{:?}", info.backend) {
                    continue;
                }

                if let Some(driver) = &preference.driver {
                    if *driver != info.driver {
                        continue;
                    }
                }
                if let Some(vendor) = &preference.vendor {
                    if *vendor != info.vendor {
                        continue;
                    }
                }
                if let Some(device) = &preference.device {
                    if *device != info.device {
                        continue;
                    }
                }

                adapter.replace(a);
                break;
            }

            if adapter.is_none() {
                let adapters = compute_adapter_list(&instance, backends);
                log::warn!(
                    "Your webgpu preferred adapter '{}' was either not \
                     found or is not compatible with your display. Available:\n{}",
                    preference.to_string(),
                    adapters.join("\n")
                );
            }
        }

        if adapter.is_none() {
            adapter = Some(
                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: match config.webgpu_power_preference {
                            WebGpuPowerPreference::HighPerformance => {
                                wgpu::PowerPreference::HighPerformance
                            }
                            WebGpuPowerPreference::LowPower => wgpu::PowerPreference::LowPower,
                        },
                        compatible_surface: None,
                        force_fallback_adapter: config.webgpu_force_fallback_adapter,
                    })
                    .await?,
            );
        }

        let adapter = adapter.ok_or_else(|| {
            let adapters = compute_adapter_list(&instance, backends);
            anyhow!(
                "no compatible adapter found. Available:\n{}",
                adapters.join("\n")
            )
        })?;

        let adapter_info = adapter.get_info();
        log::trace!("Using adapter: {adapter_info:?}");
        let downlevel_caps = adapter.get_downlevel_capabilities();
        log::trace!("downlevel_caps: {downlevel_caps:?}");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                // WebGL doesn't support all of wgpu's features, so if
                // we're building for the web we'll have to disable some.
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::downlevel_defaults()
                }
                .using_resolution(adapter.limits()),
                label: None,
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let queue = Arc::new(queue);

        // The sRGB format the surface used to pick on macOS; the layer's
        // pixel format matches it, so blending and colors are unchanged.
        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let presenter = unsafe { MetalPresenter::new(layer, &device, format)? };
        presenter.set_colorspace(config.macos_colorspace);

        let shader = device.create_shader_module(wgpu::include_wgsl!("../shader.wgsl"));

        let shader_uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("ShaderUniform bind group layout"),
            });

        let texture_nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let texture_linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
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
                label: Some("texture bind group layout"),
            });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &shader_uniform_bind_group_layout,
                    &texture_bind_group_layout,
                    &texture_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),

            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
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

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ShaderUniform Buffer"),
            contents: bytemuck::cast_slice(&[ShaderUniform::default()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &shader_uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("ShaderUniform Bind Group"),
        });

        Ok(Self {
            adapter_info,
            downlevel_caps,
            device,
            queue,
            presenter,
            render_pipeline,
            handle,
            uniform_buffer,
            uniform_bind_group,
            atlas_bind_groups: RefCell::new(None),
            frame: Cell::new(0),
            submissions: RefCell::new(VecDeque::new()),
            finished_frame: Cell::new(None),
            texture_bind_group_layout,
            texture_nearest_sampler,
            texture_linear_sampler,
        })
    }

    /// The number of the frame being built.
    pub fn current_frame(&self) -> u64 {
        self.frame.get()
    }

    /// Record the submission that draws the current frame, and move on
    /// to the next frame.
    pub fn note_submission(&self, index: wgpu::SubmissionIndex) {
        let frame = self.frame.get();
        let mut submissions = self.submissions.borrow_mut();
        submissions.push_back((frame, index));
        // Buffers are reused three frames later; a few more for slack.
        while submissions.len() > 8 {
            submissions.pop_front();
        }
        self.frame.set(frame + 1);
    }

    /// Block until the GPU has finished `frame`. Submissions finish in
    /// order, so if `frame` has dropped out of the list, waiting for the
    /// oldest one still in it covers it too.
    pub fn wait_for_frame(&self, frame: u64) {
        // Every vertex buffer asks; poll only when this frame isn't
        // already known to be done.
        if self.finished_frame.get().is_some_and(|done| done >= frame) {
            return;
        }
        let found = self
            .submissions
            .borrow()
            .iter()
            .find(|(f, _)| *f >= frame)
            .map(|(f, index)| (*f, index.clone()));
        if let Some((waited_frame, index)) = found {
            match self
                .device
                .poll(wgpu::PollType::WaitForSubmissionIndex(index))
            {
                Ok(_) => self.finished_frame.set(Some(waited_frame)),
                Err(err) => log::error!("waiting for frame {frame} on the GPU: {err:#}"),
            }
        }
    }

    /// Set this frame's uniforms. The write is queue-ordered: it lands
    /// before the next submit's commands and after the previous ones, so
    /// a frame still on the GPU keeps its own values.
    pub fn write_uniform(&self, uniform: ShaderUniform) {
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    /// The linear and nearest bind groups for the glyph atlas `texture`,
    /// made again only when the atlas texture is replaced (it is
    /// reallocated when it grows or the font scale changes).
    pub fn atlas_bind_groups(&self, texture: &Rc<dyn Texture2d>) -> Ref<'_, AtlasBindGroups> {
        let stale = match &*self.atlas_bind_groups.borrow() {
            // The cache holds a clone of the Rc, so the address can't be
            // reused by a new texture while it is cached.
            Some(cached) => !std::ptr::addr_eq(Rc::as_ptr(&cached.texture), Rc::as_ptr(texture)),
            None => true,
        };
        if stale {
            let tex = texture.downcast_ref::<WebGpuTexture>().unwrap();
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            let make = |sampler: &wgpu::Sampler, label| {
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &self.texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(sampler),
                        },
                    ],
                    label: Some(label),
                })
            };
            let linear = make(&self.texture_linear_sampler, "linear bind group");
            let nearest = make(&self.texture_nearest_sampler, "nearest bind group");
            self.atlas_bind_groups.replace(Some(AtlasBindGroups {
                texture: Rc::clone(texture),
                linear,
                nearest,
            }));
        }
        Ref::map(self.atlas_bind_groups.borrow(), |c| c.as_ref().unwrap())
    }
}
