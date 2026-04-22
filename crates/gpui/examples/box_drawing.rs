#![cfg_attr(target_family = "wasm", no_main)]

use assets::Assets;
use gpui::{
    App, AssetSource, Bounds, Context, CursorStyle, ElementId, Font, FontFallbacks, FontFeatures,
    FontStyle, FontWeight, MouseMoveEvent, Point, RenderImage, TextRun, Window, WindowBounds,
    WindowOptions, WindowTextSystem, div, img, prelude::*, px, rgb, size, white,
};
use gpui_platform::application;
use image::imageops;
use smallvec::SmallVec;
use std::sync::Arc;

struct BoxDrawing {
    sizes: Vec<(f32, String)>,
    all_sizes: Vec<(f32, String)>,
    single_size_mode: bool,
    logged: bool,
    hover_font: Option<usize>,
    // Pixel-zoom state
    captured_rgba: Option<Arc<image::RgbaImage>>, // full-window capture, updated on hover enter
    zoom_mouse_pos: Point<gpui::Pixels>,          // last mouse position (window coords)
    zoom_image: Option<Arc<RenderImage>>,         // current cropped+upscaled view
    zoom_level: u32,                              // 4, 8, or 16
}

const ZOOM_LEVELS: &[u32] = &[4, 8, 16];

const FONTS: &[&str] = &["JetBrains Mono", "Menlo", "Lilex"];
const TEST_CHARS: &str = "╭╮╰╯─│";

const SIZES: &[(f32, &str)] = &[(12., "12px"), (14., "14px"), (16., "16px"), (24., "24px")];
const LINE_HEIGHT_MULTIPLIER: f32 = 1.3;

fn line_height_for(font_size: f32) -> f32 {
    (font_size * LINE_HEIGHT_MULTIPLIER).round()
}

fn text_col(font_size: f32, children: &[&'static str]) -> gpui::Div {
    let lh = line_height_for(font_size);
    let mut d = div()
        .flex()
        .flex_col()
        .text_color(rgb(0xffffff))
        .text_size(px(font_size))
        .line_height(px(lh));
    for child in children {
        d = d.child(*child);
    }
    d
}

fn box_columns(font_size: f32, tall: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .gap(px(32.))
        .items_start()
        .child(text_col(
            font_size,
            if tall {
                &["╭─────╮", "│     │", "│     │", "╰─────╯"]
            } else {
                &["╭─────╮", "│     │", "╰─────╯"]
            },
        ))
        .child(text_col(font_size, &["╭╮╰╯"]))
        .child(text_col(font_size, &["│╭─╮│", "│╰─╯│"]))
        .child(text_col(
            font_size,
            if tall {
                &["████", "▀▀▀▀", "████", "▄▄▄▄"]
            } else {
                &["████", "▀▀▀▀", "▄▄▄▄"]
            },
        ))
        .child(text_col(
            font_size,
            if tall {
                &["╭──╮█", "│  │█", "│  │█", "╰──╯█"]
            } else {
                &["╭──╮█", "│  │█", "╰──╯█"]
            },
        ))
}

/// Crop a region from the captured RGBA frame, nearest-neighbour upscale by `zoom`, convert to
/// BGRA (as GPUI's RenderImage expects), and return an Arc<RenderImage> ready for painting.
fn capture_zoom(
    rgba: &image::RgbaImage,
    src_x: u32,
    src_y: u32,
    src_w: u32,
    src_h: u32,
    zoom: u32,
) -> Option<Arc<RenderImage>> {
    let x = src_x.min(rgba.width().saturating_sub(1));
    let y = src_y.min(rgba.height().saturating_sub(1));
    let w = src_w.min(rgba.width().saturating_sub(x));
    let h = src_h.min(rgba.height().saturating_sub(y));
    if w == 0 || h == 0 {
        return None;
    }
    let cropped = imageops::crop_imm(rgba, x, y, w, h).to_image();
    let zoomed = imageops::resize(&cropped, w * zoom, h * zoom, imageops::FilterType::Nearest);
    // Convert RGBA → BGRA in-place (GPUI's atlas shader expects BGRA)
    let mut raw = zoomed.into_raw();
    for px in raw.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let bgra = image::RgbaImage::from_raw(w * zoom, h * zoom, raw)?;
    Some(Arc::new(RenderImage::new(SmallVec::from_elem(
        image::Frame::new(bgra),
        1,
    ))))
}

/// Capture the previous rendered frame as a full-window RGBA image.
fn capture_full_frame(window: &Window) -> Option<Arc<image::RgbaImage>> {
    window.render_to_image().ok().map(Arc::new)
}

/// Crop a region centred on the mouse position, upscale by `zoom` with nearest-neighbour, and
/// return an Arc<RenderImage> (BGRA, ready for GPUI's atlas).
///
/// The source half-extent shrinks as zoom increases so the output is always ~300×300 logical:
///   zoom=4:  source half = 75 logical (150 device on Retina) → 600 px output
///   zoom=8:  source half = 37 logical (75 device on Retina)  → 600 px output
///   zoom=16: source half = 19 logical (38 device on Retina)  → 608 px output (~300 logical)
fn crop_zoom_at(
    rgba: &image::RgbaImage,
    mouse_x: f32,
    mouse_y: f32,
    scale: f32,
    zoom: u32,
) -> Option<Arc<RenderImage>> {
    // half = 75 logical pixels / (zoom/4) → scales with zoom so output stays ~300 logical wide
    let half = ((75.0 * scale) / (zoom as f32 / 4.0)).round() as u32;
    let cx = (mouse_x * scale) as u32;
    let cy = (mouse_y * scale) as u32;
    let src_x = cx.saturating_sub(half);
    let src_y = cy.saturating_sub(half);
    capture_zoom(rgba, src_x, src_y, half * 2, half * 2, zoom)
}

impl Render for BoxDrawing {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        struct GlyphInfo {
            ch: char,
            family: String,
            y: f32,
        }
        let diagnostics: Vec<(&str, Vec<GlyphInfo>)> = FONTS
            .iter()
            .map(|font_name| {
                let run = TextRun {
                    len: TEST_CHARS.len(),
                    font: Font {
                        family: (*font_name).into(),
                        features: FontFeatures::default(),
                        fallbacks: None::<FontFallbacks>,
                        weight: FontWeight::default(),
                        style: FontStyle::default(),
                    },
                    color: white(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let layout = WindowTextSystem::layout_line(
                    window.text_system(),
                    TEST_CHARS,
                    px(16.),
                    &[run],
                    None,
                );
                let glyphs: Vec<GlyphInfo> = layout
                    .runs
                    .iter()
                    .flat_map(|shaped_run| {
                        let family = cx
                            .text_system()
                            .font_name_for_id(shaped_run.font_id)
                            .unwrap_or_else(|| format!("#{}", shaped_run.font_id.0));
                        shaped_run.glyphs.iter().filter_map(move |g| {
                            TEST_CHARS[g.index..].chars().next().map(|ch| GlyphInfo {
                                ch,
                                family: family.clone(),
                                y: g.position.y.as_f32(),
                            })
                        })
                    })
                    .collect();
                (*font_name, glyphs)
            })
            .collect();

        let tall = self.sizes.len() == 1;

        if !self.logged {
            self.logged = true;
            eprintln!("╭─────╮  ████");
            eprintln!("│     │  ▀▀▀▀");
            if tall {
                eprintln!("│     │  ████");
            }
            eprintln!("╰─────╯  ▄▄▄▄");
            eprintln!();
            for (font_name, glyphs) in &diagnostics {
                eprintln!("[{}]", font_name);
                for glyph in glyphs {
                    eprintln!("  {:?}  font={}  y={:.3}", glyph.ch, glyph.family, glyph.y);
                }
                let font_id = cx.text_system().resolve_font(&Font {
                    family: (*font_name).into(),
                    features: FontFeatures::default(),
                    fallbacks: None::<FontFallbacks>,
                    weight: FontWeight::default(),
                    style: FontStyle::default(),
                });
                for size in &self.sizes {
                    let font_size = px(size.0);
                    for ch in ['╭', '╰', '─', '│'] {
                        if let Ok(b) = cx.text_system().typographic_bounds(font_id, font_size, ch) {
                            eprintln!(
                                "  {}  {:?}  y=[{:.3}..{:.3}]  x=[{:.3}..{:.3}]",
                                size.1,
                                ch,
                                b.origin.y.as_f32(),
                                b.origin.y.as_f32() + b.size.height.as_f32(),
                                b.origin.x.as_f32(),
                                b.origin.x.as_f32() + b.size.width.as_f32(),
                            );
                        }
                    }
                    for ch in ['█', '▀', '▄', '▌', '▐'] {
                        if let Ok(b) = cx.text_system().typographic_bounds(font_id, font_size, ch) {
                            eprintln!(
                                "  {}  {:?}  y=[{:.3}..{:.3}]  x=[{:.3}..{:.3}]",
                                size.1,
                                ch,
                                b.origin.y.as_f32(),
                                b.origin.y.as_f32() + b.size.height.as_f32(),
                                b.origin.x.as_f32(),
                                b.origin.x.as_f32() + b.size.width.as_f32(),
                            );
                        }
                    }
                    let line_height = line_height_for(size.0);
                    let b_corner = cx.text_system().typographic_bounds(font_id, font_size, '╭');
                    let b_wall = cx.text_system().typographic_bounds(font_id, font_size, '│');
                    let b_corner_bot =
                        cx.text_system().typographic_bounds(font_id, font_size, '╰');
                    if let (Ok(bc), Ok(bw), Ok(bbot)) = (b_corner, b_wall, b_corner_bot) {
                        let corner_screen_bot = -bc.origin.y.as_f32();
                        let wall_screen_top_next =
                            line_height - (bw.origin.y.as_f32() + bw.size.height.as_f32());
                        let overlap_top = corner_screen_bot - wall_screen_top_next;
                        eprintln!(
                            "  {}  join ╭↓│ (lh={:.1}): overlap={:.3}px{}",
                            size.1,
                            line_height,
                            overlap_top,
                            if overlap_top > 0.3 {
                                "  ← OVERLAP"
                            } else if overlap_top < -0.3 {
                                "  ← GAP"
                            } else {
                                "  ✓"
                            }
                        );
                        let wall_screen_bot = -bw.origin.y.as_f32();
                        let corner_bot_top_next =
                            line_height - (bbot.origin.y.as_f32() + bbot.size.height.as_f32());
                        let overlap_bot = wall_screen_bot - corner_bot_top_next;
                        eprintln!(
                            "  {}  join │↓╰ (lh={:.1}): overlap={:.3}px{}",
                            size.1,
                            line_height,
                            overlap_bot,
                            if overlap_bot > 0.3 {
                                "  ← OVERLAP"
                            } else if overlap_bot < -0.3 {
                                "  ← GAP"
                            } else {
                                "  ✓"
                            }
                        );
                        let wall_cont = bw.size.height.as_f32() - line_height;
                        eprintln!(
                            "  {}  │ continuity: height={:.3}  lh={:.1}  overlap={:.3}px{}",
                            size.1,
                            bw.size.height.as_f32(),
                            line_height,
                            wall_cont,
                            if wall_cont > 0.3 {
                                "  ← OVERLAP"
                            } else if wall_cont < -0.3 {
                                "  ← GAP"
                            } else {
                                "  ✓"
                            }
                        );
                    }
                    let cell_w = cx
                        .text_system()
                        .advance(font_id, font_size, '─')
                        .map(|a| a.width.as_f32())
                        .unwrap_or(font_size.as_f32() * 0.6);
                    let b_corner_h = cx.text_system().typographic_bounds(font_id, font_size, '╭');
                    let b_horiz = cx.text_system().typographic_bounds(font_id, font_size, '─');
                    if let (Ok(bc), Ok(bh)) = (b_corner_h, b_horiz) {
                        let corner_right = bc.origin.x.as_f32() + bc.size.width.as_f32();
                        let horiz_left_next = cell_w + bh.origin.x.as_f32();
                        let h_gap = horiz_left_next - corner_right;
                        eprintln!(
                            "  {}  join ╭→─ (cell_w={:.3}): gap={:.3}px{}",
                            size.1,
                            cell_w,
                            h_gap,
                            if h_gap < 0.0 {
                                "  ← OVERLAP"
                            } else if h_gap > 0.5 {
                                "  ← GAP"
                            } else {
                                "  ✓"
                            }
                        );
                    }
                }
            }

            // Scan all box-drawing chars at 14px; reports any glyph with |position.y| > 0.
            let all_box_chars: Arc<str> = (0x2500u32..=0x257F)
                .chain(0x2580..=0x259F)
                .chain(0x25A0..=0x25FF)
                .chain(0xE0B0..=0xE0D7)
                .filter_map(char::from_u32)
                .collect::<String>()
                .into();
            let total = all_box_chars.chars().count();
            eprintln!();
            eprintln!("── glyph.position.y scan: {} box-drawing chars at 14px ──", total);
            for font_name in FONTS {
                let run = TextRun {
                    len: all_box_chars.len(), // byte length
                    font: Font {
                        family: (*font_name).into(),
                        features: FontFeatures::default(),
                        fallbacks: None::<FontFallbacks>,
                        weight: FontWeight::default(),
                        style: FontStyle::default(),
                    },
                    color: white(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let layout = WindowTextSystem::layout_line(
                    window.text_system(),
                    &*all_box_chars,
                    px(14.),
                    &[run],
                    None,
                );
                let chars = all_box_chars.clone();
                let nonzero: Vec<(char, f32, String)> = layout
                    .runs
                    .iter()
                    .flat_map(|shaped_run| {
                        let family = cx
                            .text_system()
                            .font_name_for_id(shaped_run.font_id)
                            .unwrap_or_else(|| format!("#{}", shaped_run.font_id.0));
                        let chars = chars.clone();
                        shaped_run.glyphs.iter().filter_map(move |g| {
                            let y = g.position.y.as_f32();
                            if y.abs() > 0.001 {
                                chars[g.index..]
                                    .chars()
                                    .next()
                                    .map(|ch| (ch, y, family.clone()))
                            } else {
                                None
                            }
                        })
                    })
                    .collect();
                if nonzero.is_empty() {
                    eprintln!("[{}] all {} chars have y=0", font_name, total);
                } else {
                    eprintln!(
                        "[{}] {} / {} chars have non-zero y:",
                        font_name,
                        nonzero.len(),
                        total
                    );
                    for (ch, y, family) in &nonzero {
                        eprintln!(
                            "  {:?} U+{:04X}  y={:+.3}  font={}",
                            ch,
                            *ch as u32,
                            y,
                            family
                        );
                    }
                }
            }
        }

        // Zoom panel — shows the actual rendered pixels at N× (captured from the last frame)
        let zoom_font_idx = self.hover_font.unwrap_or(0);
        let zoom_font_name = FONTS[zoom_font_idx];
        let zoom_size = self.sizes[0].0;
        let is_hovering = self.hover_font.is_some();
        let zoom_image = self.zoom_image.clone();
        let current_zoom = self.zoom_level;

        let zoom_buttons = div()
            .flex()
            .flex_row()
            .gap(px(3.))
            .children(ZOOM_LEVELS.iter().map(|&level| {
                let is_current = level == current_zoom;
                div()
                    .id(ElementId::Integer(200 + level as u64))
                    .cursor_pointer()
                    .px(px(5.))
                    .py(px(1.))
                    .rounded(px(3.))
                    .bg(if is_current { rgb(0x1a2a4a) } else { rgb(0x252535) })
                    .text_size(px(10.))
                    .text_color(if is_current { rgb(0x8899ff) } else { rgb(0x445566) })
                    .child(format!("{level}×"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.zoom_level = level;
                        if let Some(rgba) = this.captured_rgba.clone() {
                            let scale = window.scale_factor();
                            let mx = this.zoom_mouse_pos.x.as_f32();
                            let my = this.zoom_mouse_pos.y.as_f32();
                            this.zoom_image = crop_zoom_at(&rgba, mx, my, scale, level);
                        }
                        cx.notify();
                    }))
            }));

        // Measurement annotation for the panel footer
        let measurement_text = {
            let font_id = cx.text_system().resolve_font(&Font {
                family: zoom_font_name.into(),
                features: FontFeatures::default(),
                fallbacks: None::<FontFallbacks>,
                weight: FontWeight::default(),
                style: FontStyle::default(),
            });
            let font_size = px(zoom_size);
            let lh = line_height_for(zoom_size);
            if let Ok(bw) = cx.text_system().typographic_bounds(font_id, font_size, '│') {
                let overlap = bw.size.height.as_f32() - lh;
                format!(
                    "│ h={:.2}px  lh={:.1}px  overlap={:+.2}px",
                    bw.size.height.as_f32(),
                    lh,
                    overlap
                )
            } else {
                String::new()
            }
        };

        let zoom_content: gpui::AnyElement = if let Some(image) = zoom_image {
            // Fixed 300×300 logical: at 2× Retina this is 600×600 physical display pixels.
            // The image is a 4× upscale of a 150×150 device-pixel source → 600×600 px in the
            // image buffer, so each source physical pixel maps to exactly 4 display pixels.
            img(image).w(px(300.)).h(px(300.)).into_any_element()
        } else {
            div()
                .p(px(16.))
                .text_size(px(11.))
                .text_color(rgb(0x333355))
                .child("hover a font section to capture pixels")
                .into_any_element()
        };

        let zoom_panel = div()
            .w(px(300.))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(0x191924))
            .border_l_1()
            .border_color(rgb(0x2a2a3a))
            .flex()
            .flex_col()
            .child(
                div()
                    .px(px(10.))
                    .py(px(6.))
                    .bg(rgb(0x20202e))
                    .border_b_1()
                    .border_color(rgb(0x2a2a3a))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(zoom_buttons)
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(if is_hovering {
                                rgb(0x8899ff)
                            } else {
                                rgb(0x444466)
                            })
                            .child(if is_hovering {
                                format!("{zoom_font_name}  {zoom_size}px")
                            } else {
                                "← hover a font section".to_string()
                            }),
                    ),
            )
            .child(div().flex_1().overflow_hidden().child(zoom_content))
            .child(
                div()
                    .px(px(12.))
                    .py(px(6.))
                    .border_t_1()
                    .border_color(rgb(0x2a2a3a))
                    .text_size(px(10.))
                    .text_color(rgb(0x556677))
                    .child(measurement_text),
            );

        // Toolbar — build size picker first so cx.listener calls happen before any move closure
        let single_size_mode = self.single_size_mode;
        let current_size = self.sizes[0].0;
        let size_picker = single_size_mode.then(|| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .children(self.all_sizes.iter().enumerate().map(|(idx, (sz, lbl))| {
                    let is_current = (sz - current_size).abs() < 0.01;
                    let sz = *sz;
                    let lbl = lbl.clone();
                    div()
                        .id(ElementId::Integer(100 + idx as u64))
                        .cursor_pointer()
                        .px(px(8.))
                        .py(px(3.))
                        .rounded(px(4.))
                        .bg(if is_current { rgb(0x1a3a1a) } else { rgb(0x333333) })
                        .text_size(px(12.))
                        .text_color(if is_current { rgb(0x66cc66) } else { rgb(0x777777) })
                        .child(lbl)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.sizes = vec![(sz, format!("{sz}px"))];
                            this.logged = false;
                            cx.notify();
                        }))
                }))
        });
        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(16.))
            .px(px(20.))
            .py(px(10.))
            .bg(rgb(0x2a2a2a))
            .when_some(size_picker, |row, picker| row.child(picker));

        // Glyph output
        let glyph_output = div()
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(32.))
            .children(FONTS.iter().enumerate().map(|(font_idx, font)| {
                div()
                    .id(ElementId::Integer(font_idx as u64))
                    .cursor(CursorStyle::Crosshair)
                    .on_hover(cx.listener(move |this, hovered, window, cx| {
                        this.hover_font = if *hovered { Some(font_idx) } else { None };
                        if *hovered {
                            // Capture full window pixels once on hover enter; mouse-move re-crops cheaply.
                            if let Some(rgba) = capture_full_frame(window) {
                                let scale = window.scale_factor();
                                let mx = this.zoom_mouse_pos.x.as_f32();
                                let my = this.zoom_mouse_pos.y.as_f32();
                                this.zoom_image =
                                    crop_zoom_at(&rgba, mx, my, scale, this.zoom_level);
                                this.captured_rgba = Some(rgba);
                            }
                        }
                        cx.notify();
                    }))
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, window, cx| {
                        this.zoom_mouse_pos = event.position;
                        if let Some(rgba) = this.captured_rgba.clone() {
                            let scale = window.scale_factor();
                            let mx = event.position.x.as_f32();
                            let my = event.position.y.as_f32();
                            this.zoom_image =
                                crop_zoom_at(&rgba, mx, my, scale, this.zoom_level);
                            cx.notify();
                        }
                    }))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div()
                            .text_color(rgb(0xaaaaaa))
                            .text_size(px(11.))
                            .child(*font),
                    )
                    .children(self.sizes.iter().map(|(font_size, size_label)| {
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(16.))
                            .items_start()
                            .font_family(*font)
                            .child(
                                div()
                                    .w(px(40.))
                                    .text_color(rgb(0x666666))
                                    .text_size(px(11.))
                                    .child(size_label.clone()),
                            )
                            .child(box_columns(*font_size, tall))
                    }))
            }));

        let diagnostics_panel = div()
            .px(px(24.))
            .pb(px(24.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .text_color(rgb(0x666666))
                    .text_size(px(11.))
                    .child("── glyph fallback at 16px (char → resolved font) ──"),
            )
            .children(diagnostics.into_iter().map(
                |(font_name, glyphs): (&str, Vec<GlyphInfo>)| {
                    let entries: Vec<(char, String, bool)> = glyphs
                        .iter()
                        .map(|g| (g.ch, g.family.clone(), g.family != font_name))
                        .collect();
                    let any_fallback = entries.iter().any(|(_, _, fell_back)| *fell_back);
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_color(rgb(0x888888))
                                .text_size(px(11.))
                                .child(font_name),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .gap(px(8.))
                                .children(entries.into_iter().map(
                                    |(ch, family, fell_back)| {
                                        let color = if fell_back {
                                            rgb(0xffaa44)
                                        } else {
                                            rgb(0x666666)
                                        };
                                        div().text_color(color).text_size(px(11.)).child(
                                            if fell_back {
                                                format!("{ch} → {family}")
                                            } else {
                                                format!("{ch} ✓")
                                            },
                                        )
                                    },
                                ))
                                .when(!any_fallback, |d| {
                                    d.child(
                                        div()
                                            .text_color(rgb(0x44aa66))
                                            .text_size(px(11.))
                                            .child("all glyphs in requested font"),
                                    )
                                }),
                        )
                },
            ));

        div()
            .bg(rgb(0x1e1e1e))
            .size_full()
            .flex()
            .flex_col()
            .child(toolbar)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0() // allows the scroll child to shrink below its content height
                    .child(
                        div()
                            .id("scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .child(glyph_output)
                            .child(diagnostics_panel),
                    )
                    .child(zoom_panel),
            )
    }
}

fn parse_sizes() -> (Vec<(f32, String)>, bool) {
    let args: Vec<String> = std::env::args().collect();
    let mut requested: Vec<f32> = args
        .windows(2)
        .filter(|w| w[0] == "--px")
        .filter_map(|w| w[1].parse::<f32>().ok())
        .collect();

    if requested.is_empty() {
        (
            SIZES.iter().map(|(s, l)| (*s, l.to_string())).collect(),
            false,
        )
    } else {
        requested.sort_by(|a, b| a.partial_cmp(b).unwrap());
        requested.dedup();
        (
            requested
                .into_iter()
                .map(|s| (s, format!("{s}px")))
                .collect(),
            true,
        )
    }
}

fn run_example() {
    let (sizes, single_size_mode) = parse_sizes();
    let all_sizes: Vec<(f32, String)> = SIZES.iter().map(|(s, l)| (*s, l.to_string())).collect();
    application().run(move |cx: &mut App| {
        let font_paths = Assets.list("fonts").unwrap();
        let fonts: Vec<_> = font_paths
            .iter()
            .filter(|p| p.ends_with(".ttf"))
            .filter_map(|p| Assets.load(p).ok().flatten())
            .collect();
        cx.text_system().add_fonts(fonts).unwrap();
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(980.), px(860.)), cx);
        cx.open_window(
            WindowOptions {
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Box Drawing Alignment".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            {
                let sizes = sizes.clone();
                let all_sizes = all_sizes.clone();
                move |_, cx| {
                    cx.new(move |_| BoxDrawing {
                        sizes: sizes.clone(),
                        all_sizes: all_sizes.clone(),
                        single_size_mode,
                        logged: false,
                        hover_font: None,
                        captured_rgba: None,
                        zoom_mouse_pos: gpui::Point::default(),
                        zoom_image: None,
                        zoom_level: 4,
                    })
                }
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    run_example();
}
