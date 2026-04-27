use anyhow::{bail, Result};

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use std::ffi::OsStr;
    use std::mem::size_of;
    use std::os::windows::ffi::OsStrExt;
    use std::thread;
    use std::time::Duration;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        KEYEVENTF_UNICODE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
        MOUSEEVENTF_MOVE, MOUSEEVENTF_VIRTUALDESK, VK_BACK, VK_CONTROL, VK_RETURN,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetSystemMetrics, SetForegroundWindow, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
        SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
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
            use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;

            unsafe {
                SetForegroundWindow(self.hwnd);
            }
            thread::sleep(Duration::from_millis(150));

            unsafe {
                if !SetCursorPos(x, y).as_bool() {
                    bail!("SetCursorPos failed");
                }
            }
            thread::sleep(Duration::from_millis(100));

            for _ in 0..2 {
                self.send_mouse_input(0, 0, MOUSEEVENTF_LEFTDOWN)?;
                thread::sleep(Duration::from_millis(50));
                self.send_mouse_input(0, 0, MOUSEEVENTF_LEFTUP)?;
                thread::sleep(Duration::from_millis(50));
            }

            Ok(())
        }

        pub fn clear_search_field(&self) -> Result<()> {
            unsafe {
                SetForegroundWindow(self.hwnd);
            }
            let empty_flags = KEYBD_EVENT_FLAGS(0);
            self.send_key_input(VK_CONTROL.0, empty_flags)?;
            thread::sleep(Duration::from_millis(20));
            self.send_key_input(0x41, empty_flags)?; // 'A'
            thread::sleep(Duration::from_millis(20));
            self.send_key_input(0x41, KEYEVENTF_KEYUP)?;
            thread::sleep(Duration::from_millis(20));
            self.send_key_input(VK_CONTROL.0, KEYEVENTF_KEYUP)?;
            thread::sleep(Duration::from_millis(20));
            self.send_key_input(VK_BACK.0, empty_flags)?;
            thread::sleep(Duration::from_millis(20));
            self.send_key_input(VK_BACK.0, KEYEVENTF_KEYUP)?;
            Ok(())
        }

        pub fn type_text(&self, text: &str) -> Result<()> {
            unsafe {
                SetForegroundWindow(self.hwnd);
            }
            let empty_flags = KEYBD_EVENT_FLAGS(0);
            for ch in text.encode_utf16() {
                self.send_unicode_char(ch, empty_flags)?;
                thread::sleep(Duration::from_millis(15));
                self.send_unicode_char(ch, KEYEVENTF_KEYUP)?;
                thread::sleep(Duration::from_millis(15));
            }
            Ok(())
        }

        pub fn press_enter(&self) -> Result<()> {
            unsafe {
                SetForegroundWindow(self.hwnd);
            }
            let empty_flags = KEYBD_EVENT_FLAGS(0);
            for _ in 0..2 {
                self.send_key_input(VK_RETURN.0, empty_flags)?;
                thread::sleep(Duration::from_millis(30));
                self.send_key_input(VK_RETURN.0, KEYEVENTF_KEYUP)?;
                thread::sleep(Duration::from_millis(30));
            }
            Ok(())
        }

        fn send_mouse_input(
            &self,
            dx: i32,
            dy: i32,
            flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
        ) -> Result<()> {
            let mut input = INPUT::default();
            input.r#type = INPUT_MOUSE;
            input.Anonymous.mi.dx = dx;
            input.Anonymous.mi.dy = dy;
            input.Anonymous.mi.dwFlags = flags;

            unsafe {
                if SendInput(&[input], size_of::<INPUT>() as i32) != 1 {
                    bail!("SendInput failed for mouse");
                }
            }
            Ok(())
        }

        fn send_key_input(&self, vk: u16, flags: KEYBD_EVENT_FLAGS) -> Result<()> {
            let mut input = INPUT::default();
            input.r#type = INPUT_KEYBOARD;
            input.Anonymous.ki.wVk = windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk);
            input.Anonymous.ki.dwFlags = flags;

            unsafe {
                if SendInput(&[input], size_of::<INPUT>() as i32) != 1 {
                    bail!("SendInput failed for key {}", vk);
                }
            }
            Ok(())
        }

        fn send_unicode_char(&self, ch: u16, flags: KEYBD_EVENT_FLAGS) -> Result<()> {
            let mut input = INPUT::default();
            input.r#type = INPUT_KEYBOARD;
            input.Anonymous.ki.wScan = ch;
            input.Anonymous.ki.dwFlags = flags | KEYEVENTF_UNICODE;

            unsafe {
                if SendInput(&[input], size_of::<INPUT>() as i32) != 1 {
                    bail!("SendInput failed for unicode char {}", ch);
                }
            }
            Ok(())
        }
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
