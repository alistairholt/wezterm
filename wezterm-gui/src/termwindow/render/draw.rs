use crate::termwindow::webgpu::ShaderUniform;

impl crate::TermWindow {
    pub fn call_draw(&mut self) -> anyhow::Result<()> {
        use crate::termwindow::metal_present::drawable_size_for;
        use ::window::WindowOps;

        let webgpu = self.webgpu.as_mut().unwrap();
        let render_state = self.render_state.as_ref().unwrap();

        // Size the drawable from the dimensions we laid out and project
        // for (not from the view bounds), so a frame is never scaled: if
        // they lag the view, the layer's top-left gravity shows it at 1:1.
        let Some(size) = drawable_size_for(
            self.dimensions.pixel_width,
            self.dimensions.pixel_height,
            webgpu.device.limits().max_texture_dimension_2d,
        ) else {
            log::trace!("metal_present: no drawable for {:?}", self.dimensions);
            return Ok(());
        };
        let Some(output) = webgpu
            .presenter
            .begin_frame(&webgpu.device, size, self.live_resizing)
        else {
            // No drawable in time; try again on a later pass. The layer
            // keeps showing the previous frame, unscaled.
            if let Some(window) = self.window.as_ref() {
                window.invalidate();
            }
            return Ok(());
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = webgpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        let atlas = render_state.glyph_cache.borrow().atlas.texture();
        let atlas_bind_groups = webgpu.atlas_bind_groups(&atlas);

        let foreground_text_hsb = self.config.foreground_text_hsb;
        let foreground_text_hsb = [
            foreground_text_hsb.hue,
            foreground_text_hsb.saturation,
            foreground_text_hsb.brightness,
        ];

        let milliseconds = self.created.elapsed().as_millis() as u32;
        let projection = euclid::Transform3D::<f32, f32, f32>::ortho(
            -(self.dimensions.pixel_width as f32) / 2.0,
            self.dimensions.pixel_width as f32 / 2.0,
            self.dimensions.pixel_height as f32 / 2.0,
            -(self.dimensions.pixel_height as f32) / 2.0,
            -1.0,
            1.0,
        )
        .to_arrays_transposed();
        webgpu.write_uniform(ShaderUniform {
            foreground_text_hsb,
            milliseconds,
            projection,
        });

        {
            // One pass for every layer: each extra pass would make the
            // GPU store the whole frame and load it back in. The clear
            // also covers a frame with nothing to draw, whose drawable
            // contents are otherwise undefined (wgpu treats a texture
            // from hal as initialized).
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            render_pass.set_pipeline(&webgpu.render_pipeline);
            render_pass.set_bind_group(0, &webgpu.uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &atlas_bind_groups.linear, &[]);
            render_pass.set_bind_group(2, &atlas_bind_groups.nearest, &[]);

            for layer in render_state.layers.borrow().iter() {
                for idx in 0..3 {
                    let vb = &layer.vb.borrow()[idx];
                    let (vertex_count, index_count) = vb.vertex_index_count();
                    if vertex_count > 0 {
                        let vertices = vb.current_vb_mut();
                        render_pass.set_vertex_buffer(0, vertices.draw_slice(vertex_count));
                        render_pass
                            .set_index_buffer(vb.indices.slice(..), wgpu::IndexFormat::Uint32);
                        render_pass.draw_indexed(0..index_count as _, 0, 0..1);
                    }

                    vb.next_index();
                }
            }
        }
        drop(atlas_bind_groups);

        // submit will accept anything that implements IntoIter
        let submission = webgpu.queue.submit(std::iter::once(encoder.finish()));
        // Vertex buffers drawn above are marked with this frame; they
        // wait for this submission before they are written again.
        webgpu.note_submission(submission);
        drop(view);
        webgpu.presenter.present(&webgpu.queue, output);

        Ok(())
    }
}
