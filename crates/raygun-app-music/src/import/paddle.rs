//! PaddleOCR's PP-OCRv5 mobile models (Apache-2.0, the ONNX exports RapidOCR publishes), run
//! on the device by rten as ocrs's are: a DB text detector finds the lines, a CTC recognizer
//! (SVTR-LCNet, Chinese, Japanese, English and the Latin alphabet) reads each one. The catalogue
//! model `ppocr-v5-mobile` holds `det.onnx`, `rec.onnx` and the recognizer's `dict.txt`.

use std::path::Path;

use rten::Model;
use rten_tensor::NdTensor;
use rten_tensor::prelude::*;

use super::ocr::ReadLine;

/// The catalogue's PaddleOCR models.
pub const PADDLE_MODEL: &str = "ppocr-v5-mobile";

/// The detector sees pictures at most this long on their longer side (sides are multiples of 32).
const DETECT_MAX_SIDE: u32 = 1920;
/// A pixel is text when the detector's probability passes this.
const PIXEL_THRESHOLD: f32 = 0.3;
/// A found region is kept when its pixels' mean probability passes this.
const BOX_THRESHOLD: f32 = 0.6;
/// How far a region's box grows (DB shrinks text regions while training; this undoes it).
const UNCLIP_RATIO: f32 = 1.5;
/// The share of an unclipped box its letters fill: the box is padded above and below them, and the
/// layout compares line heights and gaps as the other readers report them (the letters').
const GLYPH_SHARE: f32 = 0.75;
/// Regions smaller than this many pixels (in the detector's picture) are specks.
const MIN_REGION: usize = 12;
/// A box: left, top, right, bottom.
type Bounds = [f32; 4];
/// The recognizer reads lines at this height.
const LINE_HEIGHT: u32 = 48;
/// Lines wider than this (after scaling to `LINE_HEIGHT`) are squeezed to it.
const LINE_MAX_WIDTH: u32 = 3200;

/// The loaded models and the recognizer's characters.
pub struct Paddle {
    detector: Model,
    recognizer: Model,
    characters: Vec<String>,
}

impl Paddle {
    /// Loads the models from an installed `ppocr-v5-mobile` folder. Each file is read whole into
    /// memory, so a reader started after an install never holds an older file.
    pub fn load(folder: &Path) -> Result<Self, String> {
        let failed =
            |error: &dyn std::fmt::Display| format!("The text reader couldn't start ({error})");
        let detector = Model::load_file(folder.join("det.onnx")).map_err(|e| failed(&e))?;
        let recognizer = Model::load_file(folder.join("rec.onnx")).map_err(|e| failed(&e))?;
        let dict = std::fs::read_to_string(folder.join("dict.txt")).map_err(|e| failed(&e))?;
        Ok(Self {
            detector,
            recognizer,
            characters: characters(&dict),
        })
    }

    /// The text lines of a grey picture with where each is, in reading order.
    pub(crate) fn read(&self, picture: &image::GrayImage) -> Result<Vec<ReadLine>, String> {
        let regions = self.detect(picture)?;
        let mut lines = Vec::with_capacity(regions.len());
        // The detector finds text shrunk (as DB is trained); the grown box is the text's own.
        for (_, grown) in regions {
            let text = self.recognize(picture, &grown)?;
            let text = text.trim().to_owned();
            if text.is_empty() {
                continue;
            }
            lines.push(ReadLine {
                x: grown[0],
                y: grown[1],
                height: (grown[3] - grown[1]) * GLYPH_SHARE,
                bounds: grown,
                text,
            });
        }
        lines.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
        Ok(without_marks(lines))
    }

    /// The boxes (left, top, right, bottom) around the picture's text lines, in its pixels: where
    /// the text is, and the larger box it's read in.
    fn detect(&self, picture: &image::GrayImage) -> Result<Vec<(Bounds, Bounds)>, String> {
        let (width, height) = picture.dimensions();
        let (in_width, in_height) = detect_size(width, height);
        let resized = image::imageops::resize(
            picture,
            in_width,
            in_height,
            image::imageops::FilterType::Triangle,
        );
        // ImageNet's normalization, the same on every channel of a grey picture.
        const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
        const STD: [f32; 3] = [0.229, 0.224, 0.225];
        let plane = (in_width * in_height) as usize;
        let mut data = vec![0.0_f32; 3 * plane];
        for (at, pixel) in resized.pixels().enumerate() {
            let value = f32::from(pixel.0[0]) / 255.0;
            for channel in 0..3 {
                data[channel * plane + at] = (value - MEAN[channel]) / STD[channel];
            }
        }
        let input = NdTensor::from_data([1, 3, in_height as usize, in_width as usize], data);
        let output: NdTensor<f32, 4> = self
            .detector
            .run_one(input.into(), None)
            .map_err(|error| format!("The text reader failed ({error})"))?
            .try_into()
            .map_err(|error| format!("The text reader failed ({error:?})"))?;
        let [_, _, map_height, map_width] = output.shape();
        let probabilities = output.to_vec();
        let scale_x = width as f32 / map_width as f32;
        let scale_y = height as f32 / map_height as f32;
        let scaled = |[left, top, right, bottom]: [f32; 4]| {
            [
                (left * scale_x).clamp(0.0, width as f32),
                (top * scale_y).clamp(0.0, height as f32),
                (right * scale_x).clamp(0.0, width as f32),
                (bottom * scale_y).clamp(0.0, height as f32),
            ]
        };
        Ok(regions(&probabilities, map_width, map_height)
            .into_iter()
            .map(|[left, top, right, bottom]| {
                let grow = unclip(right - left, bottom - top);
                (
                    scaled([left, top, right, bottom]),
                    scaled([left - grow, top - grow, right + grow, bottom + grow]),
                )
            })
            .filter(|(_, [left, top, right, bottom])| right - left >= 3.0 && bottom - top >= 3.0)
            .collect())
    }

    /// Reads the line inside `region`.
    fn recognize(&self, picture: &image::GrayImage, region: &[f32; 4]) -> Result<String, String> {
        let [left, top, right, bottom] = *region;
        let (left, top) = (left.floor() as u32, top.floor() as u32);
        let crop_width = (right.ceil() as u32).saturating_sub(left).max(1);
        let crop_height = (bottom.ceil() as u32).saturating_sub(top).max(1);
        let crop =
            image::imageops::crop_imm(picture, left, top, crop_width, crop_height).to_image();
        let line_width = ((crop_width as f32 * LINE_HEIGHT as f32 / crop_height as f32).ceil()
            as u32)
            .clamp(8, LINE_MAX_WIDTH);
        let line = image::imageops::resize(
            &crop,
            line_width,
            LINE_HEIGHT,
            image::imageops::FilterType::Triangle,
        );
        let plane = (line_width * LINE_HEIGHT) as usize;
        let mut data = vec![0.0_f32; 3 * plane];
        for (at, pixel) in line.pixels().enumerate() {
            let value = f32::from(pixel.0[0]) / 255.0 * 2.0 - 1.0;
            for channel in 0..3 {
                data[channel * plane + at] = value;
            }
        }
        let input = NdTensor::from_data([1, 3, LINE_HEIGHT as usize, line_width as usize], data);
        let output: NdTensor<f32, 3> = self
            .recognizer
            .run_one(input.into(), None)
            .map_err(|error| format!("The text reader failed ({error})"))?
            .try_into()
            .map_err(|error| format!("The text reader failed ({error:?})"))?;
        let [_, steps, classes] = output.shape();
        Ok(ctc_decode(
            &output.to_vec(),
            steps,
            classes,
            &self.characters,
        ))
    }
}

/// The recognizer's classes after its blank: the dictionary's lines, then a space.
fn characters(dict: &str) -> Vec<String> {
    let mut characters: Vec<String> = dict
        .trim_start_matches('\u{feff}')
        .lines()
        .map(|line| line.trim_end_matches('\r').to_owned())
        .collect();
    while characters.last().is_some_and(String::is_empty) {
        characters.pop();
    }
    characters.push(" ".to_owned());
    characters
}

/// The detector's input size for a picture: at most `DETECT_MAX_SIDE` on the longer side, each
/// side a multiple of 32 (at least 32).
fn detect_size(width: u32, height: u32) -> (u32, u32) {
    let longer = width.max(height).max(1) as f32;
    let scale = (DETECT_MAX_SIDE as f32 / longer).min(1.0);
    let round = |side: u32| (((side as f32 * scale) / 32.0).round() as u32).max(1) * 32;
    (round(width), round(height))
}

/// Greedy CTC decoding: the best class per step, repeats and blanks (class 0) dropped.
pub(crate) fn ctc_decode(
    probabilities: &[f32],
    steps: usize,
    classes: usize,
    characters: &[String],
) -> String {
    let mut text = String::new();
    let mut previous = 0;
    for step in 0..steps {
        let row = &probabilities[step * classes..(step + 1) * classes];
        let best = row
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map_or(0, |(class, _)| class);
        if best != 0
            && best != previous
            && let Some(character) = characters.get(best - 1)
        {
            text.push_str(character);
        }
        previous = best;
    }
    text
}

/// How far DB's unclip grows a box (its area times the ratio over its perimeter).
fn unclip(width: f32, height: f32) -> f32 {
    width * height * UNCLIP_RATIO / (2.0 * (width + height))
}

/// Lines without the marks beside a list's rows that read as stray characters (a menu's dots, a
/// download arrow): a line of one or two characters much smaller than the picture's lines.
fn without_marks(lines: Vec<ReadLine>) -> Vec<ReadLine> {
    let mut heights: Vec<f32> = lines.iter().map(|line| line.height).collect();
    heights.sort_by(f32::total_cmp);
    let Some(median) = heights.get(heights.len() / 2).copied() else {
        return lines;
    };
    lines
        .into_iter()
        .filter(|line| line.text.chars().count() > 2 || line.height >= median * 0.5)
        .collect()
}

/// The boxes (left, top, right, bottom, in the map's pixels) around the probability map's
/// regions of text: connected pixels above `PIXEL_THRESHOLD` whose mean passes `BOX_THRESHOLD`.
pub(crate) fn regions(probabilities: &[f32], width: usize, height: usize) -> Vec<[f32; 4]> {
    let mut seen = vec![false; width * height];
    let mut found = Vec::new();
    let mut stack = Vec::new();
    for start in 0..width * height {
        if seen[start] || probabilities[start] <= PIXEL_THRESHOLD {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
        let (mut count, mut sum) = (0usize, 0.0_f32);
        while let Some(at) = stack.pop() {
            let (x, y) = (at % width, at / width);
            left = left.min(x);
            right = right.max(x);
            top = top.min(y);
            bottom = bottom.max(y);
            count += 1;
            sum += probabilities[at];
            let mut visit = |next: usize| {
                if !seen[next] && probabilities[next] > PIXEL_THRESHOLD {
                    seen[next] = true;
                    stack.push(next);
                }
            };
            if x > 0 {
                visit(at - 1);
            }
            if x + 1 < width {
                visit(at + 1);
            }
            if y > 0 {
                visit(at - width);
            }
            if y + 1 < height {
                visit(at + width);
            }
        }
        if count < MIN_REGION || sum / (count as f32) < BOX_THRESHOLD {
            continue;
        }
        found.push([
            left as f32,
            top as f32,
            (right + 1) as f32,
            (bottom + 1) as f32,
        ]);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctc_drops_blanks_and_repeats() {
        let characters = characters("a\nb\n");
        assert_eq!(characters, ["a", "b", " "]);
        // Steps: a, a, blank, a, b, space, b.
        let picks = [1, 1, 0, 1, 2, 3, 2];
        let classes = 4;
        let mut probabilities = vec![0.0; picks.len() * classes];
        for (step, pick) in picks.iter().enumerate() {
            probabilities[step * classes + pick] = 1.0;
        }
        assert_eq!(
            ctc_decode(&probabilities, picks.len(), classes, &characters),
            "aab b"
        );
    }

    #[test]
    fn regions_are_found() {
        let (width, height) = (20, 10);
        let mut map = vec![0.0; width * height];
        // A line of text 10 × 2 at (4, 4).
        for y in 4..6 {
            for x in 4..14 {
                map[y * width + x] = 0.9;
            }
        }
        // A faint speck that isn't kept.
        map[width + 1] = 0.4;
        let found = regions(&map, width, height);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], [4.0, 4.0, 14.0, 6.0]);
        assert!(unclip(10.0, 2.0) > 0.5);
    }

    #[test]
    fn stray_marks_are_dropped() {
        let line = |height: f32, text: &str| ReadLine {
            x: 0.0,
            y: 0.0,
            height,
            bounds: [0.0; 4],
            text: text.into(),
        };
        let kept = without_marks(vec![
            line(40.0, "Clocks"),
            line(38.0, "Coldplay"),
            line(12.0, "中"),
            line(40.0, "Up"),
        ]);
        let texts: Vec<&str> = kept.iter().map(|line| line.text.as_str()).collect();
        assert_eq!(texts, ["Clocks", "Coldplay", "Up"]);
    }

    #[test]
    fn detector_sizes_are_multiples_of_32() {
        assert_eq!(detect_size(1080, 2400), (864, 1920));
        assert_eq!(detect_size(100, 40), (96, 32));
    }
}
