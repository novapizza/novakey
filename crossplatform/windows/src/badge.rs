//! badge.rs
//! Renders the tray "V" / "E" badge at runtime, ported from the macOS
//! `StatusBarController.makeIcon(letter:colored:)`:
//!   - Vietnamese: red→orange→yellow gradient circle with a heavy white "V".
//!   - English: a bare heavy "E" that adapts to the taskbar theme (white on a
//!     dark taskbar, black on a light one) — the Windows equivalent of the
//!     macOS template icon.
//!
//! Drawing happens on a 32-bpp DIB with per-pixel alpha: the letter is first
//! rasterised by GDI into a grayscale coverage mask, then composited over the
//! gradient circle in software (premultiplied alpha, as icons require).

use std::ffi::c_void;

use windows::core::PCWSTR;
use windows::Win32::Foundation::COLORREF;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject,
    DrawTextW, GdiFlush, SelectObject, SetBkMode, SetTextColor, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE, DT_VCENTER, HBITMAP, HDC, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
};

/// The macOS brand gradient (NovaTheme.brandGradient), left → right.
const GRAD_L: [f32; 3] = [230.0, 38.0, 51.0]; // 0.90, 0.15, 0.20
const GRAD_M: [f32; 3] = [250.0, 110.0, 41.0]; // 0.98, 0.43, 0.16
const GRAD_R: [f32; 3] = [255.0, 199.0, 71.0]; // 1.00, 0.78, 0.28

/// Muted slate ramp for the English ("off") badge.
const OFF_L: [f32; 3] = [88.0, 95.0, 110.0];
const OFF_M: [f32; 3] = [110.0, 118.0, 134.0];
const OFF_R: [f32; 3] = [134.0, 142.0, 158.0];

/// Build the tray icon for the current language mode. Returns `None` if any
/// GDI step fails; the caller falls back to the embedded .ico resources.
/// The returned icon is owned by the caller (destroy with `DestroyIcon`).
pub fn create(vietnamese: bool) -> Option<HICON> {
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    create_sized(vietnamese, size)
}

/// `create`, with an explicit badge edge length in pixels.
pub fn create_sized(vietnamese: bool, size: i32) -> Option<HICON> {
    let pixels = render_bgra(vietnamese, size)?;
    unsafe {
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bmi = dib_info(size);
        let dc = CreateCompatibleDC(HDC::default());
        let color = match CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(b) => b,
            Err(_) => {
                let _ = DeleteDC(dc);
                return None;
            }
        };
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u8, pixels.len());

        // 32-bpp alpha icons ignore the AND mask, but CreateIconIndirect
        // still requires one.
        let mask = CreateBitmap(size, size, 1, 1, None);
        let info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&info).ok();
        let _ = DeleteObject(mask);
        let _ = DeleteObject(color);
        let _ = DeleteDC(dc);
        icon
    }
}

/// Compose the badge into a top-down, premultiplied BGRA buffer.
///
/// Both states share the circle so the tray does not visually jump on toggle;
/// only the fill and letter change. macOS can get away with a bare template
/// letter for English, but Windows tray icons are full-colour and a lone thin
/// glyph reads as a rendering glitch next to the gradient badge.
pub fn render_bgra(vietnamese: bool, size: i32) -> Option<Vec<u8>> {
    let letter = if vietnamese { "V" } else { "E" };
    // Letter ≈ 3/4 of the badge height (cap height then lands near 1/2),
    // scaled from the macOS icon's 12pt glyph in an 18pt badge.
    let coverage = unsafe { letter_coverage(letter, size, -(size * 3 / 4).max(8)) }?;
    // DT_CENTER centres the advance width and line box, not the glyph's ink, so
    // "V" and "E" each land a pixel or two off. At 16px that is visible.
    let coverage = center_ink(&coverage, size);

    let n = (size * size) as usize;
    let mut px = vec![0u8; n * 4];

    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            let t = coverage[(y * size + x) as usize] as f32 / 255.0;
            let c = circle_coverage(x, y, size);
            let [fr, fg, fb] = gradient(x as f32 / (size - 1).max(1) as f32, vietnamese);
            // White letter over the fill, the whole thing masked by the circle.
            let a = c;
            let r = (255.0 * t + fr * (1.0 - t)) * c;
            let g = (255.0 * t + fg * (1.0 - t)) * c;
            let b = (255.0 * t + fb * (1.0 - t)) * c;
            px[i] = b as u8;
            px[i + 1] = g as u8;
            px[i + 2] = r as u8;
            px[i + 3] = (a * 255.0) as u8;
        }
    }
    Some(px)
}

/// Rasterise `letter` centered in a size×size box and return its per-pixel
/// grayscale coverage (0 = background, 255 = fully inside the glyph).
unsafe fn letter_coverage(letter: &str, size: i32, font_h: i32) -> Option<Vec<u8>> {
    let mut bits: *mut c_void = std::ptr::null_mut();
    let bmi = dib_info(size);
    let dc = CreateCompatibleDC(HDC::default());
    let bmp: HBITMAP = match CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
        Ok(b) => b,
        Err(_) => {
            let _ = DeleteDC(dc);
            return None;
        }
    };
    let old_bmp = SelectObject(dc, bmp);

    let n = (size * size) as usize;
    std::ptr::write_bytes(bits as *mut u8, 0, n * 4); // black background

    // charset=DEFAULT(1), quality=ANTIALIASED(4) → grayscale coverage,
    // not ClearType subpixel tinting. Weight 900 matches the macOS `.heavy`.
    let face = wide("Segoe UI");
    let font = CreateFontW(font_h, 0, 0, 0, 900, 0, 0, 0, 1, 0, 0, 4, 0, PCWSTR(face.as_ptr()));
    let old_font = SelectObject(dc, font);
    SetTextColor(dc, COLORREF(0x00FF_FFFF));
    SetBkMode(dc, TRANSPARENT);

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: size,
        bottom: size,
    };
    let mut text: Vec<u16> = letter.encode_utf16().collect();
    DrawTextW(dc, &mut text, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    let _ = GdiFlush();

    let px = std::slice::from_raw_parts(bits as *const u8, n * 4);
    let coverage: Vec<u8> = (0..n).map(|i| px[i * 4 + 1]).collect();

    SelectObject(dc, old_font);
    let _ = DeleteObject(font);
    SelectObject(dc, old_bmp);
    let _ = DeleteObject(bmp);
    let _ = DeleteDC(dc);
    Some(coverage)
}

/// Shift a coverage mask so the glyph's ink bounding box is centred in the box.
fn center_ink(coverage: &[u8], size: i32) -> Vec<u8> {
    let ink = |x: i32, y: i32| coverage[(y * size + x) as usize] > 24;
    let cols: Vec<i32> = (0..size).filter(|&x| (0..size).any(|y| ink(x, y))).collect();
    let rows: Vec<i32> = (0..size).filter(|&y| (0..size).any(|x| ink(x, y))).collect();
    let (Some(&x0), Some(&x1), Some(&y0), Some(&y1)) =
        (cols.first(), cols.last(), rows.first(), rows.last())
    else {
        return coverage.to_vec(); // nothing drawn; nothing to centre
    };

    // Round toward the top-left so a 1px imbalance never clips at the edge.
    let dx = (size - 1 - x1 - x0) / 2;
    let dy = (size - 1 - y1 - y0) / 2;
    if dx == 0 && dy == 0 {
        return coverage.to_vec();
    }

    let mut out = vec![0u8; coverage.len()];
    for y in 0..size {
        let sy = y - dy;
        if sy < 0 || sy >= size {
            continue;
        }
        for x in 0..size {
            let sx = x - dx;
            if sx < 0 || sx >= size {
                continue;
            }
            out[(y * size + x) as usize] = coverage[(sy * size + sx) as usize];
        }
    }
    out
}

fn dib_info(size: i32) -> BITMAPINFO {
    BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Antialiased coverage of the circle inscribed in the badge (0.5px inset,
/// like the macOS `ovalIn: rect.insetBy(dx: 0.5, dy: 0.5)`).
fn circle_coverage(x: i32, y: i32, size: i32) -> f32 {
    let c = size as f32 / 2.0;
    let r = c - 0.5;
    let dx = x as f32 + 0.5 - c;
    let dy = y as f32 + 0.5 - c;
    let d = (dx * dx + dy * dy).sqrt();
    (r - d + 0.5).clamp(0.0, 1.0)
}

/// Three-stop horizontal gradient, t in 0..=1. The English badge uses a slate
/// ramp of the same shape so "off" reads as muted rather than as a different
/// icon, and stays visible on both light and dark taskbars.
fn gradient(t: f32, vietnamese: bool) -> [f32; 3] {
    let stops = if vietnamese {
        [GRAD_L, GRAD_M, GRAD_R]
    } else {
        [OFF_L, OFF_M, OFF_R]
    };
    let (a, b, f) = if t < 0.5 {
        (stops[0], stops[1], t * 2.0)
    } else {
        (stops[1], stops[2], (t - 0.5) * 2.0)
    };
    [
        a[0] + (b[0] - a[0]) * f,
        a[1] + (b[1] - a[1]) * f,
        a[2] + (b[2] - a[2]) * f,
    ]
}

/// UTF-16, NUL-terminated.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Print the alpha channel as ASCII art for eyeballing the glyph fit.
    #[test]
    #[ignore = "visual aid; run with --ignored --nocapture"]
    fn dump_badges() {
        for size in [16, 20, 24] {
            for vn in [true, false] {
                let px = render_bgra(vn, size).expect("render");
                println!("--- {} @ {size}px ---", if vn { "V" } else { "E" });
                for y in 0..size {
                    let row: String = (0..size)
                        .map(|x| {
                            let i = ((y * size + x) * 4) as usize;
                            let (b, g, r, a) = (px[i], px[i + 1], px[i + 2], px[i + 3]);
                            if a < 40 {
                                return ' '; // outside the circle
                            }
                            // Letter pixels are the near-white ones.
                            let lum = (b as u32 + g as u32 + r as u32) / 3;
                            if lum > 200 {
                                '#'
                            } else if lum > 160 {
                                '+'
                            } else {
                                '.'
                            }
                        })
                        .collect();
                    println!("|{row}|");
                }
            }
        }
    }

    /// Both badges must be a circle (opaque centre, clear corners) carrying a
    /// legible white letter, with premultiplied alpha as icons require.
    #[test]
    fn badge_is_drawn() {
        for size in [16, 20, 24, 32, 48] {
            for vn in [true, false] {
                let px = render_bgra(vn, size).expect("render");
                let at = |x: i32, y: i32| {
                    let i = ((y * size + x) * 4) as usize;
                    (px[i], px[i + 1], px[i + 2], px[i + 3])
                };
                let case = format!("vn={vn}, size={size}");

                assert_eq!(at(0, 0).3, 0, "corner not transparent ({case})");
                assert_eq!(at(size / 2, size / 2).3, 255, "centre not opaque ({case})");

                // A visible amount of the circle must be white letter ink, and
                // the letter must span the badge rather than sit in a corner.
                let ink: Vec<(i32, i32)> = (0..size)
                    .flat_map(|y| (0..size).map(move |x| (x, y)))
                    .filter(|&(x, y)| {
                        let (b, g, r, a) = at(x, y);
                        a > 200 && b > 225 && g > 225 && r > 225
                    })
                    .collect();
                assert!(
                    ink.len() >= (size * size / 24) as usize,
                    "letter barely visible: {} px ({case})",
                    ink.len()
                );
                let span = |f: fn(&(i32, i32)) -> i32| {
                    let vals: Vec<i32> = ink.iter().map(f).collect();
                    vals.iter().max().unwrap() - vals.iter().min().unwrap() + 1
                };
                // "E" is a narrow glyph, so only the height is held to 1/2.
                assert!(
                    span(|p| p.0) >= size / 4 && span(|p| p.1) >= size / 2,
                    "letter too small: {}x{} in {size}px ({case})",
                    span(|p| p.0),
                    span(|p| p.1)
                );

                // Centred within a pixel of the badge midpoint.
                let mid = |f: fn(&(i32, i32)) -> i32| {
                    let vals: Vec<i32> = ink.iter().map(f).collect();
                    vals.iter().max().unwrap() + vals.iter().min().unwrap()
                };
                assert!(
                    (mid(|p| p.0) - (size - 1)).abs() <= 1,
                    "letter off-centre horizontally ({case})"
                );
                assert!(
                    (mid(|p| p.1) - (size - 1)).abs() <= 1,
                    "letter off-centre vertically ({case})"
                );

                assert!(
                    px.chunks(4).all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]),
                    "not premultiplied ({case})"
                );
            }
        }
    }

    /// The two states must be clearly distinguishable by fill colour, not just
    /// by the letter — a glance at the tray should be enough.
    #[test]
    fn states_differ_in_colour() {
        let size = 24;
        let (vn, en) = (
            render_bgra(true, size).expect("render"),
            render_bgra(false, size).expect("render"),
        );
        // Sample the fill well away from the letter: the left edge of the circle.
        let i = ((size / 2 * size) + 1) as usize * 4;
        let (vr, vb) = (vn[i + 2] as i32, vn[i] as i32);
        let (er, eb) = (en[i + 2] as i32, en[i] as i32);
        assert!(vr - vb > 80, "Vietnamese fill not warm (r={vr}, b={vb})");
        assert!((er - eb).abs() < 40, "English fill not neutral (r={er}, b={eb})");
    }
}
