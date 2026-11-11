#![allow(unexpected_cfgs)] // <https://github.com/SSheldon/rust-objc/issues/125>
//! Presents WebGpu frames into the view's CAMetalLayer without a
//! `wgpu::Surface`.
//!
//! wgpu's Metal surface resets `presentsWithTransaction` on every
//! `configure`, and we configure on every live-resize step, so we can't
//! present in step with AppKit's window resize through it. Instead we own
//! the layer: we set its drawable size, get each drawable ourselves, wrap
//! its texture for wgpu, and present it either asynchronously (normal
//! frames) or synchronously inside the current Core Animation transaction
//! (any frame whose size changed, and every live-resize frame), so the
//! window's new frame and the matching pixels show up together.
//!
//! See docs/internal/metal-renderer-plan.md (branch metal-design).

use core_graphics_types::geometry::CGSize;
use metal::foreign_types::ForeignTypeRef;
use objc::runtime::{Object, BOOL, YES};
use objc::{class, msg_send, sel, sel_impl};
use std::cell::Cell;
use std::time::{Duration, Instant};
use wgpu::hal::api::Metal;

/// Synchronous presents slower than this are logged at debug level.
const SLOW_SYNC_STEP: Duration = Duration::from_millis(12);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentMode {
    /// `presentDrawable:` on a command buffer; returns without waiting.
    Async,
    /// Commit, `waitUntilScheduled`, then `[drawable present]`, with the
    /// layer's `presentsWithTransaction` set, so the frame lands in the
    /// same Core Animation transaction as the window resize.
    Sync,
}

/// Any live-resize frame, and any frame whose size changed (zoom, full
/// screen, font scale, a move to a screen with a different scale), is
/// presented in step with the window; everything else is async.
pub fn choose_present_mode(live_resizing: bool, size_changed: bool) -> PresentMode {
    if live_resizing || size_changed {
        PresentMode::Sync
    } else {
        PresentMode::Async
    }
}

/// The drawable size for a frame rendered at `pixel_width` x `pixel_height`,
/// or None if we must not draw: wgpu panics on a 0 sized texture
/// (<https://github.com/wezterm/wezterm/issues/2881>), and Metal can't make
/// one wider or taller than `max_dimension`.
pub fn drawable_size_for(
    pixel_width: usize,
    pixel_height: usize,
    max_dimension: u32,
) -> Option<(u32, u32)> {
    let w: u32 = std::convert::TryFrom::try_from(pixel_width).ok()?;
    let h: u32 = std::convert::TryFrom::try_from(pixel_height).ok()?;
    if w == 0 || h == 0 || w > max_dimension || h > max_dimension {
        None
    } else {
        Some((w, h))
    }
}

#[link(name = "QuartzCore", kind = "framework")]
extern "C" {
    static kCAGravityTopLeft: *mut Object;
    static kCAGravityBottomLeft: *mut Object;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    static kCGColorSpaceSRGB: *const std::ffi::c_void;
    static kCGColorSpaceDisplayP3: *const std::ffi::c_void;
    fn CGColorSpaceCreateWithName(name: *const std::ffi::c_void) -> *mut std::ffi::c_void;
    fn CGColorSpaceRelease(space: *mut std::ffi::c_void);
}

/// Tag the layer's pixels with `space`, so Core Animation converts them
/// to the display's colors. `None` leaves the layer untagged; on macOS 27
/// the window server then treats the pixels as sRGB, the same as `sRGB`.
unsafe fn set_layer_colorspace(layer: *mut Object, space: config::MacOsColorSpace) {
    let name = match space {
        config::MacOsColorSpace::sRGB => kCGColorSpaceSRGB,
        config::MacOsColorSpace::DisplayP3 => kCGColorSpaceDisplayP3,
        config::MacOsColorSpace::None => std::ptr::null(),
    };
    let cs = if name.is_null() {
        std::ptr::null_mut()
    } else {
        CGColorSpaceCreateWithName(name)
    };
    // The layer retains the color space.
    let () = msg_send![layer, setColorspace: cs];
    if !cs.is_null() {
        CGColorSpaceRelease(cs);
    }
}

/// The layer's `contentsGravity`, as a Rust string.
unsafe fn layer_gravity(layer: *mut Object) -> String {
    let gravity: *mut Object = msg_send![layer, contentsGravity];
    if gravity.is_null() {
        return String::new();
    }
    let utf8: *const std::os::raw::c_char = msg_send![gravity, UTF8String];
    std::ffi::CStr::from_ptr(utf8)
        .to_string_lossy()
        .into_owned()
}

/// Pin undrawn content to the visual top-left instead of stretching it.
///
/// CALayer gravity names use the layer's own y axis ("top" is maximum Y),
/// so which name is the visual top depends on whether the layer's geometry
/// ends up flipped. AppKit flips backing layers of flipped views.
unsafe fn pin_contents_top_left(layer: *mut Object) {
    let before = layer_gravity(layer);
    let geometry_flipped: BOOL = msg_send![layer, isGeometryFlipped];
    let flipped: BOOL = msg_send![layer, contentsAreFlipped];
    let gravity = if flipped == YES {
        kCAGravityBottomLeft
    } else {
        kCAGravityTopLeft
    };
    let () = msg_send![layer, setContentsGravity: gravity];
    // Logged so an on-screen check can tell which way this went.
    log::info!(
        "metal_present: geometryFlipped={} contentsAreFlipped={} gravity {} -> {}",
        geometry_flipped == YES,
        flipped == YES,
        before,
        layer_gravity(layer)
    );
}

pub struct MetalPresenter {
    layer: metal::MetalLayer,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    /// Size of the last drawable we got. Compared with this rather than
    /// the layer's drawableSize, which Core Animation may update on its
    /// own when the bounds or scale change.
    last_size: Cell<Option<(u32, u32)>>,
    #[cfg(test)]
    pub(crate) fail_next_drawable: Cell<bool>,
}

/// One acquired drawable, wrapped as a wgpu texture to render into.
pub struct PresentFrame {
    pub texture: wgpu::Texture,
    pub size: (u32, u32),
    drawable: metal::MetalDrawable,
    mode: PresentMode,
    started: Instant,
    acquired: Instant,
}

impl PresentFrame {
    pub fn mode(&self) -> PresentMode {
        self.mode
    }
}

impl MetalPresenter {
    /// Take over `layer`, which must be a CAMetalLayer, for `device`.
    /// `format` must be `Bgra8UnormSrgb` or `Bgra8Unorm`.
    ///
    /// # Safety
    /// `layer` must be a valid CAMetalLayer pointer; the presenter
    /// retains it.
    pub unsafe fn new(
        layer: *mut Object,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        Self::new_impl(layer, device, format, false)
    }

    /// Set the color space the layer's pixels are tagged with. Cheap
    /// enough to call on every config reload.
    pub fn set_colorspace(&self, space: config::MacOsColorSpace) {
        unsafe { set_layer_colorspace(self.layer.as_ptr() as *mut Object, space) };
    }

    /// `readable` makes drawable textures copyable, for tests only; it
    /// turns off the layer's `framebufferOnly` optimization.
    pub(crate) unsafe fn new_impl(
        layer: *mut Object,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        readable: bool,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(!layer.is_null(), "no layer to present into");
        let is_metal: BOOL = msg_send![layer, isKindOfClass: class!(CAMetalLayer)];
        anyhow::ensure!(is_metal == YES, "view layer is not a CAMetalLayer");

        let pixel_format = match format {
            wgpu::TextureFormat::Bgra8UnormSrgb => metal::MTLPixelFormat::BGRA8Unorm_sRGB,
            wgpu::TextureFormat::Bgra8Unorm => metal::MTLPixelFormat::BGRA8Unorm,
            _ => anyhow::bail!("unsupported presenter format {format:?}"),
        };

        let raw_device = device
            .as_hal::<Metal, _, _>(|d| d.map(|d| d.raw_device().lock().clone()))
            .ok_or_else(|| anyhow::anyhow!("wgpu device is not a Metal device"))?;

        // Retains the layer.
        let layer = metal::MetalLayerRef::from_ptr(layer as *mut _).to_owned();
        layer.set_device(&raw_device);
        layer.set_pixel_format(pixel_format);
        layer.set_framebuffer_only(!readable);
        // Not opaque, so window_background_opacity works.
        layer.set_opaque(false);
        layer.set_maximum_drawable_count(3);
        layer.set_display_sync_enabled(true);
        layer.set_presents_with_transaction(false);
        // nextDrawable returns nil after about a second instead of
        // blocking the main thread forever.
        let () = msg_send![layer.as_ptr() as *mut Object, setAllowsNextDrawableTimeout: YES];
        pin_contents_top_left(layer.as_ptr() as *mut Object);

        let mut usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        if readable {
            usage |= wgpu::TextureUsages::COPY_SRC;
        }

        Ok(Self {
            layer,
            format,
            usage,
            last_size: Cell::new(None),
            #[cfg(test)]
            fail_next_drawable: Cell::new(false),
        })
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    fn next_drawable(&self) -> Option<metal::MetalDrawable> {
        #[cfg(test)]
        if self.fail_next_drawable.replace(false) {
            return None;
        }
        objc::rc::autoreleasepool(|| self.layer.next_drawable().map(|d| d.to_owned()))
    }

    /// Get a drawable of exactly `size` pixels to render into. Returns
    /// None when no drawable is available (timeout, or all of them are in
    /// flight); the caller should skip this frame and try again later.
    pub fn begin_frame(
        &self,
        device: &wgpu::Device,
        size: (u32, u32),
        live_resizing: bool,
    ) -> Option<PresentFrame> {
        let started = Instant::now();
        let size_changed = self.last_size.get() != Some(size);
        let mode = choose_present_mode(live_resizing, size_changed);

        let current = self.layer.drawable_size();
        if (current.width, current.height) != (size.0 as f64, size.1 as f64) {
            self.layer
                .set_drawable_size(CGSize::new(size.0 as f64, size.1 as f64));
        }
        let sync = mode == PresentMode::Sync;
        if self.layer.presents_with_transaction() != sync {
            self.layer.set_presents_with_transaction(sync);
        }

        let Some(drawable) = self.next_drawable() else {
            log::debug!(
                "metal_present: no drawable for {}x{} after {:?}; skipping frame",
                size.0,
                size.1,
                started.elapsed()
            );
            return None;
        };
        let raw_texture = drawable.texture().to_owned();
        let tex_size = (raw_texture.width() as u32, raw_texture.height() as u32);
        if tex_size != size {
            // Should not happen: we just set drawableSize.
            log::warn!(
                "metal_present: drawable is {}x{}, wanted {}x{}",
                tex_size.0,
                tex_size.1,
                size.0,
                size.1
            );
        }

        let hal_texture = unsafe {
            wgpu::hal::metal::Device::texture_from_raw(
                raw_texture,
                self.format,
                metal::MTLTextureType::D2,
                1,
                1,
                wgpu::hal::CopyExtent {
                    width: tex_size.0,
                    height: tex_size.1,
                    depth: 1,
                },
            )
        };
        let texture = unsafe {
            device.create_texture_from_hal::<Metal>(
                hal_texture,
                &wgpu::TextureDescriptor {
                    label: Some("CAMetalLayer drawable"),
                    size: wgpu::Extent3d {
                        width: tex_size.0,
                        height: tex_size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.format,
                    usage: self.usage,
                    view_formats: &[],
                },
            )
        };

        self.last_size.set(Some(size));
        Some(PresentFrame {
            texture,
            size: tex_size,
            drawable,
            mode,
            started,
            acquired: Instant::now(),
        })
    }

    /// Present `frame`. Call this after `queue.submit` of the work that
    /// renders into it: command buffers on one queue are scheduled in
    /// order, so ours is scheduled after wgpu's.
    pub fn present(&self, queue: &wgpu::Queue, frame: PresentFrame) {
        let PresentFrame {
            texture,
            size,
            drawable,
            mode,
            started,
            acquired,
        } = frame;
        let presenting = Instant::now();

        unsafe {
            queue.as_hal::<Metal, _, _>(|q| {
                let Some(q) = q else {
                    log::error!("metal_present: wgpu queue is not a Metal queue");
                    return;
                };
                let raw_queue = q.as_raw().lock();
                objc::rc::autoreleasepool(|| {
                    let cb = raw_queue.new_command_buffer();
                    cb.set_label("wezterm present");
                    match mode {
                        PresentMode::Async => {
                            cb.present_drawable(&drawable);
                            cb.commit();
                        }
                        PresentMode::Sync => {
                            // Apple's presentsWithTransaction order: commit,
                            // wait until scheduled, then present directly.
                            cb.commit();
                            cb.wait_until_scheduled();
                            drawable.present();
                        }
                    }
                });
            });
        }
        drop(texture);

        let done = Instant::now();
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let acquire = acquired - started;
        let paint = presenting - acquired;
        let wait = done - presenting;
        let total = done - started;
        log::trace!(
            "metal_present: step size={}x{} mode={:?} acquire_ms={:.2} paint_ms={:.2} wait_ms={:.2} total_ms={:.2}",
            size.0,
            size.1,
            mode,
            ms(acquire),
            ms(paint),
            ms(wait),
            ms(total),
        );
        if mode == PresentMode::Sync && total > SLOW_SYNC_STEP {
            log::debug!(
                "metal_present: slow sync step size={}x{} acquire_ms={:.2} paint_ms={:.2} wait_ms={:.2} total_ms={:.2}",
                size.0,
                size.1,
                ms(acquire),
                ms(paint),
                ms(wait),
                ms(total),
            );
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use metal::foreign_types::ForeignType;

    // Pure policy: any size change or live resize forces a
    // transaction-synced present; everything else presents async.
    #[test]
    fn present_mode_policy() {
        assert_eq!(choose_present_mode(false, false), PresentMode::Async);
        assert_eq!(choose_present_mode(true, false), PresentMode::Sync);
        assert_eq!(choose_present_mode(false, true), PresentMode::Sync);
        assert_eq!(choose_present_mode(true, true), PresentMode::Sync);
    }

    // The drawable is never 0 sized (wgpu panics on 0 sized textures) and
    // never larger than the GPU can make.
    #[test]
    fn zero_size_is_skipped() {
        assert_eq!(drawable_size_for(800, 600, 8192), Some((800, 600)));
        assert_eq!(drawable_size_for(0, 600, 8192), None);
        assert_eq!(drawable_size_for(800, 0, 8192), None);
        assert_eq!(drawable_size_for(9000, 600, 8192), None);
        assert_eq!(drawable_size_for(8192, 8192, 8192), Some((8192, 8192)));
    }

    struct Gpu {
        device: wgpu::Device,
        queue: wgpu::Queue,
    }

    /// A Metal device, or None (and a note on stderr) when the machine
    /// has none, so these tests don't fail on a GPU-less CI runner.
    fn gpu() -> Option<Gpu> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..Default::default()
        });
        let adapter = match smol::block_on(instance.request_adapter(&Default::default())) {
            Ok(a) => a,
            Err(err) => {
                eprintln!("skipping: no Metal adapter: {err:#}");
                return None;
            }
        };
        let (device, queue) = smol::block_on(adapter.request_device(&Default::default())).ok()?;
        Some(Gpu { device, queue })
    }

    /// A CAMetalLayer that is not attached to any window, so no GUI opens.
    fn detached_layer() -> metal::MetalLayer {
        metal::MetalLayer::new()
    }

    fn presenter(gpu: &Gpu, layer: &metal::MetalLayer) -> MetalPresenter {
        unsafe {
            MetalPresenter::new_impl(
                layer.as_ptr() as *mut Object,
                &gpu.device,
                wgpu::TextureFormat::Bgra8UnormSrgb,
                true,
            )
        }
        .unwrap()
    }

    /// Present inside an explicit CATransaction, as AppKit's display pass
    /// would provide one; test threads have no run loop to commit an
    /// implicit transaction.
    fn in_transaction<R>(f: impl FnOnce() -> R) -> R {
        unsafe {
            let () = msg_send![class!(CATransaction), begin];
        }
        let r = f();
        unsafe {
            let () = msg_send![class!(CATransaction), commit];
        }
        r
    }

    /// Clear the frame to `color`, copy its first pixel back, present it,
    /// and return the BGRA bytes wgpu wrote into the drawable.
    fn clear_and_present(
        gpu: &Gpu,
        presenter: &MetalPresenter,
        frame: PresentFrame,
        color: wgpu::Color,
    ) -> [u8; 4] {
        let view = frame.texture.create_view(&Default::default());
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        {
            let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        enc.copy_texture_to_buffer(
            frame.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit(Some(enc.finish()));
        presenter.present(&gpu.queue, frame);

        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        gpu.device.poll(wgpu::PollType::Wait).unwrap();
        let data = slice.get_mapped_range();
        [data[0], data[1], data[2], data[3]]
    }

    // Headless: our own CAMetalLayer hands out drawables of exactly the
    // size asked for on every step, growing and shrinking, in both present
    // modes, and the pixels wgpu renders land in them.
    #[test]
    fn owned_layer_presents_exact_size() {
        let Some(gpu) = gpu() else { return };
        let layer = detached_layer();
        let presenter = presenter(&gpu, &layer);

        let sizes = [
            (800, 600),
            (807, 603),
            (814, 600),
            (400, 300),
            (401, 301),
            (1200, 900),
            (1200, 900),
            (640, 480),
            (641, 480),
            (641, 480),
        ];
        for (i, &size) in sizes.iter().enumerate() {
            let live = i % 2 == 0;
            let red = i % 2 == 0;
            let color = if red {
                wgpu::Color::RED
            } else {
                wgpu::Color::GREEN
            };
            let px = in_transaction(|| {
                let frame = presenter
                    .begin_frame(&gpu.device, size, live)
                    .expect("a drawable");
                assert_eq!(frame.size, size, "step {i}");
                assert_eq!(
                    (frame.texture.width(), frame.texture.height()),
                    size,
                    "step {i}"
                );
                clear_and_present(&gpu, &presenter, frame, color)
            });
            // BGRA order; 1.0 and 0.0 are the same in sRGB and linear.
            let want = if red {
                [0, 0, 255, 255]
            } else {
                [0, 255, 0, 255]
            };
            assert_eq!(px, want, "step {i}");
        }
    }

    // A changed size, or a live resize, must flip the layer into
    // transaction-synced presents; a steady frame must flip it back so
    // normal frames don't block on the GPU.
    #[test]
    fn present_mode_drives_presents_with_transaction() {
        let Some(gpu) = gpu() else { return };
        let layer = detached_layer();
        let presenter = presenter(&gpu, &layer);

        let cases = [
            // (size, live_resizing, expected mode)
            ((800, 600), false, PresentMode::Sync), // first frame: size changed
            ((800, 600), false, PresentMode::Async),
            ((800, 600), true, PresentMode::Sync),
            ((810, 600), false, PresentMode::Sync),
            ((810, 600), false, PresentMode::Async),
        ];
        for (i, &(size, live, want)) in cases.iter().enumerate() {
            in_transaction(|| {
                let frame = presenter
                    .begin_frame(&gpu.device, size, live)
                    .expect("a drawable");
                assert_eq!(frame.mode(), want, "case {i}");
                assert_eq!(
                    layer.presents_with_transaction(),
                    want == PresentMode::Sync,
                    "case {i}"
                );
                clear_and_present(&gpu, &presenter, frame, wgpu::Color::BLACK);
            });
        }
    }

    // A nil drawable (timeout, or all drawables in flight) is a skipped
    // frame, not a panic, and the next frame works again.
    #[test]
    fn nil_drawable_skips_frame() {
        let Some(gpu) = gpu() else { return };
        let layer = detached_layer();
        let presenter = presenter(&gpu, &layer);

        presenter.fail_next_drawable.set(true);
        assert!(presenter
            .begin_frame(&gpu.device, (800, 600), true)
            .is_none());

        in_transaction(|| {
            let frame = presenter
                .begin_frame(&gpu.device, (800, 600), true)
                .expect("a drawable after a skipped frame");
            clear_and_present(&gpu, &presenter, frame, wgpu::Color::BLACK);
        });
    }

    // The layer is set up for transparency and a non-stretching gravity,
    // and with the scale left alone (the window crate owns contentsScale).
    #[test]
    fn layer_is_configured() {
        let Some(gpu) = gpu() else { return };
        let layer = detached_layer();
        layer.set_contents_scale(2.0);
        let _presenter = presenter(&gpu, &layer);

        assert!(!layer.is_opaque());
        assert_eq!(layer.pixel_format(), metal::MTLPixelFormat::BGRA8Unorm_sRGB);
        assert_eq!(layer.contents_scale(), 2.0);
        let gravity = unsafe { layer_gravity(layer.as_ptr() as *mut Object) };
        assert!(
            gravity == "topLeft" || gravity == "bottomLeft",
            "gravity is {gravity}"
        );
    }

    // Dropping the presenter right after a synchronous present leaves the
    // device in a clean state.
    #[test]
    fn presenter_drop_after_present_is_clean() {
        let Some(gpu) = gpu() else { return };
        let layer = detached_layer();
        let presenter = presenter(&gpu, &layer);
        in_transaction(|| {
            let frame = presenter
                .begin_frame(&gpu.device, (320, 200), true)
                .expect("a drawable");
            assert_eq!(frame.mode(), PresentMode::Sync);
            clear_and_present(&gpu, &presenter, frame, wgpu::Color::BLACK);
        });
        drop(presenter);
        drop(layer);
        assert!(gpu.device.poll(wgpu::PollType::Wait).is_ok());
    }
}
