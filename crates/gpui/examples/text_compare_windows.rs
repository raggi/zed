use std::sync::Arc;

#[cfg(windows)]
use anyhow::{Result, anyhow};
#[cfg(windows)]
use gpui::{
    App, Application, Element, Image, ImageFormat, KeyBinding, Menu, MenuItem, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, img, prelude::*, px, rgb,
    size,
};
#[cfg(windows)]
use image::{ImageBuffer, ImageEncoder, Rgba};
#[cfg(windows)]
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
#[cfg(windows)]
use windows::{
    Win32::{
        Graphics::{
            Direct2D::{
                D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
                D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
                D2D1CreateFactory, ID2D1RenderTarget, ID2D1SolidColorBrush,
            },
            DirectWrite::{
                DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_WORD_WRAPPING_NO_WRAP, IDWriteFactory, IDWriteTextFormat,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Imaging::{
                CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
                WICBitmapCacheOnLoad, WICRect,
            },
        },
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        },
    },
    core::{HSTRING, Interface},
};

#[cfg(windows)]
const RIGHT_PANEL_TITLE: &str = "GPUI";
#[cfg(windows)]
#[derive(Clone)]
struct LineSpec {
    text: &'static str,
    text_color: [f32; 4], // sRGB RGBA 0..1
    bg_color: [f32; 4],   // sRGB RGBA 0..1
    font: &'static str,
    size: f32,
}

#[cfg(windows)]
fn scenarios() -> Vec<LineSpec> {
    vec![
        LineSpec {
            text: "The quick brown fox jumps over the lazy dog 1234567890",
            text_color: srgb(0x00, 0x00, 0x00),
            bg_color: srgb(0xff, 0xff, 0xff),
            font: "Segoe UI",
            size: 13.0,
        },
        LineSpec {
            text: "The quick brown fox jumps over the lazy dog 1234567890",
            text_color: srgb(0xff, 0xff, 0xff),
            bg_color: srgb(0x00, 0x00, 0x00),
            font: "Segoe UI",
            size: 13.0,
        },
        LineSpec {
            text: "Sphinx of black quartz, judge my vow",
            text_color: srgb(0xaa, 0xaa, 0xaa),
            bg_color: srgb(0x16, 0x16, 0x16),
            font: "Segoe UI",
            size: 12.0,
        },
        LineSpec {
            text: "Pack my box with five dozen liquor jugs",
            text_color: srgb(0x1E, 0x90, 0xFF),
            bg_color: srgb(0xff, 0xff, 0xff),
            font: "Segoe UI",
            size: 12.0,
        },
        LineSpec {
            text: "How vexingly quick daft zebras jump!",
            text_color: srgb(0xFF, 0x44, 0x44),
            bg_color: srgb(0x00, 0x00, 0x00),
            font: "Segoe UI",
            size: 12.0,
        },
        LineSpec {
            text: "Waltz, bad nymph, for quick jigs vex",
            text_color: srgb(0x00, 0xC8, 0x53),
            bg_color: srgb(0x20, 0x20, 0x20),
            font: "Segoe UI",
            size: 12.0,
        },
        LineSpec {
            text: "Quick zephyrs blow, vexing daft Jim",
            text_color: srgb(0x00, 0xCC, 0xFF),
            bg_color: srgb(0xff, 0xff, 0xff),
            font: "Segoe UI",
            size: 12.0,
        },
        LineSpec {
            text: "Grumpy wizards make toxic brew for the jovial queen",
            text_color: srgb(0xFF, 0x00, 0x99),
            bg_color: srgb(0x00, 0x00, 0x00),
            font: "Segoe UI",
            size: 12.0,
        },
        LineSpec {
            text: "Hello emoji 😊🚀❤️👍",
            text_color: srgb(0xff, 0xff, 0xff),
            bg_color: srgb(0x00, 0x00, 0x00),
            font: "Segoe UI Emoji",
            size: 16.0,
        },
    ]
}

#[cfg(windows)]
fn srgb(r: u8, g: u8, b: u8) -> [f32; 4] {
    [
        (r as f32) / 255.0,
        (g as f32) / 255.0,
        (b as f32) / 255.0,
        1.0,
    ]
}

#[cfg(windows)]
#[derive(Copy, Clone)]
enum PanelMode {
    DirectClear,
    DirectGray,
}

#[cfg(windows)]
struct D2DContext {
    d2d: windows::Win32::Graphics::Direct2D::ID2D1Factory1,
    dwrite: IDWriteFactory,
    wic: IWICImagingFactory,
}

#[cfg(windows)]
impl D2DContext {
    fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        let d2d_base: windows::Win32::Graphics::Direct2D::ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        let d2d: windows::Win32::Graphics::Direct2D::ID2D1Factory1 = d2d_base.cast()?;
        let dwrite: IDWriteFactory = unsafe {
            windows::Win32::Graphics::DirectWrite::DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?
        };
        let wic: IWICImagingFactory =
            unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }?;
        Ok(Self { d2d, dwrite, wic })
    }
}

#[cfg(windows)]
struct RenderedPanel {
    width: u32,
    height: u32,
    rgba: Vec<u8>, // RGBA straight alpha
}

#[cfg(windows)]
fn render_panel(
    ctx: &D2DContext,
    mode: PanelMode,
    width: u32,
    line_height: u32,
) -> Result<RenderedPanel> {
    let lines = scenarios();
    let height = (lines.len() as u32) * line_height;
    let wic_bitmap = unsafe {
        ctx.wic.CreateBitmap(
            width,
            height,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapCacheOnLoad,
        )?
    };
    // Create D2D render target on WIC bitmap
    let rt_props = windows::Win32::Graphics::Direct2D::D2D1_RENDER_TARGET_PROPERTIES {
        r#type: windows::Win32::Graphics::Direct2D::D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        usage: windows::Win32::Graphics::Direct2D::D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: windows::Win32::Graphics::Direct2D::D2D1_FEATURE_LEVEL_DEFAULT,
    };

    let rt: ID2D1RenderTarget = unsafe {
        let base: windows::Win32::Graphics::Direct2D::ID2D1Factory = ctx.d2d.cast()?;
        base.CreateWicBitmapRenderTarget(&wic_bitmap, &rt_props)?
    };

    unsafe {
        rt.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        match mode {
            PanelMode::DirectClear => {
                rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);
            }
            PanelMode::DirectGray => {
                rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            }
        }
        rt.BeginDraw();
    }

    // Draw per-line
    let padding_left = 12.0f32;
    let mut y = 0.0f32;
    for spec in lines.iter() {
        // bg
        let rect = D2D_RECT_F {
            left: 0.0,
            top: y,
            right: width as f32,
            bottom: y + line_height as f32,
        };
        let brush_bg = solid_brush(&rt, spec.bg_color)?;
        unsafe { rt.FillRectangle(&rect, &brush_bg) };

        // text
        let fmt = make_text_format(&ctx.dwrite, spec.font, spec.size)?;
        unsafe {
            fmt.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            fmt.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        }
        let max_w = (width as f32) - padding_left - 12.0;

        let brush_fg = solid_brush(&rt, spec.text_color)?;
        let layout_rect = D2D_RECT_F {
            left: padding_left,
            top: y + 6.0,
            right: padding_left + max_w,
            bottom: y + (line_height as f32) - 2.0,
        };
        unsafe {
            rt.DrawText(
                &HSTRING::from(spec.text),
                &fmt,
                &layout_rect,
                &brush_fg,
                windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        y += line_height as f32;
    }

    unsafe {
        rt.EndDraw(None, None)?;
    }

    // Copy pixels out
    let rect = WICRect {
        X: 0,
        Y: 0,
        Width: width as i32,
        Height: height as i32,
    };
    let stride = (width * 4) as u32;
    let mut bgra_premul = vec![0u8; (height * stride) as usize];
    unsafe {
        wic_bitmap.CopyPixels(&rect as *const WICRect, stride, &mut bgra_premul)?;
    }

    let rgba = bgra_premul_to_rgba_straight(&bgra_premul);

    Ok(RenderedPanel {
        width,
        height,
        rgba,
    })
}

#[cfg(windows)]
fn solid_brush(rt: &ID2D1RenderTarget, rgba: [f32; 4]) -> Result<ID2D1SolidColorBrush> {
    let color = D2D1_COLOR_F {
        r: rgba[0],
        g: rgba[1],
        b: rgba[2],
        a: rgba[3],
    };
    let brush = unsafe { rt.CreateSolidColorBrush(&color, None)? };
    Ok(brush)
}

#[cfg(windows)]
fn make_text_format(dwrite: &IDWriteFactory, family: &str, size: f32) -> Result<IDWriteTextFormat> {
    let fmt: IDWriteTextFormat = unsafe {
        dwrite.CreateTextFormat(
            &HSTRING::from(family),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            &HSTRING::from("en-us"),
        )
    }?;
    Ok(fmt)
}

#[cfg(windows)]
fn bgra_premul_to_rgba_straight(bgra: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; bgra.len()];
    for (i, chunk) in bgra.chunks_exact(4).enumerate() {
        let b = chunk[0] as u32;
        let g = chunk[1] as u32;
        let r = chunk[2] as u32;
        let a = chunk[3] as u32;
        let (r2, g2, b2) = if a == 0 {
            (0, 0, 0)
        } else {
            let r2 = ((r * 255 + a / 2) / a).min(255);
            let g2 = ((g * 255 + a / 2) / a).min(255);
            let b2 = ((b * 255 + a / 2) / a).min(255);
            (r2, g2, b2)
        };
        let o = i * 4;
        out[o + 0] = r2 as u8;
        out[o + 1] = g2 as u8;
        out[o + 2] = b2 as u8;
        out[o + 3] = a as u8;
    }
    out
}

#[cfg(windows)]
fn encode_png_bytes(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| anyhow!("failed to build image buffer"))?;
    let mut buf: Vec<u8> = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    image::codecs::png::PngEncoder::new(&mut cursor).write_image(
        img.as_raw(),
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(buf)
}

#[cfg(windows)]
fn make_gpui_live_view() -> impl Element {
    // Build the same scenarios in GPUI so we can visually compare on the right panel
    let scenarios = scenarios();
    let mut column = div().flex().flex_col();
    for s in scenarios {
        let text = SharedString::from(s.text);
        let mut row = div().w_full().h(px(28.0)).bg(gpui::Rgba {
            r: s.bg_color[0],
            g: s.bg_color[1],
            b: s.bg_color[2],
            a: s.bg_color[3],
        });

        row = row.child(
            div()
                .text_color(gpui::Rgba {
                    r: s.text_color[0],
                    g: s.text_color[1],
                    b: s.text_color[2],
                    a: s.text_color[3],
                })
                .font_family(s.font)
                .text_size(px(s.size))
                .pl(px(12.0))
                .w(px(350.0))
                .line_height(px(28.0))
                .child(text),
        );
        column = column.child(row);
    }
    column
}

#[cfg(windows)]
actions!(text_compare_windows, [Quit]);

#[cfg(windows)]
#[derive(Clone)]
struct CompareView {
    direct_clear_img: Arc<Image>,
    direct_gray_img: Arc<Image>,
}

#[cfg(windows)]
impl gpui::Render for CompareView {
    fn render(
        &mut self,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        div()
            .size_full()
            .bg(rgb(0x111111))
            .text_color(rgb(0xeeeeee))
            .p(px(8.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex_1()
                            .p(px(8.0))
                            .bg(rgb(0xffffff))
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .text_color(rgb(0x000000))
                                    .child("DirectWrite Grayscale"),
                            )
                            .child(
                                div()
                                    .w(px(350.))
                                    .bg(rgb(0xffffff))
                                    .border_1()
                                    .border_color(rgb(0xdddddd))
                                    .overflow_hidden()
                                    .child(img(self.direct_gray_img.clone()).w(px(640.))),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p(px(8.0))
                            .bg(rgb(0xffffff))
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .text_color(rgb(0x000000))
                                    .child("DirectWrite ClearType"),
                            )
                            .child(
                                div()
                                    .w(px(350.))
                                    .bg(rgb(0xffffff))
                                    .border_1()
                                    .border_color(rgb(0xdddddd))
                                    .overflow_hidden()
                                    .child(img(self.direct_clear_img.clone()).w(px(640.))),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p(px(8.0))
                            .bg(rgb(0xffffff))
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .text_color(rgb(0x000000))
                                    .child(RIGHT_PANEL_TITLE),
                            )
                            .child(
                                div()
                                    .w(px(350.))
                                    .bg(rgb(0xffffff))
                                    .border_1()
                                    .border_color(rgb(0xdddddd))
                                    .overflow_hidden()
                                    .child(div().w(px(350.)).child(make_gpui_live_view())),
                            ),
                    ),
            )
    }
}

#[cfg(windows)]
fn run_example() -> Result<()> {
    env_logger::init();

    // Rasterize panels (DirectWrite ClearType and DirectWrite Grayscale)
    let ctx = D2DContext::new()?;
    let width = 640;
    let line_height = 28;
    let direct_clear = render_panel(&ctx, PanelMode::DirectClear, width, line_height)?;
    let direct_gray = render_panel(&ctx, PanelMode::DirectGray, width, line_height)?;

    // Encode PNGs for UI
    let direct_clear_png =
        encode_png_bytes(direct_clear.width, direct_clear.height, &direct_clear.rgba)?;
    let direct_gray_png =
        encode_png_bytes(direct_gray.width, direct_gray.height, &direct_gray.rgba)?;

    // Build UI
    Application::new().run(move |cx: &mut App| {
        cx.set_menus(vec![Menu {
            name: "Text Compare".into(),
            items: vec![MenuItem::action("Quit", Quit)],
        }]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

        let direct_clear_img = Arc::new(Image::from_bytes(ImageFormat::Png, direct_clear_png));
        let direct_gray_img = Arc::new(Image::from_bytes(ImageFormat::Png, direct_gray_png));

        let window = match cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from("Windows Text Compare")),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(gpui::Bounds {
                    size: size(px(1180.), px(340.)),
                    origin: gpui::Point::new(px(120.), px(80.)),
                })),
                ..Default::default()
            },
            move |_window, cx| {
                cx.new(|_| CompareView {
                    direct_clear_img: direct_clear_img.clone(),
                    direct_gray_img: direct_gray_img.clone(),
                })
            },
        ) {
            Ok(win) => win,
            Err(err) => {
                log::error!("Failed to open window: {err:?}");
                return;
            }
        };

        if let Err(err) = window.update(cx, |_view, _window, cx| {
            cx.activate(true);
        }) {
            log::error!("Failed to update window: {err:?}");
        }
    });

    Ok(())
}

#[cfg(windows)]
fn main() {
    if let Err(err) = run_example() {
        eprintln!("Error: {err:?}");
    }
}

#[cfg(not(windows))]
fn main() {
    println!("This example is Windows-only and requires Direct2D/DirectWrite.");
}
