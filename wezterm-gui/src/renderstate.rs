use super::glyphcache::GlyphCache;
use super::quad::*;
use super::utilsprites::{RenderMetrics, UtilSprites};
use crate::termwindow::webgpu::{adapter_info_to_gpu_info, WebGpuState, WebGpuTexture};
use ::window::bitmaps::atlas::OutOfTextureSpace;
use ::window::bitmaps::Texture2d;
use anyhow::Context;
use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;
use wezterm_font::FontConfiguration;
use wgpu::util::DeviceExt;

const INDICES_PER_CELL: usize = 6;

/// The GPU state that render resources are allocated from.
#[derive(Clone)]
pub struct RenderContext(Rc<WebGpuState>);

impl RenderContext {
    pub fn new(state: Rc<WebGpuState>) -> Self {
        Self(state)
    }

    pub fn allocate_index_buffer(&self, indices: &[u32]) -> WebGpuIndexBuffer {
        WebGpuIndexBuffer::new(indices, &self.0)
    }

    pub fn allocate_vertex_buffer(&self, num_quads: usize) -> WebGpuVertexBuffer {
        WebGpuVertexBuffer::new(num_quads * VERTICES_PER_CELL, &self.0)
    }

    pub fn allocate_texture_atlas(&self, size: usize) -> anyhow::Result<Rc<dyn Texture2d>> {
        let texture: Rc<dyn Texture2d> =
            Rc::new(WebGpuTexture::new(size as u32, size as u32, &self.0)?);
        Ok(texture)
    }

    pub fn renderer_info(&self) -> String {
        let info = adapter_info_to_gpu_info(self.0.adapter_info.clone());
        format!("WebGPU: {}", info.to_string())
    }
}

pub struct MappedQuads<'a> {
    mapping: WebGpuMappedVertexBuffer,
    next: RefMut<'a, usize>,
    capacity: usize,
}

impl<'a> MappedQuads<'a> {
    /// Writes cached quads straight into the mapped buffer, without
    /// building each quad's vertices on the stack and copying them in.
    pub fn extend_with_quads(&mut self, quads: &[BoxedQuad]) {
        let idx = *self.next;
        *self.next += quads.len();
        // As in extend_with: only copy in if there is enough room, and
        // let the end of the render pass detect running out of space.
        if idx + quads.len() <= self.capacity {
            let start = idx * VERTICES_PER_CELL;
            let end = start + quads.len() * VERTICES_PER_CELL;
            for (quad, out) in quads
                .iter()
                .zip(self.mapping.mapping[start..end].chunks_exact_mut(VERTICES_PER_CELL))
            {
                quad.write_vertices(out);
            }
        }
    }
}

pub struct WebGpuMappedVertexBuffer {
    mapping: &'static mut [Vertex],
    // Owner of the vertices, must be dropped after mapping
    _owner: RefMut<'static, WebGpuVertexBuffer>,
}

/// A vertex buffer in shared (CPU and GPU visible) Metal memory that
/// lives as long as its capacity doesn't change. Quads are written
/// straight into it and the GPU reads them from there: no staging copy.
///
/// It replaces making a new mapped wgpu buffer each frame, which made
/// wgpu allocate and zero-fill a staging buffer of the whole capacity
/// and then copy all of it.
///
/// The CPU must not write while a submitted frame may still be reading,
/// so `vertices_mut` first waits for the frame that last drew from it.
/// TripleVertexBuffer rotates three of these, so that frame is three
/// frames old and has almost always finished.
pub struct WebGpuVertexBuffer {
    buf: wgpu::Buffer,
    raw: metal::Buffer,
    num_vertices: usize,
    /// The frame (WebGpuState::current_frame) that last drew from this.
    last_frame: std::cell::Cell<Option<u64>>,
    state: Rc<WebGpuState>,
}

impl WebGpuVertexBuffer {
    pub fn new(num_vertices: usize, state: &Rc<WebGpuState>) -> Self {
        use wgpu::hal::api::Metal;

        // Metal can't make a 0 length buffer.
        let size = (num_vertices * std::mem::size_of::<Vertex>()).max(256) as u64;
        let raw_device = unsafe {
            state
                .device
                .as_hal::<Metal, _, _>(|d| d.map(|d| d.raw_device().lock().clone()))
        }
        .expect("WebGpu device is Metal");
        let raw = raw_device.new_buffer(
            size,
            metal::MTLResourceOptions::StorageModeShared
                | metal::MTLResourceOptions::CPUCacheModeDefaultCache,
        );
        let buf = unsafe {
            // wgpu treats a buffer from hal as fully initialized, so it
            // never clears it over what we wrote.
            let hal_buffer = wgpu::hal::metal::Device::buffer_from_raw(raw.clone(), size);
            state.device.create_buffer_from_hal::<Metal>(
                hal_buffer,
                &wgpu::BufferDescriptor {
                    label: Some("Vertex Buffer"),
                    size,
                    usage: wgpu::BufferUsages::VERTEX,
                    mapped_at_creation: false,
                },
            )
        };
        Self {
            buf,
            raw,
            num_vertices,
            last_frame: std::cell::Cell::new(None),
            state: Rc::clone(state),
        }
    }

    /// The vertices, to fill in for the frame being built.
    ///
    /// # Safety
    /// The slice points into `raw`; the caller must not use it after this
    /// buffer is dropped or drawn from (`draw_slice`).
    unsafe fn vertices_mut(&self) -> &'static mut [Vertex] {
        if let Some(frame) = self.last_frame.get() {
            self.state.wait_for_frame(frame);
        }
        std::slice::from_raw_parts_mut(self.raw.contents() as *mut Vertex, self.num_vertices)
    }

    /// The part of the buffer holding the first `vertex_count` vertices,
    /// for drawing in the current frame.
    pub fn draw_slice(&self, vertex_count: usize) -> wgpu::BufferSlice<'_> {
        self.last_frame.set(Some(self.state.current_frame()));
        let vertex_count = vertex_count.min(self.num_vertices);
        self.buf
            .slice(..(vertex_count * std::mem::size_of::<Vertex>()) as wgpu::BufferAddress)
    }
}

pub struct WebGpuIndexBuffer {
    buf: wgpu::Buffer,
}

impl std::ops::Deref for WebGpuIndexBuffer {
    type Target = wgpu::Buffer;
    fn deref(&self) -> &Self::Target {
        &self.buf
    }
}

impl WebGpuIndexBuffer {
    pub fn new(indices: &[u32], state: &WebGpuState) -> Self {
        Self {
            buf: state
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Index Buffer"),
                    usage: wgpu::BufferUsages::INDEX,
                    contents: bytemuck::cast_slice(indices),
                }),
        }
    }
}

impl<'a> QuadAllocator for MappedQuads<'a> {
    fn allocate<'b>(&'b mut self) -> anyhow::Result<QuadImpl<'b>> {
        let idx = *self.next;
        *self.next += 1;
        let idx = if idx >= self.capacity {
            // We don't have enough quads, so we'll keep re-using
            // the first quad until we reach the end of the render
            // pass, at which point we'll detect this condition
            // and re-allocate the quads.
            0
        } else {
            idx
        };

        let idx = idx * VERTICES_PER_CELL;
        let mut quad = Quad {
            vert: &mut self.mapping.mapping[idx..idx + VERTICES_PER_CELL],
        };

        quad.set_has_color(false);

        Ok(QuadImpl::Vert(quad))
    }

    fn extend_with(&mut self, vertices: &[Vertex]) {
        let idx = *self.next;
        let len = vertices.len();

        // idx and next are number of quads, so divide by number of vertices
        *self.next += len / VERTICES_PER_CELL;
        // Only copy in if there is enough room.
        // We'll detect the out of space condition at the end of
        // the render pass.
        let idx = idx * VERTICES_PER_CELL;
        let capacity = self.capacity * VERTICES_PER_CELL;
        if idx + len <= capacity {
            self.mapping.mapping[idx..idx + len].copy_from_slice(vertices);
        }
    }
}

pub struct TripleVertexBuffer {
    pub index: RefCell<usize>,
    pub bufs: RefCell<[WebGpuVertexBuffer; 3]>,
    pub indices: WebGpuIndexBuffer,
    pub capacity: usize,
    pub next_quad: RefCell<usize>,
}

/// A trait to avoid broadly-scoped transmutes; we only want to
/// transmute to extend a lifetime to static, and not to change
/// the underlying type.
/// These ExtendStatic trait impls constrain the transmutes in that way,
/// so that the type checker can still catch issues.
unsafe trait ExtendStatic {
    type T;
    unsafe fn extend_lifetime(self) -> Self::T;
}

unsafe impl<'a, T: 'static> ExtendStatic for Ref<'a, T> {
    type T = Ref<'static, T>;
    unsafe fn extend_lifetime(self) -> Self::T {
        std::mem::transmute(self)
    }
}

unsafe impl<'a, T: 'static> ExtendStatic for RefMut<'a, T> {
    type T = RefMut<'static, T>;
    unsafe fn extend_lifetime(self) -> Self::T {
        std::mem::transmute(self)
    }
}

unsafe impl<'a> ExtendStatic for MappedQuads<'a> {
    type T = MappedQuads<'static>;
    unsafe fn extend_lifetime(self) -> Self::T {
        std::mem::transmute(self)
    }
}

impl TripleVertexBuffer {
    pub fn clear_quad_allocation(&self) {
        *self.next_quad.borrow_mut() = 0;
    }

    pub fn need_more_quads(&self) -> Option<usize> {
        let next = *self.next_quad.borrow();
        if next > self.capacity {
            Some(next)
        } else {
            None
        }
    }

    pub fn vertex_index_count(&self) -> (usize, usize) {
        let num_quads = *self.next_quad.borrow();
        (num_quads * VERTICES_PER_CELL, num_quads * INDICES_PER_CELL)
    }

    pub fn map(&self) -> MappedQuads<'_> {
        let bufs = self.current_vb_mut();

        // The mapping points into the buffer owned by `bufs`, a RefMut
        // whose lifetime has been extended to static. They travel together
        // in WebGpuMappedVertexBuffer, which drops the owner after the
        // mapping.
        let mapping = unsafe { bufs.vertices_mut() };
        let mapping = WebGpuMappedVertexBuffer {
            mapping,
            _owner: bufs,
        };

        MappedQuads {
            mapping,
            next: self.next_quad.borrow_mut(),
            capacity: self.capacity,
        }
    }

    pub fn current_vb_mut(&self) -> RefMut<'static, WebGpuVertexBuffer> {
        let index = *self.index.borrow();
        let bufs = self.bufs.borrow_mut();
        unsafe { RefMut::map(bufs, |bufs| &mut bufs[index]).extend_lifetime() }
    }

    pub fn next_index(&self) {
        let mut index = self.index.borrow_mut();
        *index += 1;
        if *index >= 3 {
            *index = 0;
        }
    }
}

pub struct RenderLayer {
    pub vb: RefCell<[TripleVertexBuffer; 3]>,
    context: RenderContext,
    zindex: i8,
}

impl RenderLayer {
    pub fn new(context: &RenderContext, num_quads: usize, zindex: i8) -> anyhow::Result<Self> {
        let vb = [
            Self::compute_vertices(context, 32)?,
            Self::compute_vertices(context, num_quads)?,
            Self::compute_vertices(context, 32)?,
        ];

        Ok(Self {
            context: context.clone(),
            vb: RefCell::new(vb),
            zindex,
        })
    }

    pub fn clear_quad_allocation(&self) {
        for vb in self.vb.borrow().iter() {
            vb.clear_quad_allocation();
        }
    }

    pub fn quad_allocator(&self) -> TripleLayerQuadAllocator<'_> {
        // We're creating a self-referential struct here to manage the lifetimes
        // of these related items.  The transmutes are safe because we're only
        // transmuting the lifetimes (not the types), and we're keeping hold
        // of the owner in the returned struct.
        unsafe {
            let vbs = self.vb.borrow().extend_lifetime();
            let layer0 = vbs[0].map().extend_lifetime();
            let layer1 = vbs[1].map().extend_lifetime();
            let layer2 = vbs[2].map().extend_lifetime();
            TripleLayerQuadAllocator::Gpu(BorrowedLayers {
                layers: [layer0, layer1, layer2],
                _owner: vbs,
            })
        }
    }

    pub fn need_more_quads(&self, vb_idx: usize) -> Option<usize> {
        self.vb.borrow()[vb_idx].need_more_quads()
    }

    pub fn reallocate_quads(&self, idx: usize, num_quads: usize) -> anyhow::Result<()> {
        let vb = Self::compute_vertices(&self.context, num_quads)?;
        self.vb.borrow_mut()[idx] = vb;
        Ok(())
    }

    /// Compute a vertex buffer to hold the quads that comprise the visible
    /// portion of the screen.   We recreate this when the screen is resized.
    /// The idea is that we want to minimize any heavy lifting and computation
    /// and instead just poke some attributes into the offset that corresponds
    /// to a changed cell when we need to repaint the screen, and then just
    /// let the GPU figure out the rest.
    fn compute_vertices(
        context: &RenderContext,
        num_quads: usize,
    ) -> anyhow::Result<TripleVertexBuffer> {
        log::trace!("compute_vertices num_quads={}", num_quads);
        let mut indices = vec![];
        indices.reserve(num_quads * INDICES_PER_CELL);

        for q in 0..num_quads {
            let idx = (q * VERTICES_PER_CELL) as u32;

            // Emit two triangles to form the glyph quad
            indices.push(idx + V_TOP_LEFT as u32);
            indices.push(idx + V_TOP_RIGHT as u32);
            indices.push(idx + V_BOT_LEFT as u32);

            indices.push(idx + V_TOP_RIGHT as u32);
            indices.push(idx + V_BOT_LEFT as u32);
            indices.push(idx + V_BOT_RIGHT as u32);
        }

        let buffer = TripleVertexBuffer {
            index: RefCell::new(0),
            bufs: RefCell::new([
                context.allocate_vertex_buffer(num_quads),
                context.allocate_vertex_buffer(num_quads),
                context.allocate_vertex_buffer(num_quads),
            ]),
            capacity: num_quads,
            indices: context.allocate_index_buffer(&indices),
            next_quad: RefCell::new(0),
        };

        Ok(buffer)
    }
}

pub struct BorrowedLayers {
    pub layers: [MappedQuads<'static>; 3],

    // layers references _owner, so it must be dropped after layers.
    _owner: Ref<'static, [TripleVertexBuffer; 3]>,
}

impl TripleLayerQuadAllocatorTrait for BorrowedLayers {
    fn allocate(&mut self, layer_num: usize) -> anyhow::Result<QuadImpl<'_>> {
        self.layers[layer_num].allocate()
    }

    fn extend_with(&mut self, layer_num: usize, vertices: &[Vertex]) {
        self.layers[layer_num].extend_with(vertices)
    }

    fn extend_with_quads(&mut self, layer_num: usize, quads: &[BoxedQuad]) {
        self.layers[layer_num].extend_with_quads(quads)
    }
}

pub struct RenderState {
    pub context: RenderContext,
    pub glyph_cache: RefCell<GlyphCache>,
    pub util_sprites: UtilSprites,
    pub layers: RefCell<Vec<Rc<RenderLayer>>>,
}

impl RenderState {
    pub fn new(
        context: RenderContext,
        fonts: &Rc<FontConfiguration>,
        metrics: &RenderMetrics,
        mut atlas_size: usize,
    ) -> anyhow::Result<Self> {
        loop {
            let glyph_cache = RefCell::new(GlyphCache::new_gl(&context, fonts, atlas_size)?);
            let result = UtilSprites::new(&mut *glyph_cache.borrow_mut(), metrics);
            match result {
                Ok(util_sprites) => {
                    let main_layer = Rc::new(RenderLayer::new(&context, 1024, 0)?);

                    return Ok(Self {
                        context,
                        glyph_cache,
                        util_sprites,
                        layers: RefCell::new(vec![main_layer]),
                    });
                }
                Err(OutOfTextureSpace {
                    size: Some(size), ..
                }) => {
                    atlas_size = size;
                }
                Err(OutOfTextureSpace { size: None, .. }) => {
                    anyhow::bail!("requested texture size is impossible!?")
                }
            };
        }
    }

    pub fn layer_for_zindex(&self, zindex: i8) -> anyhow::Result<Rc<RenderLayer>> {
        if let Some(layer) = self
            .layers
            .borrow()
            .iter()
            .find(|l| l.zindex == zindex)
            .map(Rc::clone)
        {
            return Ok(layer);
        }

        let layer = Rc::new(RenderLayer::new(&self.context, 128, zindex)?);
        let mut layers = self.layers.borrow_mut();
        layers.push(Rc::clone(&layer));

        // Keep the layers sorted by zindex so that they are rendered in
        // the correct order when the layers array is iterated.
        layers.sort_by(|a, b| a.zindex.cmp(&b.zindex));

        Ok(layer)
    }

    /// Returns true if any of the layers needed more quads to be allocated,
    /// and if we successfully allocated them.
    /// Returns false if the quads were sufficient.
    /// Returns Err if we needed to allocate but failed.
    pub fn allocated_more_quads(&mut self) -> anyhow::Result<bool> {
        let mut allocated = false;

        for layer in self.layers.borrow().iter() {
            for vb_idx in 0..3 {
                if let Some(need_quads) = layer.need_more_quads(vb_idx) {
                    // Round up to next multiple of 128 that is >=
                    // the number of needed quads for this frame
                    let num_quads = (need_quads + 127) & !127;
                    layer.reallocate_quads(vb_idx, num_quads).with_context(|| {
                        format!(
                            "Failed to allocate {} quads (needed {})",
                            num_quads, need_quads,
                        )
                    })?;
                    log::trace!("Allocated {} quads (needed {})", num_quads, need_quads);
                    allocated = true;
                }
            }
        }

        Ok(allocated)
    }

    pub fn config_changed(&mut self) {
        self.glyph_cache.borrow_mut().config_changed();
    }

    pub fn recreate_texture_atlas(
        &mut self,
        fonts: &Rc<FontConfiguration>,
        metrics: &RenderMetrics,
        size: Option<usize>,
    ) -> anyhow::Result<()> {
        // We make a a couple of passes at resizing; if the user has selected a large
        // font size (or a large scaling factor) then the `size==None` case will not
        // be able to fit the initial utility glyphs and apply_scale_change won't
        // be able to deal with that error situation.  Rather than make every
        // caller know how to deal with OutOfTextureSpace we try to absorb
        // and accomodate that here.
        let mut size = size;
        let mut attempt = 10;
        loop {
            match self.recreate_texture_atlas_impl(fonts, metrics, size) {
                Ok(_) => return Ok(()),
                Err(err) => {
                    attempt -= 1;
                    if attempt == 0 {
                        return Err(err);
                    }

                    if let Some(&OutOfTextureSpace {
                        size: Some(needed_size),
                        ..
                    }) = err.downcast_ref::<OutOfTextureSpace>()
                    {
                        size.replace(needed_size);
                        continue;
                    }

                    return Err(err);
                }
            }
        }
    }

    fn recreate_texture_atlas_impl(
        &mut self,
        fonts: &Rc<FontConfiguration>,
        metrics: &RenderMetrics,
        size: Option<usize>,
    ) -> anyhow::Result<()> {
        let size = size.unwrap_or_else(|| self.glyph_cache.borrow().atlas.size());
        let mut new_glyph_cache = GlyphCache::new_gl(&self.context, fonts, size)?;
        self.util_sprites = UtilSprites::new(&mut new_glyph_cache, metrics)?;

        let mut glyph_cache = self.glyph_cache.borrow_mut();

        // Steal the decoded image cache; without this, any animating gifs
        // would reset back to frame 0 each time we filled the texture
        std::mem::swap(
            &mut glyph_cache.image_cache,
            &mut new_glyph_cache.image_cache,
        );

        *glyph_cache = new_glyph_cache;
        Ok(())
    }
}
