//! Throwaway spike for kata 021q. Not production code.
//!
//! Question: with wgpu 25, can wezterm own the CAMetalLayer and the drawable
//! (no wgpu::Surface), render into the drawable texture with wgpu, and present
//! it synchronously inside a CATransaction (presentsWithTransaction +
//! waitUntilScheduled), at a new drawable size on every "resize step"?
//!
//! Headless: the layer is not attached to a window. No GUI is opened.

use core_graphics_types::geometry::CGSize;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::time::Instant;
use wgpu::hal::api::Metal;

fn main() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    println!("adapter: {:?}", adapter.get_info().name);
    let (device, queue) =
        pollster::block_on(adapter.request_device(&Default::default())).unwrap();

    // The wgpu device's MTLDevice, so the layer hands out textures that wgpu can use.
    let raw_device: metal::Device = unsafe {
        device.as_hal::<Metal, _, _>(|d| d.unwrap().raw_device().lock().clone())
    };

    // Our own CAMetalLayer, configured by us, never touched by wgpu.
    let layer = metal::MetalLayer::new();
    layer.set_device(&raw_device);
    layer.set_pixel_format(metal::MTLPixelFormat::BGRA8Unorm);
    layer.set_framebuffer_only(false); // only so the spike can read pixels back
    layer.set_presents_with_transaction(true);
    layer.set_maximum_drawable_count(3);
    layer.set_display_sync_enabled(true);
    let () = unsafe { msg_send![layer.as_ref(), setAllowsNextDrawableTimeout: true] };

    // Simulated live-resize steps (pixel sizes).
    let steps: Vec<(u32, u32)> = (0..40)
        .map(|i| (800 + i * 7, 600 + (i % 5) * 3))
        .collect();

    let mut worst_ms = 0f64;
    let mut ok = 0;
    for (i, &(w, h)) in steps.iter().enumerate() {
        let t0 = Instant::now();
        unsafe {
            let () = msg_send![class!(CATransaction), begin];
        }

        // 1. New drawable size for this step.
        layer.set_drawable_size(CGSize::new(w as f64, h as f64));

        // 2. Acquire a drawable ourselves.
        let drawable = match layer.next_drawable() {
            Some(d) => d.to_owned(),
            None => {
                println!("step {i}: nextDrawable returned nil");
                unsafe {
                    let () = msg_send![class!(CATransaction), commit];
                }
                continue;
            }
        };
        let t_acquire = t0.elapsed();
        let mtl_tex = drawable.texture().to_owned();
        let (tw, th) = (mtl_tex.width() as u32, mtl_tex.height() as u32);

        // 3. Wrap the drawable texture as a wgpu::Texture.
        let hal_tex = unsafe {
            wgpu::hal::metal::Device::texture_from_raw(
                mtl_tex,
                wgpu::TextureFormat::Bgra8Unorm,
                metal::MTLTextureType::D2,
                1,
                1,
                wgpu::hal::CopyExtent {
                    width: tw,
                    height: th,
                    depth: 1,
                },
            )
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("drawable"),
            size: wgpu::Extent3d {
                width: tw,
                height: th,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        let tex = unsafe { device.create_texture_from_hal::<Metal>(hal_tex, &desc) };
        let view = tex.create_view(&Default::default());

        // 4. Render with wgpu (a clear stands in for the quad pass) and copy one
        //    row back so we can check wgpu really wrote into the drawable.
        let red = (i as f64 / steps.len() as f64).min(1.0);
        let padded = ((tw * 4 + 255) / 256) * 256;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: padded as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: red,
                            g: 0.5,
                            b: 0.25,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: tw,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(Some(enc.finish()));

        // 5. Synchronous present: an empty command buffer on wgpu's own
        //    MTLCommandQueue is scheduled after the work above (same queue),
        //    wait for it to be scheduled, then present inside the transaction.
        unsafe {
            queue.as_hal::<Metal, _, _>(|q| {
                let q = q.unwrap().as_raw().lock();
                let cb = q.new_command_buffer();
                cb.commit();
                cb.wait_until_scheduled();
            });
        }
        drawable.present();
        unsafe {
            let () = msg_send![class!(CATransaction), commit];
        }
        let t_present = t0.elapsed();

        // Check pixels (outside the timed region).
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::Wait).unwrap();
        let data = slice.get_mapped_range();
        let px = [data[0], data[1], data[2], data[3]];
        drop(data);
        readback.unmap();
        let want_r = (red * 255.0).round() as i32;
        let good = (tw, th) == (w, h) && (px[2] as i32 - want_r).abs() <= 1 && px[1] >= 127;
        if good {
            ok += 1;
        }
        let ms = t_present.as_secs_f64() * 1000.0;
        worst_ms = worst_ms.max(ms);
        if i < 3 || !good || i == steps.len() - 1 {
            println!(
                "step {i}: want {w}x{h} got {tw}x{th} bgra={px:?} acquire={:.2}ms acquire+render+present={ms:.2}ms {}",
                t_acquire.as_secs_f64() * 1000.0,
                if good { "OK" } else { "MISMATCH" }
            );
        }
    }
    let pwt: bool = unsafe {
        let l: *mut Object = layer.as_ref() as *const _ as *mut Object;
        let v: objc::runtime::BOOL = msg_send![l, presentsWithTransaction];
        v == objc::runtime::YES
    };
    println!(
        "{ok}/{} steps OK; worst step {worst_ms:.2} ms; presentsWithTransaction still {pwt}",
        steps.len()
    );
}
