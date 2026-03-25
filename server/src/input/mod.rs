pub mod keymap;

use std::fs::{File, OpenOptions};
use std::sync::Arc;

use input_linux::{
    EventKind, EventTime, InputId, Key, KeyEvent, KeyState, RelativeAxis, RelativeEvent,
    SynchronizeEvent, UInputHandle,
};
use input_linux::sys::BUS_USB;
use tokio::sync::Mutex;

/// All keys that we register with the virtual keyboard device.
/// This covers all standard keys from code 1 through 248.
const REGISTERED_KEY_RANGE: std::ops::RangeInclusive<u16> = 1..=248;

/// Virtual input device manager wrapping uinput mouse and keyboard handles.
pub struct VirtualInput {
    mouse: Mutex<UInputHandle<File>>,
    keyboard: Mutex<UInputHandle<File>>,
}

impl VirtualInput {
    /// Create a new virtual input device pair (mouse + keyboard).
    ///
    /// Opens `/dev/uinput` and configures both a relative-axis mouse device
    /// (with left/right/middle buttons and scroll wheels) and a keyboard device
    /// (with all standard key codes 1..=248).
    pub fn new() -> Result<Arc<Self>, std::io::Error> {
        let mouse = Self::create_mouse()?;
        let keyboard = Self::create_keyboard()?;

        Ok(Arc::new(Self {
            mouse: Mutex::new(mouse),
            keyboard: Mutex::new(keyboard),
        }))
    }

    fn open_uinput() -> Result<UInputHandle<File>, std::io::Error> {
        let file = OpenOptions::new()
            .write(true)
            .open("/dev/uinput")?;
        Ok(UInputHandle::new(file))
    }

    fn create_mouse() -> Result<UInputHandle<File>, std::io::Error> {
        let handle = Self::open_uinput()?;

        // Enable event types
        handle.set_evbit(EventKind::Key)?;
        handle.set_evbit(EventKind::Relative)?;
        handle.set_evbit(EventKind::Synchronize)?;

        // Mouse buttons
        handle.set_keybit(Key::ButtonLeft)?;
        handle.set_keybit(Key::ButtonRight)?;
        handle.set_keybit(Key::ButtonMiddle)?;

        // Relative axes
        handle.set_relbit(RelativeAxis::X)?;
        handle.set_relbit(RelativeAxis::Y)?;
        handle.set_relbit(RelativeAxis::Wheel)?;
        handle.set_relbit(RelativeAxis::HorizontalWheel)?;

        let id = InputId {
            bustype: BUS_USB,
            vendor: 0x1234,
            product: 0x5678,
            version: 1,
        };

        handle.create(&id, b"niri-remote-mouse", 0, &[])?;

        Ok(handle)
    }

    fn create_keyboard() -> Result<UInputHandle<File>, std::io::Error> {
        let handle = Self::open_uinput()?;

        // Enable event types
        handle.set_evbit(EventKind::Key)?;
        handle.set_evbit(EventKind::Synchronize)?;

        // Register all standard keys (codes 1 through 248)
        for code in REGISTERED_KEY_RANGE {
            if let Ok(key) = Key::from_code(code) {
                handle.set_keybit(key)?;
            }
        }

        let id = InputId {
            bustype: BUS_USB,
            vendor: 0x1234,
            product: 0x5679,
            version: 1,
        };

        handle.create(&id, b"niri-remote-keyboard", 0, &[])?;

        Ok(handle)
    }

    /// Helper: write a slice of events followed by a SYN_REPORT to the given handle.
    fn write_events(
        handle: &UInputHandle<File>,
        events: &[input_linux::sys::input_event],
    ) -> Result<(), std::io::Error> {
        handle.write(events)?;
        let syn = SynchronizeEvent::report(EventTime::new(0, 0));
        handle.write(&[syn.into_event().into_raw()])?;
        Ok(())
    }

    /// Send a relative mouse movement.
    pub async fn mouse_move(&self, dx: f64, dy: f64) {
        let mouse = self.mouse.lock().await;
        let mut events = Vec::new();

        let dx_i = dx.round() as i32;
        let dy_i = dy.round() as i32;

        if dx_i != 0 {
            let ev = RelativeEvent::new(EventTime::new(0, 0), RelativeAxis::X, dx_i);
            events.push(ev.into_event().into_raw());
        }
        if dy_i != 0 {
            let ev = RelativeEvent::new(EventTime::new(0, 0), RelativeAxis::Y, dy_i);
            events.push(ev.into_event().into_raw());
        }

        if !events.is_empty() {
            if let Err(e) = Self::write_events(&mouse, &events) {
                tracing::error!("mouse_move error: {e}");
            }
        }
    }

    /// Send a mouse button press or release.
    ///
    /// `button` should be one of `"left"`, `"right"`, or `"middle"`.
    pub async fn mouse_button(&self, button: &str, pressed: bool) {
        let key = match button {
            "left" => Key::ButtonLeft,
            "right" => Key::ButtonRight,
            "middle" => Key::ButtonMiddle,
            other => {
                tracing::warn!("unknown mouse button: {other}");
                return;
            }
        };

        let mouse = self.mouse.lock().await;
        let state = if pressed {
            KeyState::PRESSED
        } else {
            KeyState::RELEASED
        };
        let ev = KeyEvent::new(EventTime::new(0, 0), key, state);

        if let Err(e) = Self::write_events(&mouse, &[ev.into_event().into_raw()]) {
            tracing::error!("mouse_button error: {e}");
        }
    }

    /// Send a scroll event.
    ///
    /// `dy` is negated for natural scrolling (positive dy from the client
    /// scrolls down, but the wheel axis convention is inverted).
    /// `dx` is passed through for horizontal scrolling.
    pub async fn scroll(&self, dx: f64, dy: f64) {
        let mouse = self.mouse.lock().await;
        let mut events = Vec::new();

        let dy_i = -(dy.round() as i32); // negate for natural scrolling
        let dx_i = dx.round() as i32;

        if dy_i != 0 {
            let ev = RelativeEvent::new(EventTime::new(0, 0), RelativeAxis::Wheel, dy_i);
            events.push(ev.into_event().into_raw());
        }
        if dx_i != 0 {
            let ev =
                RelativeEvent::new(EventTime::new(0, 0), RelativeAxis::HorizontalWheel, dx_i);
            events.push(ev.into_event().into_raw());
        }

        if !events.is_empty() {
            if let Err(e) = Self::write_events(&mouse, &events) {
                tracing::error!("scroll error: {e}");
            }
        }
    }

    /// Send a key-down event.
    pub async fn key_press(&self, key: Key) {
        let keyboard = self.keyboard.lock().await;
        let ev = KeyEvent::new(EventTime::new(0, 0), key, KeyState::PRESSED);

        if let Err(e) = Self::write_events(&keyboard, &[ev.into_event().into_raw()]) {
            tracing::error!("key_press error: {e}");
        }
    }

    /// Send a key-up event.
    pub async fn key_release(&self, key: Key) {
        let keyboard = self.keyboard.lock().await;
        let ev = KeyEvent::new(EventTime::new(0, 0), key, KeyState::RELEASED);

        if let Err(e) = Self::write_events(&keyboard, &[ev.into_event().into_raw()]) {
            tracing::error!("key_release error: {e}");
        }
    }

    /// Type a string of text, handling shift for uppercase letters and
    /// shifted punctuation characters on a US keyboard layout.
    pub async fn type_text(&self, text: &str) {
        for ch in text.chars() {
            if let Some((key, needs_shift)) = keymap::char_to_key_shifted(ch) {
                if needs_shift {
                    self.key_press(Key::LeftShift).await;
                }
                self.key_press(key).await;
                self.key_release(key).await;
                if needs_shift {
                    self.key_release(Key::LeftShift).await;
                }
            } else {
                tracing::warn!("type_text: unsupported character '{ch}'");
            }
        }
    }
}
