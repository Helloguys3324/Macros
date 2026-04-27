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
        let builder = builder
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

        let resized = image::imageops::resize(
            &image,
            self.input_w,
            self.input_h,
            FilterType::Triangle,
        );

        let mut input = Array4::<f32>::zeros((
            1,
            3,
            self.input_h as usize,
            self.input_w as usize,
        ));

        for y in 0..self.input_h {
            for x in 0..self.input_w {
                let px = resized.get_pixel(x, y).0[0];
                let bw = if px >= threshold { 255.0 } else { 0.0 };
                let norm = (bw / 255.0 - 0.5) / 0.5;
                for ch in 0..3 {
                    input[[0, ch, y as usize, x as usize]] = norm;
                }
            }
        }

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

        let digits: String = decoded.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return Ok(None);
        }

        Ok(digits.parse::<u32>().ok())
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

