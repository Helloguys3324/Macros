use anyhow::{anyhow, Context, Result};
use image::{imageops::FilterType, GrayImage};
use ndarray::{Array4, IxDyn};
use ort::{
    environment::Environment,
    session::SessionBuilder,
    tensor::OrtOwnedTensor,
    GraphOptimizationLevel,
    LoggingLevel,
};
use std::fs;
use std::sync::Arc;

pub struct OcrEngine {
    _env: Arc<Environment>,
    session: ort::session::Session,
    dict: Vec<char>,
    input_w: u32,
    input_h: u32,
}

impl OcrEngine {
    pub fn new(model_path: &str, dict_path: &str) -> Result<Self> {
        let env = Arc::new(
            Environment::builder()
                .with_name("clan-tracker-ocr")
                .with_log_level(LoggingLevel::Warning)
                .build()?,
        );

        let session = SessionBuilder::new(&env)?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_model_from_file(model_path)?;

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
            _env: env,
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

        let outputs: Vec<OrtOwnedTensor<f32, IxDyn>> = self.session.run(vec![input])?;
        let logits = outputs
            .get(0)
            .context("OCR model returned no outputs")?
            .view();
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
    logits: &ndarray::ArrayViewD<'_, f32>,
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
    logits: &ndarray::ArrayViewD<'_, f32>,
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

