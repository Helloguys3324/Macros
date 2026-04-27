use anyhow::{bail, Result};

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::thread;
    use std::time::Duration;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_BACK, VK_CONTROL, VK_RETURN};
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetWindowRect, PostMessageW, SetForegroundWindow, WM_CHAR, WM_KEYDOWN,
        WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    };

    pub struct BackgroundInput {
        hwnd: HWND,
    }

    impl BackgroundInput {
        pub fn connect(window_title: &str) -> Result<Self> {
            let wide: Vec<u16> = OsStr::new(window_title)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let hwnd = unsafe { FindWindowW(None, PCWSTR(wide.as_ptr()))? };
            if hwnd.0.is_null() {
                bail!("Game window not found by exact title: {}", window_title);
            }
            Ok(Self { hwnd })
        }

        pub fn click_search_field(&self, x: i32, y: i32) -> Result<()> {
            unsafe {
                SetForegroundWindow(self.hwnd);
            }
            let (cx, cy) = self.screen_to_client(x, y)?;
            let lp = make_mouse_lparam(cx, cy);

            for _ in 0..10 {
                self.post(WM_MOUSEMOVE, WPARAM(0), lp)?;
                self.post(WM_LBUTTONDOWN, WPARAM(1), lp)?;
                thread::sleep(Duration::from_millis(10));
                self.post(WM_LBUTTONUP, WPARAM(0), lp)?;
                thread::sleep(Duration::from_millis(20));
            }
            Ok(())
        }

        pub fn clear_search_field(&self) -> Result<()> {
            unsafe {
                SetForegroundWindow(self.hwnd);
            }
            self.key_down(VK_CONTROL.0 as u16)?;
            thread::sleep(Duration::from_millis(20));
            self.key_down(0x41)?;
            thread::sleep(Duration::from_millis(20));
            self.key_up(0x41)?;
            thread::sleep(Duration::from_millis(20));
            self.key_up(VK_CONTROL.0 as u16)?;
            thread::sleep(Duration::from_millis(20));
            self.key_down(VK_BACK.0 as u16)?;
            thread::sleep(Duration::from_millis(20));
            self.key_up(VK_BACK.0 as u16)?;
            Ok(())
        }

        pub fn type_text(&self, text: &str) -> Result<()> {
            unsafe {
                SetForegroundWindow(self.hwnd);
            }
            for ch in text.encode_utf16() {
                self.post(WM_CHAR, WPARAM(ch as usize), LPARAM(1))?;
                thread::sleep(Duration::from_millis(10));
            }
            Ok(())
        }

        pub fn press_enter(&self) -> Result<()> {
            unsafe {
                SetForegroundWindow(self.hwnd);
            }
            for _ in 0..3 {
                self.key_down(VK_RETURN.0 as u16)?;
                thread::sleep(Duration::from_millis(20));
                self.key_up(VK_RETURN.0 as u16)?;
                thread::sleep(Duration::from_millis(20));
            }
            Ok(())
        }

        fn key_down(&self, vk: u16) -> Result<()> {
            self.post(WM_KEYDOWN, WPARAM(vk as usize), LPARAM(1))
        }

        fn key_up(&self, vk: u16) -> Result<()> {
            self.post(WM_KEYUP, WPARAM(vk as usize), LPARAM(0xC000_0001))
        }

        fn post(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Result<()> {
            unsafe { PostMessageW(self.hwnd, msg, wparam, lparam)? };
            Ok(())
        }

        fn screen_to_client(&self, x: i32, y: i32) -> Result<(i32, i32)> {
            let mut rect = RECT::default();
            unsafe { GetWindowRect(self.hwnd, &mut rect)? };
            Ok((x - rect.left, y - rect.top))
        }
    }

    fn make_mouse_lparam(x: i32, y: i32) -> LPARAM {
        let xw = (x as u32) & 0xFFFF;
        let yw = (y as u32) & 0xFFFF;
        LPARAM(((yw << 16) | xw) as isize)
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::*;

    pub struct BackgroundInput;

    impl BackgroundInput {
        pub fn connect(_window_title: &str) -> Result<Self> {
            bail!("Background window automation is only supported on Windows")
        }

        pub fn click_search_field(&self, _x: i32, _y: i32) -> Result<()> {
            bail!("Not supported on this platform")
        }

        pub fn clear_search_field(&self) -> Result<()> {
            bail!("Not supported on this platform")
        }

        pub fn type_text(&self, _text: &str) -> Result<()> {
            bail!("Not supported on this platform")
        }

        pub fn press_enter(&self) -> Result<()> {
            bail!("Not supported on this platform")
        }
    }
}

pub use imp::BackgroundInput;
