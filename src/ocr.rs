use anyhow::{anyhow, Context, Result};
use image::{imageops::FilterType, GrayImage};
use ndarray::{Array4, ArrayViewD};
use ort::{
    session::{builder::GraphOptimizationLevel, Session},
    value::TensorRef,
};
use std::fs;

pub struct OcrEngine {
    session: Session,
    dict: Vec<char>,
    input_w: u32,
    input_h: u32,
}

impl OcrEngine {
    pub fn new(model_path: &str, dict_path: &str) -> Result<Self> {
        let builder = Session::builder().map_err(|e| anyhow!(e.to_string()))?;
        let mut builder = builder
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow!(e.to_string()))?;
        let session = builder
            .commit_from_file(model_path)
            .map_err(|e| anyhow!(e.to_string()))?;

        let dict_raw = fs::read_to_string(dict_path)
            .with_context(|| format!("Failed to read OCR dictionary: {}", dict_path))?;
        let dict = dict_raw
            .lines()
            .filter_map(|line| line.chars().next())
            .collect::<Vec<_>>();

        if dict.is_empty() {
            return Err(anyhow!("OCR dictionary is empty: {}", dict_path));
        }

        Ok(Self {
            session,
            dict,
            input_w: 320,
            input_h: 48,
        })
    }

    pub fn read_points(
        &mut self,
        gray_roi: &[u8],
        roi_w: u32,
        roi_h: u32,
        threshold: u8,
    ) -> Result<Option<u32>> {
        let image = GrayImage::from_raw(roi_w, roi_h, gray_roi.to_vec())
            .ok_or_else(|| anyhow!("Invalid grayscale ROI buffer"))?;

        // Save debug image of what was captured directly from screen
        let _ = image.save("debug_raw_roi.png");

        // Calculate padding to preserve aspect ratio
        let target_h = self.input_h;
        let target_w = self.input_w;

        // Scale to 48px height, preserve width
        let scale = target_h as f32 / image.height() as f32;
        let scaled_w = (image.width() as f32 * scale).round() as u32;

        let resized =
            image::imageops::resize(&image, scaled_w, target_h, FilterType::Triangle);

        let mut debug_bin = GrayImage::new(target_w, target_h);

        let mut input = Array4::<f32>::zeros((1, 3, target_h as usize, target_w as usize));

        for y in 0..target_h {
            for x in 0..target_w {
                let px = if x < scaled_w {
                    resized.get_pixel(x, y).0[0]
                } else {
                    // Fill padding with background color (dark) so it becomes white after binarization
                    0
                };

                // Use user-defined threshold. Text is bright green, background is dark.
                // We want the bright text to become black (0.0) and dark background to become white (255.0).
                let binarized = if px >= threshold { 0.0 } else { 255.0 };

                debug_bin.put_pixel(x, y, image::Luma([binarized as u8]));

                // Normalize to [-1.0, 1.0]
                let norm = (binarized / 255.0 - 0.5) / 0.5;

                for ch in 0..3 {
                    input[[0, ch, y as usize, x as usize]] = norm;
                }
            }
        }

        // Save debug image of what the OCR model is actually seeing
        let _ = debug_bin.save("debug_binarized_roi.png");

        let input_tensor =
            TensorRef::from_array_view(input.view()).map_err(|e| anyhow!(e.to_string()))?;
        let outputs = self
            .session
            .run(ort::inputs![input_tensor])
            .map_err(|e| anyhow!(e.to_string()))?;
        let logits = outputs[0]
            .try_extract_array::<f32>()
            .context("Failed to extract OCR output tensor as f32 array")?;
        let shape = logits.shape().to_vec();

        let decoded = match shape.as_slice() {
            [1, t, c] => decode_ctc_3d(&logits, *t, *c, &self.dict),
            [t, c] => decode_ctc_2d(&logits, *t, *c, &self.dict),
            _ => return Err(anyhow!("Unexpected OCR output shape: {:?}", shape)),
        };

        Ok(parse_points(&decoded))
    }
}

fn decode_ctc_3d(
    logits: &ArrayViewD<'_, f32>,
    timesteps: usize,
    classes: usize,
    dict: &[char],
) -> String {
    let mut out = String::new();
    let mut prev_idx = 0usize;

    for t in 0..timesteps {
        let mut best_idx = 0usize;
        let mut best_score = f32::MIN;

        for c in 0..classes {
            let score = logits[[0, t, c]];
            if score > best_score {
                best_score = score;
                best_idx = c;
            }
        }

        if best_idx != 0 && best_idx != prev_idx {
            if let Some(ch) = dict.get(best_idx - 1) {
                out.push(*ch);
            }
        }
        prev_idx = best_idx;
    }

    out
}

fn decode_ctc_2d(
    logits: &ArrayViewD<'_, f32>,
    timesteps: usize,
    classes: usize,
    dict: &[char],
) -> String {
    let mut out = String::new();
    let mut prev_idx = 0usize;

    for t in 0..timesteps {
        let mut best_idx = 0usize;
        let mut best_score = f32::MIN;

        for c in 0..classes {
            let score = logits[[t, c]];
            if score > best_score {
                best_score = score;
                best_idx = c;
            }
        }

        if best_idx != 0 && best_idx != prev_idx {
            if let Some(ch) = dict.get(best_idx - 1) {
                out.push(*ch);
            }
        }
        prev_idx = best_idx;
    }

    out
}

fn parse_points(s: &str) -> Option<u32> {
    let mut cleaned = String::new();
    let mut multiplier = 1f64;

    for mut c in s.chars() {
        match c {
            'O' | 'o' => c = '0',
            'l' | 'I' => c = '1',
            _ => {}
        }

        if c.is_ascii_digit() || c == '.' {
            cleaned.push(c);
        } else if c == ',' {
            // skip
        } else if c.eq_ignore_ascii_case(&'k') {
            multiplier = 1_000.0;
        } else if c.eq_ignore_ascii_case(&'m') {
            multiplier = 1_000_000.0;
        } else if c.eq_ignore_ascii_case(&'b') {
            multiplier = 1_000_000_000.0;
        }
    }

    if cleaned.is_empty() || cleaned == "." {
        return None;
    }

    if let Ok(val) = cleaned.parse::<f64>() {
        let total = val * multiplier;
        Some(total as u32)
    } else {
        None
    }
}
