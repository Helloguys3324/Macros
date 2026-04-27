use crate::models::Roi;
use anyhow::{bail, Result};
use scrap::{Capturer, Display};
use std::io::ErrorKind;
use std::thread;
use std::time::Duration;

pub struct ScreenCapture {
    capturer: Capturer,
    width: usize,
    height: usize,
}

impl ScreenCapture {
    pub fn new_primary() -> Result<Self> {
        let display = Display::primary()?;
        let width = display.width();
        let height = display.height();
        let capturer = Capturer::new(display)?;
        Ok(Self {
            capturer,
            width,
            height,
        })
    }

    pub fn capture_roi_grayscale(&mut self, roi: Roi) -> Result<Vec<u8>> {
        if roi.w == 0 || roi.h == 0 {
            bail!("ROI is empty");
        }

        let x2 = roi.x as usize + roi.w as usize;
        let y2 = roi.y as usize + roi.h as usize;
        if x2 > self.width || y2 > self.height {
            bail!("ROI is outside screen bounds");
        }

        let frame = loop {
            match self.capturer.frame() {
                Ok(frame) => break frame,
                Err(err) if err.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(8));
                }
                Err(err) => return Err(err.into()),
            }
        };

        let mut gray = Vec::with_capacity((roi.w * roi.h) as usize);
        let stride = self.width * 4;

        for y in roi.y as usize..(roi.y + roi.h) as usize {
            for x in roi.x as usize..(roi.x + roi.w) as usize {
                let i = y * stride + x * 4;
                let b = frame[i] as f32;
                let g = frame[i + 1] as f32;
                let r = frame[i + 2] as f32;
                let luma = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
                gray.push(luma);
            }
        }

        Ok(gray)
    }
}

