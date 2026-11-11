//! The pictures converted sources draw (Android's `Bitmap`, `BitmapFactory`, `Canvas`), held in
//! Rust (`raygun_net::bitmap::Bitmaps`) and reached by number from `mihon/kotlin.js`; and bytes
//! between `Uint8Array`s and base64 without a pass in script.

use std::cell::RefCell;
use std::rc::Rc;

use base64::Engine as _;
use raygun_net::bitmap::{Affine, Bitmaps};
use rquickjs::{Ctx, Function, IntoJs, Object, TypedArray, Value};
use serde::Deserialize;
use serde_json::json;

use super::text;

/// What `__rgMihon.bitmap(op, args)` is asked.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
enum Op {
    Create {
        width: u32,
        height: u32,
        #[serde(default)]
        color: u32,
    },
    Crop {
        id: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    Scaled {
        id: u32,
        width: u32,
        height: u32,
        #[serde(default)]
        filter: bool,
    },
    Copy {
        id: u32,
    },
    Free {
        id: u32,
    },
    Size {
        id: u32,
    },
    /// Picture `source`'s part `[left, top, right, bottom]` into `target` through `matrix`
    /// (`[a, b, c, d, e, f]`, from the source's pixels to the target's).
    Draw {
        target: u32,
        source: u32,
        part: [i64; 4],
        matrix: [f64; 6],
        #[serde(default)]
        clip: Option<[i64; 4]>,
    },
    Fill {
        id: u32,
        #[serde(default)]
        rect: Option<[f64; 4]>,
        #[serde(default = "identity")]
        matrix: [f64; 6],
        color: u32,
        #[serde(default)]
        replace: bool,
    },
    Pixels {
        id: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    SetPixels {
        id: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        colors: Vec<i64>,
    },
    /// Android's `ColorMatrix` (4 rows of 5: red, green, blue, alpha from the pixel's and an
    /// offset) applied to every pixel of picture `id`, as a `ColorMatrixColorFilter` draws it.
    Colors {
        id: u32,
        matrix: [f64; 20],
    },
    /// Text laid out in lines `width` wide (Android's `StaticLayout`): its height.
    MeasureText {
        text: String,
        size: f32,
        width: f32,
        #[serde(default = "one")]
        spacing: f32,
        #[serde(default)]
        extra: f32,
        #[serde(default)]
        bold: bool,
    },
    /// The same text drawn on picture `id` at `(x, y)` through `matrix`, in `color`.
    DrawText {
        id: u32,
        text: String,
        size: f32,
        width: f32,
        #[serde(default = "one")]
        spacing: f32,
        #[serde(default)]
        extra: f32,
        #[serde(default)]
        bold: bool,
        x: f32,
        y: f32,
        /// `y` is the first line's baseline (`canvas.drawText(text, x, y, paint)`), not its top.
        #[serde(default)]
        baseline: bool,
        #[serde(default = "identity")]
        matrix: [f64; 6],
        color: u32,
    },
}

fn identity() -> [f64; 6] {
    [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
}

fn one() -> f32 {
    1.0
}

fn affine([a, b, c, d, e, f]: [f64; 6]) -> Affine {
    Affine { a, b, c, d, e, f }
}

fn run(bitmaps: &mut Bitmaps, op: Op) -> Result<serde_json::Value, String> {
    let picture = |bitmaps: &Bitmaps, id: u32| -> Result<serde_json::Value, String> {
        let (width, height) = bitmaps.size(id)?;
        Ok(json!({ "id": id, "width": width, "height": height }))
    };
    Ok(match op {
        Op::Create {
            width,
            height,
            color,
        } => {
            let id = bitmaps.create(width, height, color)?;
            picture(bitmaps, id)?
        }
        Op::Crop {
            id,
            x,
            y,
            width,
            height,
        } => {
            let id = bitmaps.crop(id, x, y, width, height)?;
            picture(bitmaps, id)?
        }
        Op::Scaled {
            id,
            width,
            height,
            filter,
        } => {
            let id = bitmaps.scaled(id, width, height, filter)?;
            picture(bitmaps, id)?
        }
        Op::Copy { id } => {
            let id = bitmaps.copy(id)?;
            picture(bitmaps, id)?
        }
        Op::Free { id } => {
            bitmaps.free(id);
            json!(null)
        }
        Op::Size { id } => picture(bitmaps, id)?,
        Op::Draw {
            target,
            source,
            part,
            matrix,
            clip,
        } => {
            bitmaps.draw(
                target,
                source,
                (part[0], part[1], part[2], part[3]),
                affine(matrix),
                clip.map(|clip| (clip[0], clip[1], clip[2], clip[3])),
            )?;
            json!(null)
        }
        Op::Fill {
            id,
            rect,
            matrix,
            color,
            replace,
        } => {
            bitmaps.fill(
                id,
                rect.map(|rect| (rect[0], rect[1], rect[2], rect[3])),
                affine(matrix),
                color,
                replace,
            )?;
            json!(null)
        }
        Op::Pixels {
            id,
            x,
            y,
            width,
            height,
        } => {
            // Java's ints are signed.
            let colors: Vec<i32> = bitmaps
                .pixels_of(id, x, y, width, height)?
                .into_iter()
                .map(|color| color as i32)
                .collect();
            json!(colors)
        }
        Op::Colors { id, matrix } => {
            let (width, height) = bitmaps.size(id)?;
            let colors: Vec<u32> = bitmaps
                .pixels_of(id, 0, 0, width, height)?
                .into_iter()
                .map(|color| color_matrix(color, &matrix))
                .collect();
            bitmaps.set_pixels(id, 0, 0, width, height, &colors)?;
            json!(null)
        }
        Op::SetPixels {
            id,
            x,
            y,
            width,
            height,
            colors,
        } => {
            let colors: Vec<u32> = colors.into_iter().map(|color| color as u32).collect();
            bitmaps.set_pixels(id, x, y, width, height, &colors)?;
            json!(null)
        }
        Op::MeasureText {
            text,
            size,
            width,
            spacing,
            extra,
            bold,
        } => {
            let layout = text::Layout::new(&text, size, width, spacing, extra, bold)?;
            json!({
                "height": layout.height(),
                "lines": layout.line_count(),
                "width": layout.width(),
            })
        }
        Op::DrawText {
            id,
            text,
            size,
            width,
            spacing,
            extra,
            bold,
            x,
            y,
            baseline,
            matrix,
            color,
        } => {
            let layout = text::Layout::new(&text, size, width, spacing, extra, bold)?;
            let y = if baseline { y - layout.ascent() } else { y };
            let mut matrix = affine(matrix);
            // The layout's own origin, through the canvas's matrix.
            matrix.e += matrix.a * f64::from(x) + matrix.c * f64::from(y);
            matrix.f += matrix.b * f64::from(x) + matrix.d * f64::from(y);
            layout.draw(bitmaps.image_mut(id)?, matrix, color);
            json!(null)
        }
    })
}

/// An `ARGB` colour through Android's `ColorMatrix`, each channel kept within 0-255.
fn color_matrix(color: u32, matrix: &[f64; 20]) -> u32 {
    let [alpha, red, green, blue] = color.to_be_bytes().map(f64::from);
    let channel = |row: usize| {
        let at = row * 5;
        let value = matrix[at] * red
            + matrix[at + 1] * green
            + matrix[at + 2] * blue
            + matrix[at + 3] * alpha
            + matrix[at + 4];
        value.round().clamp(0.0, 255.0) as u8
    };
    u32::from_be_bytes([channel(3), channel(0), channel(1), channel(2)])
}

fn envelope(result: Result<serde_json::Value, String>) -> String {
    match result {
        Ok(value) => json!({ "value": value }).to_string(),
        Err(error) => json!({ "error": error }).to_string(),
    }
}

/// Installs the picture natives on `native` (`__rgMihon`).
pub(crate) fn install<'js>(ctx: &Ctx<'js>, native: &Object<'js>) -> rquickjs::Result<()> {
    let bitmaps = Rc::new(RefCell::new(Bitmaps::default()));
    // PDF pages drawn into those pictures (`PdfRenderer`): open a document, draw a page, close.
    let pdfs = Rc::new(RefCell::new(super::pdf::Pdfs::default()));
    let held = pdfs.clone();
    native.set(
        "pdfOpen",
        Function::new(ctx.clone(), move |bytes: TypedArray<'js, u8>| {
            // SAFETY: the bytes are copied before any script runs again.
            let data = unsafe { bytes.as_bytes() }.map(<[u8]>::to_vec);
            envelope(
                data.ok_or_else(|| "the bytes were let go".to_owned())
                    .and_then(|data| held.borrow_mut().open(data)),
            )
        })?,
    )?;
    let (held, held_bitmaps) = (pdfs.clone(), bitmaps.clone());
    native.set(
        "pdfRender",
        Function::new(ctx.clone(), move |id: u32, page: u32, bitmap: u32| {
            envelope(
                held.borrow()
                    .render(id, page as usize, &mut held_bitmaps.borrow_mut(), bitmap)
                    .map(|()| json!(null)),
            )
        })?,
    )?;
    let held = pdfs;
    native.set(
        "pdfClose",
        Function::new(ctx.clone(), move |id: u32| {
            held.borrow_mut().close(id);
        })?,
    )?;
    let held = bitmaps.clone();
    native.set(
        "bitmap",
        Function::new(ctx.clone(), move |op: String| {
            let result = serde_json::from_str::<Op>(&op)
                .map_err(|error| format!("invalid picture call: {error}"))
                .and_then(|op| run(&mut held.borrow_mut(), op));
            envelope(result)
        })?,
    )?;
    // `BitmapFactory.decode…`: the picture's number and size, or why it couldn't be read.
    let held = bitmaps.clone();
    native.set(
        "bitmapDecode",
        Function::new(ctx.clone(), move |bytes: TypedArray<'js, u8>| {
            // SAFETY: the bytes are copied before any script runs again.
            let data = unsafe { bytes.as_bytes() }.map(<[u8]>::to_vec);
            let result = data
                .ok_or_else(|| "the bytes were let go".to_owned())
                .and_then(|data| {
                    let mut bitmaps = held.borrow_mut();
                    let id = bitmaps.decode(&data)?;
                    let (width, height) = bitmaps.size(id)?;
                    Ok(json!({ "id": id, "width": width, "height": height }))
                });
            envelope(result)
        })?,
    )?;
    // `bitmap.compress(format, quality, stream)`: the file's bytes, or why not (a string).
    let held = bitmaps;
    native.set(
        "bitmapEncode",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'js>,
                  id: u32,
                  format: String,
                  quality: f64|
                  -> rquickjs::Result<Value<'js>> {
                match held
                    .borrow()
                    .encode(id, &format, quality.clamp(0.0, 100.0) as u8)
                {
                    Ok(bytes) => Ok(TypedArray::<u8>::new(ctx, bytes)?.into_value()),
                    Err(error) => error.into_js(&ctx),
                }
            },
        )?,
    )?;
    // Bytes as base64 and back, for bodies sent and pictures handed to Raygun.
    native.set(
        "base64Encode",
        Function::new(ctx.clone(), |bytes: TypedArray<'js, u8>| -> String {
            // SAFETY: encoded before any script runs again.
            unsafe { bytes.as_bytes() }
                .map(|data| base64::engine::general_purpose::STANDARD.encode(data))
                .unwrap_or_default()
        })?,
    )?;
    native.set(
        "base64Decode",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, text: String| -> rquickjs::Result<Value<'js>> {
                let clean: String = text
                    .chars()
                    .filter(|char| !char.is_whitespace())
                    .map(|char| match char {
                        '-' => '+',
                        '_' => '/',
                        other => other,
                    })
                    .collect();
                let trimmed = clean.trim_end_matches('=');
                match base64::engine::general_purpose::STANDARD_NO_PAD.decode(trimmed) {
                    Ok(bytes) => Ok(TypedArray::<u8>::new(ctx, bytes)?.into_value()),
                    Err(error) => format!("not base64: {error}").into_js(&ctx),
                }
            },
        )?,
    )?;
    // UTF-8 bytes of text and text of UTF-8 bytes (a bad byte reads as U+FFFD, as Java's).
    native.set(
        "utf8Encode",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, text: String| -> rquickjs::Result<Value<'js>> {
                Ok(TypedArray::<u8>::new(ctx, text.into_bytes())?.into_value())
            },
        )?,
    )?;
    native.set(
        "utf8Decode",
        Function::new(ctx.clone(), |bytes: TypedArray<'js, u8>| -> String {
            // SAFETY: decoded before any script runs again.
            unsafe { bytes.as_bytes() }
                .map(|data| String::from_utf8_lossy(data).into_owned())
                .unwrap_or_default()
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::color_matrix;

    #[test]
    fn color_matrices_invert_and_grey_pixels() {
        let invert = [
            -1.0, 0.0, 0.0, 0.0, 255.0, 0.0, -1.0, 0.0, 0.0, 255.0, 0.0, 0.0, -1.0, 0.0, 255.0,
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        assert_eq!(color_matrix(0xFF10_2030, &invert), 0xFFEF_DFCF);
        // Alpha comes from the fourth row; channels stay within 0-255.
        let scaled = [
            2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -100.0, 0.0, 0.0,
            0.0, 0.5, 0.0,
        ];
        assert_eq!(color_matrix(0xFFC0_4020, &scaled), 0x80FF_4000);
    }
}
