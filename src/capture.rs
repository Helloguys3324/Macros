use crate::models::Roi;
use anyhow::{anyhow, bail, Result};
use scrap::{Capturer, Display};
use std::io::ErrorKind;
use std::thread;
use std::time::Duration;

pub struct ScreenCapture {
    backend: Backend,
}

enum Backend {
    Scrap {
        capturer: Capturer,
        width: usize,
        height: usize,
    },
    #[cfg(target_os = "windows")]
    Gdi {
        width: usize,
        height: usize,
    },
}

impl ScreenCapture {
    pub fn new_primary() -> Result<Self> {
        if let Ok(display) = Display::primary() {
            let width = display.width();
            let height = display.height();
            if let Ok(capturer) = Capturer::new(display) {
                return Ok(Self {
                    backend: Backend::Scrap {
                        capturer,
                        width,
                        height,
                    },
                });
            }
        }

        #[cfg(target_os = "windows")]
        {
            let (width, height) = gdi_screen_size()?;
            return Ok(Self {
                backend: Backend::Gdi { width, height },
            });
        }

        #[allow(unreachable_code)]
        Err(anyhow!("No screen capture backend available"))
    }

    pub fn backend_name(&self) -> &'static str {
        match self.backend {
            Backend::Scrap { .. } => "scrap",
            #[cfg(target_os = "windows")]
            Backend::Gdi { .. } => "gdi-fallback",
        }
    }

    pub fn capture_roi_grayscale(&mut self, roi: Roi) -> Result<Vec<u8>> {
        let result = match &mut self.backend {
            Backend::Scrap {
                capturer,
                width,
                height,
            } => capture_with_scrap(capturer, *width, *height, roi),
            #[cfg(target_os = "windows")]
            Backend::Gdi { width, height } => capture_with_gdi(*width, *height, roi),
        };

        #[cfg(target_os = "windows")]
        if result.is_err() {
            let can_switch = matches!(self.backend, Backend::Scrap { .. });
            if can_switch {
                let (width, height) = gdi_screen_size()?;
                self.backend = Backend::Gdi { width, height };
                return capture_with_gdi(width, height, roi);
            }
        }

        result
    }
}

fn validate_roi(roi: Roi, width: usize, height: usize) -> Result<()> {
    if roi.w == 0 || roi.h == 0 {
        bail!("ROI is empty");
    }

    let x2 = roi.x as usize + roi.w as usize;
    let y2 = roi.y as usize + roi.h as usize;
    if x2 > width || y2 > height {
        bail!("ROI is outside screen bounds");
    }
    Ok(())
}

fn capture_with_scrap(
    capturer: &mut Capturer,
    width: usize,
    height: usize,
    roi: Roi,
) -> Result<Vec<u8>> {
    validate_roi(roi, width, height)?;

    let frame = loop {
        match capturer.frame() {
            Ok(frame) => break frame,
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(8));
            }
            Err(err) => return Err(err.into()),
        }
    };

    let mut gray = Vec::with_capacity((roi.w * roi.h) as usize);
    let stride = width * 4;

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

#[cfg(target_os = "windows")]
fn gdi_screen_size() -> Result<(usize, usize)> {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    let w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    if w <= 0 || h <= 0 {
        bail!("Failed to read screen size from GDI")
    }
    Ok((w as usize, h as usize))
}

#[cfg(target_os = "windows")]
fn capture_with_gdi(width: usize, height: usize, roi: Roi) -> Result<Vec<u8>> {
    use std::ffi::c_void;
    use std::mem::size_of;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        SRCCOPY,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;

    validate_roi(roi, width, height)?;

    let w = roi.w as i32;
    let h = roi.h as i32;

    unsafe {
        let hwnd = GetDesktopWindow();
        let hdc_screen = GetDC(hwnd);
        if hdc_screen.0.is_null() {
            bail!("GetDC failed");
        }

        let hdc_mem = CreateCompatibleDC(hdc_screen);
        if hdc_mem.0.is_null() {
            let _ = ReleaseDC(hwnd, hdc_screen);
            bail!("CreateCompatibleDC failed");
        }

        let hbitmap = CreateCompatibleBitmap(hdc_screen, w, h);
        if hbitmap.0.is_null() {
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(hwnd, hdc_screen);
            bail!("CreateCompatibleBitmap failed");
        }

        let old_obj = SelectObject(hdc_mem, hbitmap);
        BitBlt(
            hdc_mem,
            0,
            0,
            w,
            h,
            hdc_screen,
            roi.x as i32,
            roi.y as i32,
            SRCCOPY,
        )?;

        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };

        let mut bgra = vec![0u8; (roi.w * roi.h * 4) as usize];
        let got = GetDIBits(
            hdc_mem,
            hbitmap,
            0,
            roi.h,
            Some(bgra.as_mut_ptr() as *mut c_void),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        let _ = SelectObject(hdc_mem, old_obj);
        let _ = DeleteObject(hbitmap);
        let _ = DeleteDC(hdc_mem);
        let _ = ReleaseDC(hwnd, hdc_screen);

        if got == 0 {
            bail!("GetDIBits failed");
        }

        let mut gray = Vec::with_capacity((roi.w * roi.h) as usize);
        for i in (0..bgra.len()).step_by(4) {
            let b = bgra[i] as f32;
            let g = bgra[i + 1] as f32;
            let r = bgra[i + 2] as f32;
            gray.push((0.299 * r + 0.587 * g + 0.114 * b) as u8);
        }
        Ok(gray)
    }
}

