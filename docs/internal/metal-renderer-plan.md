# Metal Render Path Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render WezTerm on macOS through Metal into a CAMetalLayer that WezTerm owns, so every live-resize step presents a frame of exactly the new size in the same Core Animation transaction as the window change, then remove CGL/OpenGL/glium.

**Architecture:** Keep wgpu (Metal backend) as the GPU API and keep all existing quad/atlas/shader code. Stop using `wgpu::Surface` on macOS. A new `MetalPresenter` owns the view's CAMetalLayer, sets its size, scale and gravity, gets drawables itself, hands each drawable texture to wgpu through the hal interop API, and presents either async (normal frames) or synchronously inside the CA transaction (resize frames). Then OpenGL goes away.

**Tech Stack:** Rust, wgpu 25.0.2 + wgpu-hal 25.0.2 (Metal), `metal` 0.31 (already in Cargo.lock through wgpu-hal), `objc`/`cocoa` (already used by `window`), AppKit / QuartzCore.

**Spec:** this document. Part 1 is the design, Part 2 is the task list. Kata issue `021q` (parent `khdh`). Resize test matrix: `/private/tmp/claude-501/-Users-alistair-Code-wezterm/386077d6-a8c0-4cd9-9d64-43d823c8892e/scratchpad/repro/matrix.md` (session scratchpad; new rows are in section 5 below so they survive).

---

# Part 1: Design

## 1.1 Why

During a live window drag the content stretches and lags. Measured this session:

- Default renderer on macOS is CGL. `prefer_egl` defaults true, no libEGL ships, EGL fails, and it falls back to an `NSOpenGLContext` on a layer-backed view (`window/src/os/macos/window.rs`, `cglbits`). The view's backing layer is a CAMetalLayer from `make_backing_layer`, but in CGL mode nothing draws into it.
- AppKit calls `drawRect:` / `displayLayer:` once per size step (`NSViewLayerContentsRedrawDuringViewResize`). WezTerm paints only there, capped by `max_fps` (`paint_throttled`). A skipped step shows the previous frame scaled to the new bounds. Skipping the cap while live resizing cut squeezed frames from ~50 to ~0-3 per 60-step drag on a 60 Hz external display. ProMotion (120 Hz built-in) is untested.
- Even when we do paint, the GL `flushBuffer` / wgpu `presentDrawable` is not tied to the CA transaction that carries the window's new frame, so the compositor can show new bounds with old pixels (stretched by the layer's default `contentsGravity = resize`).

The call chain is already synchronous, which is what makes the fix possible: `drawRect:` → `WindowEvent::NeedRepaint` → `TermWindow::dispatch_window_event` (called directly by `WindowInner.events.dispatch`, `window/src/lib.rs:230`) → `do_paint_webgpu` → `WebGpuState::resize` (surface `configure`) → `paint_impl` → `call_draw_webgpu` → `output.present()` (`wezterm-gui/src/termwindow/render/draw.rs:147`). All on the main thread, inside AppKit's display pass for that step.

## 1.2 Goals, non-goals, anti-goals

Goals
- Every live-resize step that we paint is presented at exactly the new pixel size, in the same CA transaction as the window frame change. Stretch share during drag: 0 on both 60 Hz and 120 Hz displays.
- A step we cannot paint in time shows the previous frame unscaled, pinned top-left (a thin unpainted strip), never stretched.
- One render path on macOS (Metal). CGL, EGL/ANGLE and glium removed.
- No main-thread blocking on a busy mux: painting uses whatever lines the client has (this is already true: `wezterm-client/src/pane/renderable.rs get_lines` returns stale or blank lines, it never waits).

Non-goals
- Fixing mux re-wrap lag (separate work, see learning `l-3413a7ad`). This plan makes the *frame* match the window; whether the *text* in it is re-wrapped yet is the mux's job.
- Rewriting the quad/atlas pipeline or shaders.
- Software rendering. Metal has no CPU fallback; `front_end = "Software"` maps to Metal with a warning.

Anti-goals (failure even if it technically works)
- Drags that feel heavier because each step now waits on the GPU. Per-step cost must stay under one refresh (8.3 ms at 120 Hz) for an idle local pane in a release build.
- Higher idle CPU/GPU use than today when nothing changes.
- Breaking `window_background_opacity` / `macos_window_background_blur` transparency.

## 1.3 Approach comparison (question 1)

| | (a) wgpu, take control of the layer | (b) native Metal backend (`objc2-metal` or `metal`) | (c) keep CGL, fix sync |
|---|---|---|---|
| What changes | New `MetalPresenter` (~400-600 lines). wgpu renders into drawables we acquire. `wgpu::Surface` unused on macOS. | New buffer/texture/pipeline types behind `RenderContext`, WGSL → MSL (naga at build time or hand-port 124 lines), new encoder in `draw.rs`, plus the same presenter. ~2-3k lines. | `NSOpenGLContext` on a layer has no transaction-synced present. `CAOpenGLLayer` is deprecated with OpenGL. |
| Effort | 3-5 days for presenter + verification | 2-3 weeks | Not viable long term |
| Risk | Low. The one risky API point (wgpu rendering into a layer-owned drawable, presented inside a CATransaction) is proven by the spike (1.5). wgpu hal APIs used are public but `unsafe`; wgpu upgrades may move them. | Medium: second GPU abstraction to keep correct, new shader dialect, new validation story. | Deprecated API (Apple: OpenGL deprecated since macOS 10.14). |
| Perf | wgpu adds validation and per-frame bind-group creation. For a terminal (a few thousand quads, one atlas) this is well under a millisecond (inference; Phase 1 measures it with signposts). | Slightly lower CPU per frame; enables Metal-only features directly. | — |
| Code kept | All of `render/`, `renderstate.rs` WebGpu side, `shader.wgsl`, glyph atlas, `webgpu.rs` device/pipeline setup. | Quad building and atlas only. | All, but nothing gained. |

**Recommendation: (a).** It fixes the resize bug with the least new code, keeps the shaders and pipeline that already work, and the presenter it builds is exactly the piece (b) would need anyway. If later profiling shows wgpu overhead matters, (b) becomes a swap of the encoder behind the same presenter (Phase 6, optional). "Built on Metal and CAMetalLayer" holds: wgpu's only backend on this fork becomes Metal, and WezTerm owns the layer.

Why not just set `presentsWithTransaction` on wgpu's surface: wgpu-hal 25 sets the layer's `presentsWithTransaction` from a plain `bool` field on `hal::metal::Surface` every `configure` (`wgpu-hal-25.0.2/src/metal/surface.rs:162`), and `Surface::as_hal` only hands out `&Surface`, so the field can't be changed after creation. `WebGpuState::resize` calls `configure` on every size step, so anything we set on the layer is overwritten. Owning the drawable avoids this entirely.

## 1.4 Presenting each resize step in the same transaction (question 2)

### Layer setup (once, in `make_backing_layer` and the presenter's constructor)

- The view's backing layer is the CAMetalLayer (`make_backing_layer`, already true). The presenter takes it from `[view layer]`; wgpu never sees it.
- `device` = wgpu's `MTLDevice` (from `Device::as_hal::<Metal>` → `raw_device()`).
- `pixelFormat = MTLPixelFormatBGRA8Unorm_sRGB` so the existing sRGB pipeline format (`Bgra8UnormSrgb`, chosen in `webgpu.rs` today from surface caps) is unchanged.
- `framebufferOnly = YES`, `opaque = NO` (transparency), `maximumDrawableCount = 3`, `allowsNextDrawableTimeout = YES` (so `nextDrawable` returns nil after ~1 s instead of blocking forever; Apple: CAMetalLayer `allowsNextDrawableTimeout`, https://developer.apple.com/documentation/quartzcore/cametallayer/allowsnextdrawabletimeout).
- `contentsScale = window.backingScaleFactor`, updated on screen/DPI change. Today it is forced to 1.0 (`make_backing_layer`, `layer_should_inherit_contents_scale_from_window`) and the drawable is backing-pixel sized; that only looks right because the default `contentsGravity` (`kCAGravityResize`) scales the drawable to the bounds. With a non-scaling gravity a scale of 1.0 would draw a 2x drawable at twice its size. *Inference* from CALayer `contentsScale`/`contentsGravity` semantics; Task 2 verifies on screen. The "stupid hack" in `WindowInner::show` (style mask toggle to fix a stuck 2x scale) should become unnecessary; remove it only after the DPI matrix row passes.
- `contentsGravity = kCAGravityTopLeft` and the view's `layerContentsPlacement = TopLeft` (already set). A step we skip then shows the old frame at 1:1, anchored top-left, instead of stretched. CALayer gravity uses the layer's own geometry, and AppKit backing layers are flipped to match `isFlipped` (WezTerm's view returns YES); whether TopLeft or BottomLeft is the visual top must be checked on screen (Task 2, step "gravity check"). *Inference*, flagged.
- `colorspace`: leave nil in Phase 1 (no behavior change). See 1.6.

### Per paint (`MetalPresenter::begin_frame` / `present`)

1. Compute `drawable_size = round(bounds.size * contentsScale)` from the view's current bounds (not from the cached `Dimensions`; AppKit has already set the new frame by the time `drawRect:` runs). If it differs from `layer.drawableSize`, set it. Apple: `drawableSize` (https://developer.apple.com/documentation/quartzcore/cametallayer/drawablesize). Setting it explicitly every change avoids relying on any implicit update.
2. Choose present mode:
   - `Sync` when `live_resizing` is true, or when the drawable size changed in this frame (covers zoom, full screen, font-scale, DPI changes, which all go through the same path).
   - `Async` otherwise.
   Set `layer.presentsWithTransaction` to match before `nextDrawable` (it is a plain layer property; toggling between frames is allowed, *inference*, the spike toggles nothing; Task 1 test covers both).
3. `nextDrawable`. If nil (timeout or all drawables in flight), skip this frame: leave `invalidated = true` so the next display pass retries. With TopLeft gravity the user sees the old frame unscaled.
4. Wrap the drawable texture: `wgpu::hal::metal::Device::texture_from_raw(...)` then `wgpu::Device::create_texture_from_hal::<Metal>(...)` with `RENDER_ATTACHMENT` usage and the drawable's real width/height. Render exactly as `call_draw_webgpu` does today, into this texture's view.
5. `queue.submit(...)`.
6. Present:
   - `Async`: commit an empty command buffer on wgpu's raw `MTLCommandQueue` that calls `presentDrawable:` (same as wgpu-hal does internally, `wgpu-hal-25.0.2/src/metal/mod.rs:468-490`).
   - `Sync`: commit an empty command buffer on the same queue, `waitUntilScheduled`, then `[drawable present]`. Apple's `presentsWithTransaction` docs describe exactly this order: commit, `waitUntilScheduled`, then present the drawable directly rather than with `presentDrawable:` (https://developer.apple.com/documentation/quartzcore/cametallayer/presentswithtransaction). Command buffers on one queue are scheduled in order, so waiting on the empty buffer means wgpu's buffers are scheduled too (Apple: MTLCommandQueue executes command buffers in the order they are enqueued, https://developer.apple.com/documentation/metal/mtlcommandqueue). Because we are inside `drawRect:` during AppKit's display pass, the present joins the implicit transaction that also carries the new window frame.

### `displaySyncEnabled`

Keep `YES` (vsync'd presents, wgpu's Fifo today; https://developer.apple.com/documentation/quartzcore/cametallayer/displaysyncenabled). With `presentsWithTransaction` the transaction commit decides when the frame shows, so this flag doesn't change resize behavior (*inference*). Phase 4 revisits it for latency.

### Throttle

Bypass `paint_throttled` while `live_resizing` (already measured as the biggest single win; agent `fix3-macos-stretch` may land this first in `draw_rect`; Task 3 takes theirs if merged, else adds it). Keep the cap for normal frames until Phase 4 replaces it with a display link.

### ProMotion / 120 Hz

- During live resize AppKit drives one display pass per resize step, which follows mouse/trackpad events (*inference*: on ProMotion trackpads deliver ~120 events/s). Budget per step is 8.3 ms: Pane resize (local rewrap) + paint + `waitUntilScheduled`. The spike measured ~0.5 ms render+wait and up to ~6.5 ms in `nextDrawable` headless (the layer isn't composited there, so drawables come back slowly). On-screen numbers are Task 2's job.
- Outside resize, Phase 4 replaces the `max_fps` timer with `NSView.displayLink(target:selector:)` (macOS 14+, https://developer.apple.com/documentation/appkit/nsview/displaylink(target:selector:)). It follows whatever display the window is on, so 120 Hz on the built-in screen and 60 Hz on an external one, with `preferredFrameRateRange` capped by `max_fps`. The link is paused when nothing is invalidated, so idle cost stays at zero. Fallback for older macOS: `CVDisplayLink` (needs a hop to the main thread) or keep the timer. Owner question Q1.

### Not blocking the main thread on a busy mux

- Painting never waits on the mux (see 1.2). The two waits we add are bounded: `waitUntilScheduled` (GPU scheduling, sub-millisecond for our workload) and `nextDrawable` (bounded by `allowsNextDrawableTimeout`, and with 3 drawables it only blocks if the GPU is 2 frames behind).
- Guard: if a Sync frame's total time exceeds 12 ms, log a trace line with the breakdown (resize, paint, acquire, wait) so the matrix can catch regressions. No adaptive logic in Phase 1 (YAGNI).
- Local panes re-wrap synchronously in `apply_dimensions`. That cost is the same today; this plan doesn't move it.

## 1.5 Spike result (riskiest API point)

Source: `docs/internal/metal-spike/` (standalone crate, not a workspace member). Headless: a detached CAMetalLayer, no window, no mouse.

Per simulated step (40 steps, sizes 800x600 → 1073x612): `CATransaction begin` → set `drawableSize` → `nextDrawable` → `texture_from_raw` + `create_texture_from_hal` → wgpu render pass (clear) + copy one row back → `queue.submit` → empty command buffer on wgpu's raw queue + `waitUntilScheduled` → `drawable.present()` → `CATransaction commit`.

Result on Apple M5 Pro, debug build:
- 40/40 steps: drawable texture was exactly the requested size, and the pixels read back were the colour wgpu cleared to. wgpu renders into a drawable it didn't create, using public (unsafe) wgpu 25 APIs only.
- `presentsWithTransaction` stayed `true` all the way through, since no `wgpu::Surface` exists to reset it.
- Worst step 8.1 ms, mostly `nextDrawable` (~6.5 ms after the first two steps). Headless the compositor never consumes drawables, so this number is not representative; it is the first thing Task 2 measures on screen.

Not proven by the spike (needs eyes on screen, Task 2): that the present lands in the window-resize transaction, the gravity/flip direction, and on-screen acquire latency.

## 1.6 Removing CGL / OpenGL / glium (question 3)

What depends on them today:

- `window` crate: `GlContextPair`, `BackendImpl::{Cgl,Egl}`, `mod cglbits`, `enable_opengl`, `finish_frame`, `gl_context_pair` in `WindowInner`, `BackendImpl::update` called from `did_resize` (`window/src/os/macos/window.rs`); `window/src/egl.rs` (724 lines); `gl_connection` in `window/src/os/macos/connection.rs`; `WindowOps::enable_opengl` / `finish_frame` and `pub use glium` (`window/src/lib.rs:38, 252-255`); `impl Texture2d for SrgbTexture2d` (`window/src/bitmaps/mod.rs:4-80`); `window/src/configuration.rs:11` (Software front end); `window/examples/async.rs`; `glium` in `window/Cargo.toml` and workspace `Cargo.toml:111`. Other OS backends (x11, wayland, windows/wgl) also use glium, but the parallel platform-removal work deletes those.
- `wezterm-gui`: `RenderContext::Glium`, `RenderFrame::Glium`, glium arms of `IndexBuffer`/`VertexBuffer`/`MappedVertexBuffer`, `glyph_prog`, `compile_prog` (`renderstate.rs`); `call_draw_glium` (`render/draw.rs`); `uniforms.rs` (glium `Uniforms` glue); `implement_vertex!` (`quad.rs:44`); `gl` field, `do_paint`, `FrontEndSelection` match (`termwindow/mod.rs`); `glyph-vertex.glsl`, `glyph-frag.glsl`.
- `config`: `FrontEndSelection::{OpenGL, Software}` (default `OpenGL`, `config/src/frontend.rs`), `prefer_egl` (`config/src/config.rs:541`, default fn at 1894). Docs: `docs/config/lua/config/front_end.md`, `prefer_egl.md`.

Order (each step builds and passes `make check` / `make test`):

1. Phase 1-2 land: Metal presenter works, WebGpu is the default, `OpenGL`/`Software` map to `WebGpu` with a one-time warning. Nothing deleted yet, so the old path is a config switch away while the matrix runs. **Wait for the platform-removal branch to land before step 2**, so we don't edit x11/wayland/windows code that is about to be deleted.
2. `wezterm-gui`: delete all glium arms and the GLSL shaders. `RenderContext`, `RenderFrame`, buffer enums collapse to one variant; keep the enums for now if collapsing them churns too many call sites (implementer's call), else replace with the WebGpu types directly.
3. `window`: delete `GlContextPair`, `cglbits`, `egl.rs`, `gl_connection`, `enable_opengl`, `finish_frame`, `SrgbTexture2d` impl, `pub use glium`, `examples/async.rs` GL use.
4. Cargo: drop `glium` from `window/Cargo.toml` and workspace. Set wgpu `default-features = false, features = ["metal", "wgsl"]` and `Backends::METAL` in `webgpu.rs` (no GL/Vulkan backends compiled). No version changes, so `ci/update-cargo-deps-with-cooldown.sh` isn't needed; `cargo check` updates the lock.
5. Config/docs: keep `front_end` and `prefer_egl` parseable (old configs must load), document them as ignored on this fork, update changelog.

## 1.7 Other macOS-only wins (question 4)

| Win | What | Expected value |
|---|---|---|
| Display-link frame pacing | `NSView.displayLink` replaces the `max_fps` timer (1.4). Smooth 120 Hz scrolling on ProMotion, correct rate per display, zero work when idle. | High on ProMotion. Phase 4. |
| Correct colour | Set `layer.colorspace = sRGB`. Today it is nil, so CA does no colour matching and sRGB palette colours are shown in the display's space (oversaturated on P3 screens). *Inference* from CAMetalLayer `colorspace` docs (https://developer.apple.com/documentation/quartzcore/cametallayer/colorspace); check with Digital Color Meter. Option to pick Display P3 for people who like the vivid look. | Medium, small change. Phase 5. Owner Q3. |
| EDR / HDR | `Rgba16Float` + `wantsExtendedDynamicRangeContent` (wgpu-hal already wires EDR to that format, `surface.rs:163-167`). Lets images (kitty/iTerm2 protocol) show HDR. Terminal text gains nothing. | Low. Not planned. |
| Lower input latency | Paint right after the display link fires, `maximumDrawableCount = 2`, consider `displaySyncEnabled = NO` for windowed mode. Saves up to one frame. | Medium, needs measurement. Phase 4 follow-up. |
| Smaller, faster builds | Drop glium, EGL, wgpu GL/Vulkan/DX12 backends. | Medium for development. Phase 3. |
| Native Metal encoder | Replace wgpu encoding behind the presenter. | Low until profiling says otherwise. Phase 6, optional. |

## 1.8 Calls I made (owner can override)

- wgpu stays (approach a). Native Metal only if profiling demands it.
- Sync present also for non-live size changes (zoom, full screen, font scale, DPI), since they share the path and the same stretch can show there.
- Skipped resize frames show the old frame top-left rather than stretched.
- `front_end = "OpenGL"`/`"Software"` keep parsing and map to WebGpu with a warning.
- Plan lives at `docs/internal/` per the brief. MkDocs may pick up `docs/internal/*.md` as pages; if `make docs` complains, exclude the folder in `docs/mkdocs-base.yml`.

## 1.9 Open questions for the owner

- **Q1 Minimum macOS.** Deployment target is 10.12 today (`ci/generate-workflows.py:831`). `NSView.displayLink` needs 14. Raise the floor to 14 (simplest), or keep a `CVDisplayLink` fallback?
- **Q2 Default switch timing.** Flip the default to WebGpu/Metal right after Phase 1 passes the matrix, or keep OpenGL default until Phase 3 removes it?
- **Q3 Colour.** Should sRGB palette colours look exactly sRGB (colour-managed), or keep today's look on P3 displays?
- **Q4 Native Metal.** Is a wgpu-free renderer a goal in itself (build time, dependency count), or only if it measurably helps?

---

# Part 2: Tasks

## Global Constraints

- macOS only. Code goes in `window/src/os/macos/` and `wezterm-gui/src/termwindow/`; anything macOS-specific in `wezterm-gui` is under `#[cfg(target_os = "macos")]` until platform removal lands.
- wgpu `=25.0.2`, wgpu-hal `=25.0.2`, `metal` `0.31.0`: no version bumps in this work. If a crate is needed that isn't already in Cargo.lock, use `ci/update-cargo-deps-with-cooldown.sh`, never plain `cargo update`.
- Formatting: `cargo +nightly fmt`. Commit subjects prefixed by area (`wezterm-gui: ...`, `window: ...`, `config: ...`, `docs: ...`).
- American English in code. Comments on tests explain intent.
- Each worktree builds with its own target dir; check a patch-specific string with `strings target/debug/wezterm-gui | grep <marker>` before measuring.
- UI changes are not done until they have been seen on screen: the parent session runs the matrix rows in section 5 with motionrec and posts captioned screenshots. Agents don't launch GUI windows or drive the mouse themselves.
- Skills for implementers and reviewers: `apple-gpu-performance` (Metal), `apple-profiling` (signposts/xctrace for Task 2/4 timing), `transition-test-matrix` (resize verification), `superpowers:test-driven-development`. Each report lists the skills used.

## Review Focus

1. **Drag across displays with different scale (2x ↔ 1x).** Expect `contentsScale` and `drawableSize` to update on the first frame on the new screen, glyphs at scale 1.0, no 2x blow-up. Test: Task 1 `drawable_size_tracks_scale_change`; matrix row M5.
2. **`nextDrawable` returns nil mid-drag.** Expect the frame skipped, no panic, a repaint on the next pass, old content shown unscaled. Test: Task 1 `nil_drawable_skips_frame` (inject via a trait seam).
3. **Zero-size or minimized window (0 px width/height).** Expect no `drawableSize` of 0 set and no paint (wgpu panics on 0-sized textures, see issue #2881 note in `webgpu.rs`). Test: Task 1 `zero_size_is_skipped`.
4. **Transparent window (`window_background_opacity < 1`).** Expect alpha preserved (layer `opaque = NO`, pipeline alpha unchanged). Matrix row M9.
5. **Window closed while a Sync frame is in flight / presenter dropped.** Expect no use-after-free of the layer: presenter holds a retained layer and drops after the device idles. Test: Task 1 `presenter_drop_after_present_is_clean`.

---

### Task 1: `MetalPresenter` (owned layer, drawable acquire, sync/async present)

**Files:**
- Create: `wezterm-gui/src/termwindow/metal_present.rs`
- Modify: `wezterm-gui/src/termwindow/mod.rs` (add `#[cfg(target_os = "macos")] pub mod metal_present;`)
- Test: `wezterm-gui/src/termwindow/metal_present.rs` (`#[cfg(test)] mod test`, headless, ported from `docs/internal/metal-spike/src/main.rs`)

**Interfaces:**
- Consumes: `wgpu::Device`, `wgpu::Queue` from `WebGpuState`; a retained `CAMetalLayer` pointer.
- Produces:
  - `pub enum PresentMode { Async, Sync }`
  - `pub fn choose_present_mode(live_resizing: bool, size_changed: bool) -> PresentMode` (Sync if either is true)
  - `pub fn drawable_size_for(bounds_w_pt: f64, bounds_h_pt: f64, scale: f64) -> Option<(u32, u32)>` (rounded; `None` if either side rounds to 0)
  - `pub struct MetalPresenter` with
    - `pub unsafe fn new(layer: *mut objc::runtime::Object, device: &wgpu::Device, format: wgpu::TextureFormat) -> anyhow::Result<Self>` (configures the layer as in 1.4; `format` must be `Bgra8UnormSrgb` or `Bgra8Unorm`)
    - `pub fn set_scale(&self, scale: f64)` (sets `contentsScale`)
    - `pub fn begin_frame(&self, device: &wgpu::Device, size: (u32, u32), mode: PresentMode) -> Option<PresentFrame>` (sets `drawableSize` if changed and `presentsWithTransaction` per mode; `None` when `nextDrawable` is nil)
    - `pub struct PresentFrame { pub texture: wgpu::Texture, pub size: (u32, u32), /* drawable, mode */ }`
    - `pub fn present(&self, queue: &wgpu::Queue, frame: PresentFrame)` (must be called after `queue.submit` of the frame's work; Sync = empty cmd buffer + `waitUntilScheduled` + `[drawable present]`; Async = empty cmd buffer with `presentDrawable:` + commit)
  - A private `trait DrawableSource` seam so tests can force a nil drawable.

- [ ] **Step 1: Write the failing tests**

```rust
// Pure policy: any resize forces a transaction-synced present.
#[test]
fn present_mode_policy() {
    assert_eq!(choose_present_mode(false, false), PresentMode::Async);
    assert_eq!(choose_present_mode(true, false), PresentMode::Sync);
    assert_eq!(choose_present_mode(false, true), PresentMode::Sync);
}

// Drawable size follows bounds * scale, and never becomes 0.
#[test]
fn zero_size_is_skipped() {
    assert_eq!(drawable_size_for(400.0, 300.0, 2.0), Some((800, 600)));
    assert_eq!(drawable_size_for(400.25, 300.0, 2.0), Some((801, 600)));
    assert_eq!(drawable_size_for(0.0, 300.0, 2.0), None);
    assert_eq!(drawable_size_for(0.2, 300.0, 1.0), None);
}

// Headless: a detached CAMetalLayer hands out drawables of exactly the size
// asked for on every step, and wgpu's pixels land in them (spike, as a test).
#[test]
fn owned_layer_presents_exact_size() { /* 10 steps, both modes alternating;
    assert frame.size == requested and read-back pixel == clear colour */ }

// Moving to a 1x screen must shrink the drawable on the next frame.
#[test]
fn drawable_size_tracks_scale_change() { /* set_scale(2.0) then (1.0);
    assert begin_frame sizes are 800x600 then 400x300 for 400x300 pt */ }

// A nil drawable is a skipped frame, not a panic.
#[test]
fn nil_drawable_skips_frame() { /* DrawableSource stub returns None;
    assert begin_frame(..).is_none() */ }

// Dropping the presenter right after a Sync present is clean.
#[test]
fn presenter_drop_after_present_is_clean() { /* present Sync, drop presenter,
    device.poll(Wait) returns Ok */ }
```

Headless GPU tests need a Metal device; mark them `#[cfg(target_os = "macos")]` and skip (return early with a log line) if `request_adapter` fails, so CI without a GPU stays green.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p wezterm-gui metal_present`
Expected: FAIL, unresolved names.

- [ ] **Step 3: Implement `metal_present.rs`** per the Interfaces block and 1.4. Use `wgpu::hal::metal::Device::texture_from_raw` + `Device::create_texture_from_hal::<wgpu::hal::api::Metal>`; get the raw queue with `Queue::as_hal::<Metal>(|q| q.unwrap().as_raw().lock())`. Retain the layer (`StrongPtr::retain`). Add a trace log `metal_present: step size={w}x{h} mode={mode:?} acquire_ms paint_ms wait_ms` (marker string for `strings` checks: `metal_present: step`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p wezterm-gui metal_present`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add wezterm-gui/src/termwindow/metal_present.rs wezterm-gui/src/termwindow/mod.rs
git commit -m "wezterm-gui: add MetalPresenter for a wezterm-owned CAMetalLayer"
```

### Task 2: Wire the presenter into the WebGpu path and fix layer scale/gravity

**Files:**
- Modify: `wezterm-gui/src/termwindow/webgpu.rs` (on macOS: no `wgpu::Surface`; pick the adapter with `instance.request_adapter` without `compatible_surface`, keep `Backends` as is until Task 5; format `Bgra8UnormSrgb`; hold `presenter: MetalPresenter`; `resize` only records dimensions, no `configure`)
- Modify: `wezterm-gui/src/termwindow/render/draw.rs:20-150` (`call_draw_webgpu`: get the target from `presenter.begin_frame`, return `Ok(())` early on `None` and mark the window invalidated; `present` after `queue.submit`)
- Modify: `wezterm-gui/src/termwindow/mod.rs:1091-1110` (`do_paint_webgpu`: compute size via `drawable_size_for` from the view bounds and scale; mode via `choose_present_mode(live_resizing, size != last_size)`)
- Modify: `window/src/os/macos/window.rs` (`make_backing_layer`: `contentsScale = backingScaleFactor`, `contentsGravity = kCAGravityTopLeft`; `layer_should_inherit_contents_scale_from_window`: return the window's scale instead of forcing 1.0; expose `Window::backing_scale()` and `Window::is_live_resizing()` and `Window::layer_ptr()` on the macOS `Window` via `WindowOps` or an inherent `#[cfg(target_os="macos")]` method)
- Test: existing `cargo nextest run -p wezterm-gui`; on-screen rows M1-M9 (parent session).

**Interfaces:**
- Consumes: Task 1 `MetalPresenter`, `PresentMode`, `choose_present_mode`, `drawable_size_for`.
- Produces: `front_end = "WebGpu"` on macOS renders through `MetalPresenter`. `Window::backing_scale(&self) -> f64`, `Window::is_live_resizing(&self) -> bool`, `Window::layer_ptr(&self) -> *mut Object` (macOS only).

- [ ] **Step 1:** Implement the changes above. Keep the non-macOS `wgpu::Surface` code under `#[cfg(not(target_os = "macos"))]` until platform removal lands.
- [ ] **Step 2:** `make check` and `cargo nextest run -p wezterm-gui` pass.
- [ ] **Step 3: Gravity check (parent session, on screen).** Build `cargo build --bin wezterm-gui`, check `strings target/debug/wezterm-gui | grep 'metal_present: step'`. Run with a test config `front_end = "WebGpu"` and `max_fps = 10` (makes skipped steps obvious). Drag the right edge out slowly. Expected: during skipped steps the old frame stays at its top-left at 1:1, a strip appears on the right, text never stretches. If the content sticks to the bottom instead, switch gravity to `kCAGravityBottomLeft` and record which one in a comment.
- [ ] **Step 4: On-screen timing.** With `WEZTERM_LOG='info,wezterm_gui::termwindow::metal_present=trace'` run S4 (local, slow shrink) on both displays; record p50/p95 of `acquire_ms`, `wait_ms`, total per step in the matrix results. Pass: p95 total < 8.3 ms on the built-in display, release build.
- [ ] **Step 5:** Run matrix rows M1-M9 (section 5). Post captioned screenshots/frame grabs.
- [ ] **Step 6: Commit**

```bash
git commit -am "wezterm-gui: present WebGpu frames through MetalPresenter on macOS"
```

### Task 3: Paint every live-resize step

**Files:**
- Modify: `window/src/os/macos/window.rs` `draw_rect` (around line 3139): when `inner.live_resizing`, dispatch `NeedRepaint` without starting the `max_fps` throttle timer.

Check first whether `fix3-macos-stretch` already merged this; if so, this task is only verification.

- [ ] **Step 1:** Implement, `make check`.
- [ ] **Step 2:** Matrix rows M1, M2 with `max_fps = 60` and `120`. Expected: stretch share 0, paints per drag == resize steps (count `metal_present: step` lines vs `Resized` trace lines).
- [ ] **Step 3: Commit** `window: paint every live-resize step on macOS`.

### Task 4: Make Metal the default; map old front ends

**Files:**
- Modify: `config/src/frontend.rs` (`#[default]` → `WebGpu`)
- Modify: `wezterm-gui/src/termwindow/mod.rs:852-870` (on macOS treat `OpenGL` and `Software` as `WebGpu`; `log::warn!` once: "front_end = {x} is not supported on this build; using WebGpu (Metal)")
- Modify: `docs/config/lua/config/front_end.md`, `docs/changelog.md` (Continuous/Nightly)
- Test: `config` unit test `front_end_defaults_to_webgpu`.

- [ ] **Step 1: Failing test**

```rust
// The fork renders through Metal unless told otherwise.
#[test]
fn front_end_defaults_to_webgpu() {
    assert_eq!(FrontEndSelection::default(), FrontEndSelection::WebGpu);
}
```
- [ ] **Step 2:** `cargo nextest run -p config front_end_defaults_to_webgpu` fails, implement, passes.
- [ ] **Step 3:** Smoke set S1-S8 with no `front_end` in config; release build once.
- [ ] **Step 4: Commit** `config: default front_end to WebGpu (Metal) on macOS`.

### Task 5: Remove glium, CGL and EGL

Start only after the platform-removal branch has landed (owner/main confirms).

**Files:** see 1.6 list. Delete `window/src/egl.rs`, `wezterm-gui/src/glyph-vertex.glsl`, `wezterm-gui/src/glyph-frag.glsl`, `wezterm-gui/src/uniforms.rs`; edit `window/src/lib.rs`, `window/src/bitmaps/mod.rs`, `window/src/configuration.rs`, `window/src/os/macos/{window,connection}.rs`, `window/examples/async.rs`, `wezterm-gui/src/{renderstate,quad}.rs`, `wezterm-gui/src/termwindow/{mod.rs,render/draw.rs}`, `window/Cargo.toml`, `Cargo.toml` (drop `glium`; wgpu `default-features = false, features = ["metal", "wgsl"]`), `webgpu.rs` (`Backends::METAL`).

- [ ] **Step 1:** Delete in the order of 1.6 steps 2-4, running `make check` after each.
- [ ] **Step 2:** `grep -rn glium --include='*.rs' --include=Cargo.toml . | grep -v target` returns nothing. `cargo tree -p wezterm-gui | grep -c glium` is 0.
- [ ] **Step 3:** `make test`, `cargo +nightly fmt --all -- --check`.
- [ ] **Step 4:** Docs: `prefer_egl.md` and `front_end.md` say the options are accepted and ignored on this fork; changelog entry.
- [ ] **Step 5:** Smoke set S1-S8 + M1, M2, release build.
- [ ] **Step 6: Commit** in three commits: `wezterm-gui: remove OpenGL render path`, `window: remove CGL and EGL`, `deps: drop glium and non-Metal wgpu backends`.

### Task 6: Display-link frame pacing (needs answer to Q1)

**Files:**
- Modify: `window/src/os/macos/window.rs` (create `[view displayLinkWithTarget:selector:]` on first show; callback `display_link_fired` calls `setNeedsDisplay:` only when `invalidated`, pauses the link (`paused = YES`) when not; `invalidate()` unpauses; set `preferredFrameRateRange` to `(min: 30, max: max_fps, preferred: max_fps)`; remove the `paint_throttled` timer)
- Test: matrix rows M8, M10.

- [ ] **Step 1:** Implement behind a `respondsToSelector:` check; keep the timer path when unavailable (or delete it if Q1 = raise floor to 14).
- [ ] **Step 2:** M8: `yes | head -c 50M` in a pane on the built-in display; Instruments (`xctrace record --template 'Animation Hitches'`) shows ~120 presents/s, no hitches. Idle 10 s: 0 presents (count `metal_present: step` lines).
- [ ] **Step 3: Commit** `window: pace macOS paints with NSView display link`.

### Task 7 (optional, Q3): Colour-managed output

- Modify: `MetalPresenter::new` sets `layer.colorspace = CGColorSpaceCreateWithName(kCGColorSpaceSRGB)`; config option `macos_colorspace = "sRGB" | "DisplayP3" | "None"` (default from Q3) in `config/src/config.rs` with docs page under `docs/config/lua/config/`.
- Verify: Digital Color Meter on a P3 display reads `#ff0000` palette red as sRGB red with `sRGB`, as display red with `None`.
- Commit: `wezterm-gui: colour-manage the Metal layer`.

### Phase 6 (not planned): native Metal encoder

Only if Task 2/6 timing shows wgpu encoding is a meaningful share of a frame. Replace `call_draw_webgpu` encoding with `metal` crate calls behind the same `MetalPresenter`; port `shader.wgsl` with naga (`naga` CLI to MSL) at build time.

### Rough size

| Phase | Tasks | Size |
|---|---|---|
| 0 Spike | done | — |
| 1 Presenter + wiring + throttle | 1, 2, 3 | ~600 lines, 3-5 days incl. matrix |
| 2 Default switch | 4 | ~50 lines, 0.5 day |
| 3 Remove OpenGL | 5 | ~-2,500 lines, 1-2 days |
| 4 Display link | 6 | ~200 lines, 1-2 days |
| 5 Colour | 7 | ~80 lines, 0.5 day |

---

# 5. Verification: matrix rows

These slot into the resize matrix (`repro/matrix.md`). Same evidence and scoring: motionrec at 120 fps, `motion-track` on the probe-coloured column and right edge, `motion-regions` for strips; score during the drag (share of wrong frames, worst frame) and settle-after-release separately. Normalise by a fixed-size element (recordings scale when the window grows). Cross-check with `WEZTERM_LOG='info,wezterm_gui::termwindow::metal_present=trace,wezterm_gui::termwindow::resize=trace'`: count `metal_present: step` lines with `mode=Sync` vs `Resized` events, and compare each step's size to the window size.

Add to the **Settings** axis: `front_end WebGpu + MetalPresenter` (marker string `metal_present: step`).

| # | Row | Checks (pass) |
|---|---|---|
| M1 | local, idle text, shrink width, slow, MetalPresenter, 60 Hz external | stretch share 0; probe glyph scale 1.0 every frame; blank-strip share ≤ 2% of drag frames |
| M2 | M1 on built-in ProMotion 120 Hz | same as M1; p95 per-step total < 8.3 ms (trace, release build) |
| M3 | local, idle text, grow corner, flick, both displays | no stretch, no bottom or right strip after first frame of each step |
| M4 | mux, heavy output in visible pane, normal speed, MetalPresenter | stretch share 0 (wrap may lag: scored by S1/S2, not here); window edge never lags cursor by > 1 frame (compare edge track to drag script timing) |
| M5 | drag window from built-in (2x) to external 1x display, then resize | `contentsScale` follows (trace); glyph size correct on first frame after crossing; no 2x blow-up; then M1 checks |
| M6 | local, `max_fps = 10`, slow grow width | skipped steps show old frame top-left at 1:1 (strip on right), never stretched |
| M7 | green-button zoom, full screen in/out, Cmd +/- font scale; local and mux | frame at new size on the first visible frame (`mode=Sync` line for each); no stretch during the zoom animation |
| M8 | idle 10 s, then `yes` flood, built-in display (after Task 6) | idle: 0 presents; flood: ~120 presents/s, no hitches in Animation Hitches trace |
| M9 | `window_background_opacity = 0.8`, `macos_window_background_blur = 20`, S4 + S5 | transparency and blur intact during and after drag |
| M10 | release build: smoke set S1-S8 + M1, M2 with default config (after Task 4) | all pass |

Hand row: trackpad corner drag with real input on the built-in display, M2 checks; say "hand" in the results.

**Exact steps for the parent session (Task 2 gravity/sync check):**
1. In the implementer's worktree: `cargo build --bin wezterm-gui` (its own target dir), then `strings target/debug/wezterm-gui | grep -c 'metal_present: step'` is ≥ 1.
2. Write a test config with `front_end = "WebGpu"`, `max_fps = 10` and the isolated mux settings from learning `l-3413a7ad` if using a mux row.
3. `target/debug/wezterm-gui --config-file <cfg> start --always-new-process`, unset `WEZTERM_*` first.
4. Start `motion-record mac` at 120 fps on the window, run `repro/drag` slow grow width (M6), then M1/M2.
5. Score with `motion-track` / `motion-regions`; paste numbers and captioned frames into the matrix results.
