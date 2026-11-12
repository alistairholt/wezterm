use crate::customglyph::*;
use crate::tabbar::{TabBarItem, TabEntry};
use crate::termwindow::box_model::*;
use crate::termwindow::render::corners::*;
use crate::termwindow::render::tab_bar::{TRAFFIC_LIGHTS_END_PT, TRAFFIC_LIGHTS_GAP_PT};

use crate::termwindow::render::window_buttons::window_button_element;
use crate::termwindow::{UIItem, UIItemType};
use crate::utilsprites::RenderMetrics;
use config::{Dimension, DimensionContext, TabBarColors, TabStyle};
use std::rc::Rc;
use wezterm_font::LoadedFont;
use wezterm_term::color::{ColorAttribute, ColorPalette};
use window::color::LinearRgba;
use window::{IntegratedTitleButtonAlignment, IntegratedTitleButtonStyle};

/// The divider between Flat and Capsule tabs: a filled box, drawn 1px wide.
const DIVIDER: &[Poly] = &[Poly {
    path: &[
        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
        PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
        PolyCommand::Close,
    ],
    intensity: BlockAlpha::Full,
    style: PolyStyle::Fill,
}];

/// The divider's height as a fraction of the tab bar's.
const DIVIDER_HEIGHT: f32 = 0.6;

const X_BUTTON: &[Poly] = &[
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
];

const PLUS_BUTTON: &[Poly] = &[
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
            PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
];

impl crate::TermWindow {
    pub fn invalidate_fancy_tab_bar(&mut self) {
        self.fancy_tab_bar.take();
    }

    pub fn build_fancy_tab_bar(&self, palette: &ColorPalette) -> anyhow::Result<ComputedElement> {
        let tab_bar_height = self.tab_bar_pixel_height()?;
        let font = self.fonts.title_font()?;
        let metrics = RenderMetrics::with_font_metrics(&font.metrics());
        let items = self.tab_bar.items();
        // The resolved palette holds the scheme's tab bar colors with any
        // from `colors` on top.
        let colors = self
            .config
            .resolved_palette
            .tab_bar
            .clone()
            .unwrap_or_else(TabBarColors::default);

        let mut left_status = vec![];
        let mut left_eles = vec![];
        let mut right_eles = vec![];
        let bar_colors = ElementColors {
            border: BorderColor::default(),
            bg: if self.focused.is_some() {
                self.config.window_frame.active_titlebar_bg
            } else {
                self.config.window_frame.inactive_titlebar_bg
            }
            .to_linear()
            .into(),
            text: if self.focused.is_some() {
                self.config.window_frame.active_titlebar_fg
            } else {
                self.config.window_frame.inactive_titlebar_fg
            }
            .to_linear()
            .into(),
        };
        let bar_bg = if self.focused.is_some() {
            self.config.window_frame.active_titlebar_bg
        } else {
            self.config.window_frame.inactive_titlebar_bg
        }
        .to_linear();
        let style = self.config.tab_style();
        let active_idx = items.iter().find_map(|item| match item.item {
            TabBarItem::Tab {
                tab_idx,
                active: true,
            } => Some(tab_idx),
            _ => None,
        });
        let tab_count = items
            .iter()
            .filter(|item| matches!(item.item, TabBarItem::Tab { .. }))
            .count();
        // Flat and Capsule tabs: padding and the pill's corner radius come
        // from window_frame. The radius is a fraction of the tab's height.
        let frame = &self.config.window_frame;
        let pad_x = frame.tab_padding_x;
        let pad_y = frame.tab_padding_y;
        let pad_y_px = pad_y.evaluate_as_pixels(DimensionContext {
            dpi: self.dimensions.dpi as f32,
            pixel_max: self.dimensions.pixel_height as f32,
            pixel_cell: metrics.cell_size.height as f32,
        });
        let tab_height = metrics.cell_size.height as f32 + 2. * pad_y_px;
        // Fancy tabs' top corners: square, so the curve isn't stretched by
        // the cell being taller than it is wide.
        let fancy_radius = Dimension::Pixels((metrics.cell_size.height as f32 * 0.4).round());
        let pill_radius = Dimension::Pixels(tab_height * frame.tab_corner_radius.clamp(0., 0.5));

        let tab_vertical_alignment = if self.config.tab_bar_at_bottom {
            VerticalAlign::Top
        } else {
            VerticalAlign::Bottom
        };

        let item_to_elem = |item: &TabEntry| -> Element {
            let element = Element::with_line(&font, &item.title, palette);

            let bg_color = item
                .title
                .get_cell(0)
                .and_then(|c| match c.attrs().background() {
                    ColorAttribute::Default => None,
                    col => Some(palette.resolve_bg(col)),
                });
            let fg_color = item
                .title
                .get_cell(0)
                .and_then(|c| match c.attrs().foreground() {
                    ColorAttribute::Default => None,
                    col => Some(palette.resolve_fg(col)),
                });

            let new_tab = colors.new_tab();
            let new_tab_hover = colors.new_tab_hover();
            let active_tab = colors.active_tab();
            let is_bottom = self.config.tab_bar_at_bottom;

            match item.item {
                TabBarItem::RightStatus | TabBarItem::LeftStatus | TabBarItem::None => element
                    .item_type(UIItemType::TabBar(TabBarItem::None))
                    .line_height(Some(1.75))
                    .margin(BoxDimension {
                        left: Dimension::Cells(0.),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.0),
                        bottom: Dimension::Cells(0.),
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.),
                        bottom: Dimension::Cells(0.),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(0.)))
                    .colors(bar_colors.clone()),
                TabBarItem::NewTabButton => Element::new(
                    &font,
                    ElementContent::Poly {
                        line_width: metrics.underline_height.max(2),
                        poly: SizedPoly {
                            poly: PLUS_BUTTON,
                            width: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
                            height: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
                        },
                    },
                )
                .vertical_align(VerticalAlign::Middle)
                .item_type(UIItemType::TabBar(item.item.clone()))
                .margin(BoxDimension {
                    left: Dimension::Cells(0.5),
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.2),
                    bottom: Dimension::Cells(0.),
                })
                .padding(BoxDimension {
                    left: Dimension::Cells(0.5),
                    right: Dimension::Cells(0.5),
                    top: Dimension::Cells(0.2),
                    bottom: Dimension::Cells(0.25),
                })
                .border(BoxDimension::new(Dimension::Pixels(1.)))
                .colors(ElementColors {
                    border: BorderColor::default(),
                    bg: new_tab.bg_color.to_linear().into(),
                    text: new_tab.fg_color.to_linear().into(),
                })
                .hover_colors(Some(ElementColors {
                    border: BorderColor::default(),
                    bg: new_tab_hover.bg_color.to_linear().into(),
                    text: new_tab_hover.fg_color.to_linear().into(),
                })),
                TabBarItem::Tab { active, .. } if style != TabStyle::Fancy => {
                    let pill = style == TabStyle::Capsule && active;
                    let (bg, fg) = if active {
                        let bg = if pill {
                            bg_color
                                .unwrap_or_else(|| active_tab.bg_color.into())
                                .to_linear()
                        } else {
                            bg_color.map(|c| c.to_linear()).unwrap_or(bar_bg)
                        };
                        (bg, fg_color.unwrap_or_else(|| active_tab.fg_color.into()))
                    } else {
                        let inactive_tab = colors.inactive_tab();
                        (
                            bg_color.map(|c| c.to_linear()).unwrap_or(bar_bg),
                            fg_color.unwrap_or_else(|| inactive_tab.fg_color.into()),
                        )
                    };
                    let border_color = BorderColor {
                        left: bg,
                        right: bg,
                        top: bg,
                        bottom: bg,
                    };
                    let hover_fg =
                        fg_color.unwrap_or_else(|| colors.inactive_tab_hover().fg_color.into());
                    let rounded = |poly| SizedPoly {
                        width: pill_radius,
                        height: pill_radius,
                        poly,
                    };
                    element
                        .vertical_align(VerticalAlign::Middle)
                        .item_type(UIItemType::TabBar(item.item.clone()))
                        .margin(BoxDimension::new(Dimension::Cells(0.)))
                        .padding(BoxDimension {
                            left: pad_x,
                            right: pad_x,
                            top: pad_y,
                            bottom: pad_y,
                        })
                        .border(BoxDimension::new(Dimension::Pixels(0.)))
                        .border_corners(if pill {
                            Some(Corners {
                                top_left: rounded(TOP_LEFT_SUPERELLIPSE_CORNER),
                                top_right: rounded(TOP_RIGHT_SUPERELLIPSE_CORNER),
                                bottom_left: rounded(BOTTOM_LEFT_SUPERELLIPSE_CORNER),
                                bottom_right: rounded(BOTTOM_RIGHT_SUPERELLIPSE_CORNER),
                            })
                        } else {
                            None
                        })
                        .colors(ElementColors {
                            border: border_color.clone(),
                            bg: bg.into(),
                            text: fg.to_linear().into(),
                        })
                        .hover_colors(if active {
                            None
                        } else {
                            Some(ElementColors {
                                border: border_color,
                                bg: bg.into(),
                                text: hover_fg.to_linear().into(),
                            })
                        })
                }
                TabBarItem::Tab { active, .. } if active => element
                    .vertical_align(tab_vertical_alignment)
                    .item_type(UIItemType::TabBar(item.item.clone()))
                    .margin(if is_bottom {
                        BoxDimension {
                            left: Dimension::Cells(0.),
                            right: Dimension::Cells(0.),
                            top: Dimension::Cells(0.),
                            bottom: Dimension::Cells(0.2),
                        }
                    } else {
                        BoxDimension {
                            left: Dimension::Cells(0.),
                            right: Dimension::Cells(0.),
                            top: Dimension::Cells(0.2),
                            bottom: Dimension::Cells(0.),
                        }
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.5),
                        top: Dimension::Cells(0.25),
                        bottom: Dimension::Cells(0.25),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(1.)))
                    .border_corners(Some(if is_bottom {
                        Corners {
                            top_left: SizedPoly::none(),
                            top_right: SizedPoly::none(),
                            bottom_left: SizedPoly {
                                width: fancy_radius,
                                height: fancy_radius,
                                poly: BOTTOM_LEFT_SUPERELLIPSE_CORNER,
                            },
                            bottom_right: SizedPoly {
                                width: fancy_radius,
                                height: fancy_radius,
                                poly: BOTTOM_RIGHT_SUPERELLIPSE_CORNER,
                            },
                        }
                    } else {
                        Corners {
                            top_left: SizedPoly {
                                width: fancy_radius,
                                height: fancy_radius,
                                poly: TOP_LEFT_SUPERELLIPSE_CORNER,
                            },
                            top_right: SizedPoly {
                                width: fancy_radius,
                                height: fancy_radius,
                                poly: TOP_RIGHT_SUPERELLIPSE_CORNER,
                            },
                            bottom_left: SizedPoly::none(),
                            bottom_right: SizedPoly::none(),
                        }
                    }))
                    .colors(ElementColors {
                        border: BorderColor::new(
                            bg_color
                                .unwrap_or_else(|| active_tab.bg_color.into())
                                .to_linear(),
                        ),
                        bg: bg_color
                            .unwrap_or_else(|| active_tab.bg_color.into())
                            .to_linear()
                            .into(),
                        text: fg_color
                            .unwrap_or_else(|| active_tab.fg_color.into())
                            .to_linear()
                            .into(),
                    }),
                TabBarItem::Tab { .. } => element
                    .vertical_align(tab_vertical_alignment)
                    .item_type(UIItemType::TabBar(item.item.clone()))
                    .margin(if is_bottom {
                        BoxDimension {
                            left: Dimension::Cells(0.),
                            right: Dimension::Cells(0.),
                            top: Dimension::Cells(0.),
                            bottom: Dimension::Cells(0.2),
                        }
                    } else {
                        BoxDimension {
                            left: Dimension::Cells(0.),
                            right: Dimension::Cells(0.),
                            top: Dimension::Cells(0.2),
                            bottom: Dimension::Cells(0.),
                        }
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.5),
                        top: Dimension::Cells(0.25),
                        bottom: Dimension::Cells(0.25),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(1.)))
                    .border_corners(Some(if is_bottom {
                        Corners {
                            top_left: SizedPoly {
                                width: Dimension::Cells(0.),
                                height: Dimension::Cells(0.33),
                                poly: &[],
                            },
                            top_right: SizedPoly {
                                width: Dimension::Cells(0.),
                                height: Dimension::Cells(0.33),
                                poly: &[],
                            },
                            bottom_left: SizedPoly {
                                width: fancy_radius,
                                height: fancy_radius,
                                poly: BOTTOM_LEFT_SUPERELLIPSE_CORNER,
                            },
                            bottom_right: SizedPoly {
                                width: fancy_radius,
                                height: fancy_radius,
                                poly: BOTTOM_RIGHT_SUPERELLIPSE_CORNER,
                            },
                        }
                    } else {
                        Corners {
                            top_left: SizedPoly {
                                width: fancy_radius,
                                height: fancy_radius,
                                poly: TOP_LEFT_SUPERELLIPSE_CORNER,
                            },
                            top_right: SizedPoly {
                                width: fancy_radius,
                                height: fancy_radius,
                                poly: TOP_RIGHT_SUPERELLIPSE_CORNER,
                            },
                            bottom_left: SizedPoly {
                                width: Dimension::Cells(0.),
                                height: Dimension::Cells(0.33),
                                poly: &[],
                            },
                            bottom_right: SizedPoly {
                                width: Dimension::Cells(0.),
                                height: Dimension::Cells(0.33),
                                poly: &[],
                            },
                        }
                    }))
                    .colors({
                        let inactive_tab = colors.inactive_tab();
                        let bg = bg_color
                            .unwrap_or_else(|| inactive_tab.bg_color.into())
                            .to_linear();
                        let edge = colors.inactive_tab_edge().to_linear();
                        ElementColors {
                            border: BorderColor {
                                left: bg,
                                right: edge,
                                top: bg,
                                bottom: bg,
                            },
                            bg: bg.into(),
                            text: fg_color
                                .unwrap_or_else(|| inactive_tab.fg_color.into())
                                .to_linear()
                                .into(),
                        }
                    })
                    .hover_colors({
                        let inactive_tab_hover = colors.inactive_tab_hover();
                        Some(ElementColors {
                            border: BorderColor::new(
                                bg_color
                                    .unwrap_or_else(|| inactive_tab_hover.bg_color.into())
                                    .to_linear(),
                            ),
                            bg: bg_color
                                .unwrap_or_else(|| inactive_tab_hover.bg_color.into())
                                .to_linear()
                                .into(),
                            text: fg_color
                                .unwrap_or_else(|| inactive_tab_hover.fg_color.into())
                                .to_linear()
                                .into(),
                        })
                    }),
                TabBarItem::WindowButton(button) => window_button_element(
                    button,
                    self.window_state.contains(window::WindowState::MAXIMIZED),
                    &font,
                    &metrics,
                    &self.config,
                ),
            }
        };

        let num_tabs: f32 = items
            .iter()
            .map(|item| match item.item {
                TabBarItem::NewTabButton | TabBarItem::Tab { .. } => 1.,
                _ => 0.,
            })
            .sum();
        let max_tab_width = ((self.dimensions.pixel_width as f32 / num_tabs)
            - (1.5 * metrics.cell_size.width as f32))
            .max(0.);

        let native_buttons_shown = self.native_traffic_lights_shown();

        for item in items {
            match item.item {
                TabBarItem::LeftStatus => left_status.push(item_to_elem(item)),
                TabBarItem::None | TabBarItem::RightStatus => right_eles.push(item_to_elem(item)),
                TabBarItem::WindowButton(_) => {
                    if self.config.integrated_title_button_alignment
                        == IntegratedTitleButtonAlignment::Left
                    {
                        left_eles.push(item_to_elem(item))
                    } else {
                        right_eles.push(item_to_elem(item))
                    }
                }
                TabBarItem::Tab { tab_idx, active } => {
                    let mut elem = item_to_elem(item);
                    elem.max_width = Some(Dimension::Pixels(max_tab_width));
                    elem.content = match elem.content {
                        ElementContent::Text(_) => unreachable!(),
                        ElementContent::Poly { .. } => unreachable!(),
                        ElementContent::Children(mut kids) => {
                            if self.config.show_close_tab_button_in_tabs {
                                kids.push(if style == TabStyle::Fancy {
                                    make_x_button(&font, &metrics, &colors, tab_idx, active)
                                } else {
                                    let tab_bg = if style == TabStyle::Capsule && active {
                                        colors.active_tab().bg_color.to_linear()
                                    } else {
                                        bar_bg
                                    };
                                    let fg = if active {
                                        colors.active_tab().fg_color
                                    } else {
                                        colors.inactive_tab().fg_color
                                    };
                                    make_flat_x_button(
                                        &font,
                                        &metrics,
                                        tab_idx,
                                        tab_bg,
                                        fg.to_linear(),
                                        colors.active_tab().fg_color.to_linear(),
                                    )
                                });
                            }
                            ElementContent::Children(kids)
                        }
                    };
                    left_eles.push(elem);
                    if style != TabStyle::Fancy && tab_idx + 1 < tab_count {
                        // A short line between this tab and the next, in the
                        // middle of the bar. Always there, so tabs don't shift
                        // when one hides; none beside the pill.
                        let beside_pill = style == TabStyle::Capsule
                            && (active || active_idx == Some(tab_idx + 1));
                        let color = if beside_pill {
                            bar_bg
                        } else {
                            colors.inactive_tab_edge().to_linear()
                        };
                        left_eles.push(
                            Element::new(
                                &font,
                                ElementContent::Poly {
                                    line_width: 1,
                                    poly: SizedPoly {
                                        poly: DIVIDER,
                                        width: Dimension::Pixels(1.),
                                        height: Dimension::Pixels(
                                            (tab_bar_height * DIVIDER_HEIGHT).round(),
                                        ),
                                    },
                                },
                            )
                            .vertical_align(VerticalAlign::Middle)
                            .item_type(UIItemType::TabBar(TabBarItem::None))
                            .colors(ElementColors {
                                border: BorderColor::default(),
                                bg: LinearRgba::TRANSPARENT.into(),
                                text: color.into(),
                            }),
                        );
                    }
                }
                _ => left_eles.push(item_to_elem(item)),
            }
        }

        let mut children = vec![];

        if !left_status.is_empty() {
            children.push(
                Element::new(&font, ElementContent::Children(left_status))
                    .colors(bar_colors.clone()),
            );
        }

        let window_buttons_at_left = self
            .config
            .window_decorations
            .contains(window::WindowDecorations::INTEGRATED_BUTTONS)
            && (self.config.integrated_title_button_alignment
                == IntegratedTitleButtonAlignment::Left
                || self.config.integrated_title_button_style
                    == IntegratedTitleButtonStyle::MacOsNative);

        // Tabs start the same gap after the traffic lights as the lights
        // are from the window's edge, or that gap from the edge without
        // them. In points, so it doesn't depend on the font.
        let points = |pt: f32| Dimension::Pixels(pt * self.dimensions.dpi as f32 / 72.);
        let left_padding = if native_buttons_shown {
            points(TRAFFIC_LIGHTS_END_PT + TRAFFIC_LIGHTS_GAP_PT)
        } else if window_buttons_at_left
            && self.config.integrated_title_button_style != IntegratedTitleButtonStyle::MacOsNative
        {
            Dimension::Pixels(0.0)
        } else if style == TabStyle::Fancy {
            Dimension::Cells(0.5)
        } else {
            points(TRAFFIC_LIGHTS_GAP_PT)
        };

        children.push(
            Element::new(&font, ElementContent::Children(left_eles))
                // Flat and Capsule tabs sit in the middle of the bar, level
                // with the traffic lights; Default tabs join the pane.
                .vertical_align(if style == TabStyle::Fancy {
                    tab_vertical_alignment
                } else {
                    VerticalAlign::Middle
                })
                .colors(bar_colors.clone())
                .padding(BoxDimension {
                    left: left_padding,
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.),
                    bottom: Dimension::Cells(0.),
                })
                .zindex(1),
        );
        children.push(
            Element::new(&font, ElementContent::Children(right_eles))
                .colors(bar_colors.clone())
                .float(Float::Right),
        );

        let content = ElementContent::Children(children);

        let tabs = Element::new(&font, content)
            .display(DisplayType::Block)
            .item_type(UIItemType::TabBar(TabBarItem::None))
            .min_width(Some(Dimension::Pixels(self.dimensions.pixel_width as f32)))
            .min_height(Some(Dimension::Pixels(tab_bar_height)))
            .vertical_align(tab_vertical_alignment)
            .colors(bar_colors);

        let border = self.get_os_border();

        let mut computed = self.compute_element(
            &LayoutContext {
                height: DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_height as f32,
                    pixel_cell: metrics.cell_size.height as f32,
                },
                width: DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_width as f32,
                    pixel_cell: metrics.cell_size.width as f32,
                },
                bounds: euclid::rect(
                    border.left.get() as f32,
                    0.,
                    self.dimensions.pixel_width as f32 - (border.left + border.right).get() as f32,
                    tab_bar_height,
                ),
                metrics: &metrics,
                gl_state: self.render_state.as_ref().unwrap(),
                zindex: 10,
            },
            &tabs,
        )?;

        computed.translate(euclid::vec2(
            0.,
            if self.config.tab_bar_at_bottom {
                self.dimensions.pixel_height as f32
                    - (computed.bounds.height() + border.bottom.get() as f32)
            } else {
                border.top.get() as f32
            },
        ));

        Ok(computed)
    }

    pub fn paint_fancy_tab_bar(&self) -> anyhow::Result<Vec<UIItem>> {
        let computed = self.fancy_tab_bar.as_ref().ok_or_else(|| {
            anyhow::anyhow!("paint_fancy_tab_bar called but fancy_tab_bar is None")
        })?;
        let ui_items = computed.ui_items();

        let gl_state = self.render_state.as_ref().unwrap();
        self.render_element(&computed, gl_state, None)?;

        Ok(ui_items)
    }
}

/// The close button for Flat and Capsule tabs: a small X in a square with
/// superellipse corners, which shows a faint background under the mouse.
fn make_flat_x_button(
    font: &Rc<LoadedFont>,
    metrics: &RenderMetrics,
    tab_idx: usize,
    tab_bg: LinearRgba,
    fg: LinearRgba,
    hover_fg: LinearRgba,
) -> Element {
    let cell_h = metrics.cell_size.height as f32;
    // The X is about a third of the line height, with as much space again
    // on each side, so the hover square is about the line's height.
    let x_size = (cell_h * 0.32).round();
    let pad = Dimension::Pixels(((cell_h - x_size) / 2.).floor());
    let radius = Dimension::Pixels((cell_h * 0.3).round());
    // The hover background is the text color at a low opacity over the tab.
    let mix = |a: f32, b: f32| a + (b - a) * 0.15;
    let hover_bg = LinearRgba(
        mix(tab_bg.0, hover_fg.0),
        mix(tab_bg.1, hover_fg.1),
        mix(tab_bg.2, hover_fg.2),
        tab_bg.3,
    );
    let rounded = |poly| SizedPoly {
        width: radius,
        height: radius,
        poly,
    };
    let border = |c: LinearRgba| BorderColor {
        left: c,
        right: c,
        top: c,
        bottom: c,
    };
    Element::new(
        &font,
        ElementContent::Poly {
            line_width: metrics.underline_height.max(2),
            poly: SizedPoly {
                poly: X_BUTTON,
                width: Dimension::Pixels(x_size),
                height: Dimension::Pixels(x_size),
            },
        },
    )
    .zindex(1)
    .vertical_align(VerticalAlign::Middle)
    .float(Float::Right)
    .item_type(UIItemType::CloseTab(tab_idx))
    .border_corners(Some(Corners {
        top_left: rounded(TOP_LEFT_SUPERELLIPSE_CORNER),
        top_right: rounded(TOP_RIGHT_SUPERELLIPSE_CORNER),
        bottom_left: rounded(BOTTOM_LEFT_SUPERELLIPSE_CORNER),
        bottom_right: rounded(BOTTOM_RIGHT_SUPERELLIPSE_CORNER),
    }))
    .colors(ElementColors {
        border: border(tab_bg),
        bg: tab_bg.into(),
        text: fg.into(),
    })
    .hover_colors(Some(ElementColors {
        border: border(hover_bg),
        bg: hover_bg.into(),
        text: hover_fg.into(),
    }))
    .padding(BoxDimension::new(pad))
    .margin(BoxDimension {
        left: Dimension::Cells(0.5),
        right: Dimension::Cells(0.),
        top: Dimension::Cells(0.),
        bottom: Dimension::Cells(0.),
    })
}

fn make_x_button(
    font: &Rc<LoadedFont>,
    metrics: &RenderMetrics,
    colors: &TabBarColors,
    tab_idx: usize,
    active: bool,
) -> Element {
    Element::new(
        &font,
        ElementContent::Poly {
            line_width: metrics.underline_height.max(2),
            poly: SizedPoly {
                poly: X_BUTTON,
                width: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
                height: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
            },
        },
    )
    // Ensure that we draw our background over the
    // top of the rest of the tab contents
    .zindex(1)
    .vertical_align(VerticalAlign::Middle)
    .float(Float::Right)
    .item_type(UIItemType::CloseTab(tab_idx))
    .hover_colors({
        let inactive_tab_hover = colors.inactive_tab_hover();
        let active_tab = colors.active_tab();

        Some(ElementColors {
            border: BorderColor::default(),
            bg: (if active {
                inactive_tab_hover.bg_color
            } else {
                active_tab.bg_color
            })
            .to_linear()
            .into(),
            text: (if active {
                inactive_tab_hover.fg_color
            } else {
                active_tab.fg_color
            })
            .to_linear()
            .into(),
        })
    })
    .padding(BoxDimension {
        left: Dimension::Cells(0.25),
        right: Dimension::Cells(0.25),
        top: Dimension::Cells(0.25),
        bottom: Dimension::Cells(0.25),
    })
    .margin(BoxDimension {
        left: Dimension::Cells(0.5),
        right: Dimension::Cells(0.),
        top: Dimension::Cells(0.),
        bottom: Dimension::Cells(0.),
    })
}
