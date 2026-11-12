use crate::quad::TripleLayerQuadAllocator;
use crate::termwindow::render::RenderScreenLineParams;
use crate::utilsprites::RenderMetrics;
use config::ConfigHandle;
use mux::renderable::RenderableDimensions;
use wezterm_term::color::ColorAttribute;
use window::color::LinearRgba;

/// Where AppKit puts the traffic lights, in points (measured on macOS 26):
/// they are 14pt wide, 9pt apart and 9pt from the left edge, so the zoom
/// button ends at 69pt, and their center is 15.75pt below the window's top.
pub const TRAFFIC_LIGHTS_END_PT: f32 = 69.;
pub const TRAFFIC_LIGHTS_GAP_PT: f32 = 9.;
const TRAFFIC_LIGHT_CENTER_PT: f32 = 15.75;

impl crate::TermWindow {
    /// Whether the macOS traffic lights are drawn over the tab bar.
    pub fn native_traffic_lights_shown(&self) -> bool {
        self.config
            .window_decorations
            .contains(::window::WindowDecorations::INTEGRATED_BUTTONS)
            && self.config.integrated_title_button_style
                == ::window::IntegratedTitleButtonStyle::MacOsNative
            && !self
                .window_state
                .contains(::window::WindowState::FULL_SCREEN)
    }

    /// How many blank cells come before the first retro tab: room for the
    /// traffic lights plus the same gap they have from the window's edge,
    /// or just that gap without them. None when non-native title buttons
    /// sit at the left, as they start the bar.
    pub fn retro_leading_cells(&self) -> usize {
        let pt = if self.native_traffic_lights_shown() && !self.config.tab_bar_at_bottom {
            TRAFFIC_LIGHTS_END_PT + TRAFFIC_LIGHTS_GAP_PT
        } else if self
            .config
            .window_decorations
            .contains(::window::WindowDecorations::INTEGRATED_BUTTONS)
            && self.config.integrated_title_button_style
                != ::window::IntegratedTitleButtonStyle::MacOsNative
            && self.config.integrated_title_button_alignment
                == ::window::IntegratedTitleButtonAlignment::Left
        {
            return 0;
        } else {
            TRAFFIC_LIGHTS_GAP_PT
        };
        let px = pt * self.dimensions.dpi as f32 / 72.;
        (px / self.render_metrics.cell_size.width as f32).ceil() as usize
    }

    pub fn paint_tab_bar(&mut self, layers: &mut TripleLayerQuadAllocator) -> anyhow::Result<()> {
        // While any tab shows the indeterminate progress spinner, ask to be
        // repainted when its next frame is due. The paint scheduler wakes us
        // when this is the soonest pending animation and rebuilds the tab bar
        // to advance the frame.
        self.update_next_frame_time(self.tab_bar.next_progress_frame_due());

        if self.config.tab_style().is_fancy() {
            if self.fancy_tab_bar.is_none() {
                let palette = self.palette().clone();
                let tab_bar = self.build_fancy_tab_bar(&palette)?;
                self.fancy_tab_bar.replace(tab_bar);
            }

            self.ui_items.append(&mut self.paint_fancy_tab_bar()?);
            return Ok(());
        }

        let border = self.get_os_border();

        let palette = self.palette().clone();
        let tab_bar_height = self.tab_bar_pixel_height()?;
        let tab_bar_y = if self.config.tab_bar_at_bottom {
            ((self.dimensions.pixel_height as f32) - (tab_bar_height + border.bottom.get() as f32))
                .max(0.)
        } else {
            border.top.get() as f32
        };

        // The bar can be taller than a line, to fit the traffic lights; fill
        // it with the bar color and put the line in the middle.
        let line_height = self.render_metrics.cell_size.height as f32;
        if tab_bar_height > line_height {
            let bg = self
                .config
                .resolved_palette
                .tab_bar
                .as_ref()
                .map(|c| c.background())
                .unwrap_or_else(|| config::TabBarColors::default().background());
            self.filled_rectangle(
                layers,
                0,
                euclid::rect(
                    0.,
                    tab_bar_y,
                    self.dimensions.pixel_width as f32,
                    tab_bar_height,
                ),
                bg.to_linear(),
            )?;
        }
        let tab_bar_y = tab_bar_y + ((tab_bar_height - line_height) / 2.).floor();

        // Register the tab bar location
        self.ui_items.append(&mut self.tab_bar.compute_ui_items(
            tab_bar_y as usize,
            self.render_metrics.cell_size.height as usize,
            self.render_metrics.cell_size.width as usize,
        ));

        let window_is_transparent =
            !self.window_background.is_empty() || self.config.window_background_opacity != 1.0;
        let gl_state = self.render_state.as_ref().unwrap();
        let white_space = gl_state.util_sprites.white_space.texture_coords();
        let filled_box = gl_state.util_sprites.filled_box.texture_coords();
        let default_bg = palette
            .resolve_bg(ColorAttribute::Default)
            .to_linear()
            .mul_alpha(if window_is_transparent {
                0.
            } else {
                self.config.text_background_opacity
            });

        self.render_screen_line(
            RenderScreenLineParams {
                top_pixel_y: tab_bar_y,
                left_pixel_x: 0.,
                pixel_width: self.dimensions.pixel_width as f32,
                stable_line_idx: None,
                line: self.tab_bar.line(),
                selection: 0..0,
                cursor: &Default::default(),
                palette: &palette,
                dims: &RenderableDimensions {
                    cols: self.dimensions.pixel_width
                        / self.render_metrics.cell_size.width as usize,
                    physical_top: 0,
                    scrollback_rows: 0,
                    scrollback_top: 0,
                    viewport_rows: 1,
                    dpi: self.terminal_size.dpi,
                    pixel_height: self.render_metrics.cell_size.height as usize,
                    pixel_width: self.terminal_size.pixel_width,
                    reverse_video: false,
                },
                config: &self.config,
                cursor_border_color: LinearRgba::default(),
                foreground: palette.foreground.to_linear(),
                pane: None,
                is_active: true,
                selection_fg: LinearRgba::default(),
                selection_bg: LinearRgba::default(),
                cursor_fg: LinearRgba::default(),
                cursor_bg: LinearRgba::default(),
                cursor_is_default_color: true,
                white_space,
                filled_box,
                window_is_transparent,
                default_bg,
                style: None,
                font: None,
                use_pixel_positioning: self.config.experimental_pixel_positioning,
                render_metrics: self.render_metrics,
                shape_key: None,
                password_input: false,
            },
            layers,
        )?;

        Ok(())
    }

    pub fn tab_bar_pixel_height_impl(
        config: &ConfigHandle,
        fontconfig: &wezterm_font::FontConfiguration,
        render_metrics: &RenderMetrics,
    ) -> anyhow::Result<f32> {
        if config.tab_style().is_fancy() {
            let font = fontconfig.title_font()?;
            let height = font.metrics().cell_height.get() as f32 * 1.75;
            Ok(height
                .max(Self::traffic_light_bar_height(config, fontconfig))
                .ceil())
        } else {
            Ok((render_metrics.cell_size.height as f32)
                .max(Self::traffic_light_bar_height(config, fontconfig))
                .ceil())
        }
    }

    /// On macOS the bar is at least twice as tall as the traffic lights'
    /// center is from the top of the window, so the lights have equal space
    /// above and below them. The same with or without the lights and for
    /// every tab_style, so the bar doesn't change height and tabs
    /// aren't squeezed against its edges.
    fn traffic_light_bar_height(
        _config: &ConfigHandle,
        fontconfig: &wezterm_font::FontConfiguration,
    ) -> f32 {
        if cfg!(target_os = "macos") {
            // macOS reports 72 dpi per point of backing scale.
            2. * TRAFFIC_LIGHT_CENTER_PT * fontconfig.get_dpi() as f32 / 72.
        } else {
            0.
        }
    }

    pub fn tab_bar_pixel_height(&self) -> anyhow::Result<f32> {
        Self::tab_bar_pixel_height_impl(&self.config, &self.fonts, &self.render_metrics)
    }
}
