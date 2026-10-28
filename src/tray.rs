use snip2pin::i18n::{Locale, Message, Text};
use snip2pin::pin::MenuCommand;
use std::sync::{Arc, Mutex};
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

#[cfg(target_os = "windows")]
#[path = "tray/windows.rs"]
mod windows;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    #[cfg(target_os = "windows")]
    ShowMenu,
    Home,
    Help,
    Capture,
    ClosePins,
    UnlockPins,
    Pin {
        id: u64,
        command: MenuCommand,
    },
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuState {
    pub locale: Locale,
    pub capture_enabled: bool,
    pub has_unlocked_pins: bool,
    pub has_locked_pins: bool,
    pub capturing: bool,
}

pub struct Tray {
    _icon: TrayIcon,
    capture: MenuItem,
    home: MenuItem,
    help: MenuItem,
    close_pins: MenuItem,
    unlock_pins: MenuItem,
    quit: MenuItem,
    state: Option<MenuState>,
    callback: CallbackSlot,
    #[cfg(target_os = "windows")]
    menu: Menu,
}

type Callback = Arc<dyn Fn(Action) + Send + Sync>;
type CallbackSlot = Arc<Mutex<Option<Callback>>>;

fn emit(callback: &CallbackSlot, action: Action) {
    // Release the lock before invoking application code, including reentrant teardown.
    let callback = callback.lock().ok().and_then(|callback| callback.clone());
    if let Some(callback) = callback {
        callback(action);
    }
}

impl Tray {
    pub fn new(
        locale: Locale,
        callback: impl Fn(Action) + Send + Sync + 'static,
    ) -> Result<Self, Message> {
        let callback: CallbackSlot = Arc::new(Mutex::new(Some(Arc::new(callback))));
        let menu_callback = callback.clone();
        // Pin menus share this route even when the desktop has no tray host.
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some(action) = action(event.id.as_ref()) {
                emit(&menu_callback, action);
            }
        }));
        #[cfg(target_os = "linux")]
        {
            // KSNI can register without a host. Check once before hiding Home,
            // so desktops without a tray never leave the app inaccessible.
            let connection = zbus::blocking::connection::Builder::session()
                .and_then(|builder| {
                    builder
                        .method_timeout(std::time::Duration::from_secs(1))
                        .build()
                })
                .map_err(|error| error.to_string())?;
            let proxy = zbus::blocking::fdo::DBusProxy::new(&connection)
                .map_err(|error| error.to_string())?;
            let name = "org.kde.StatusNotifierWatcher"
                .try_into()
                .map_err(|error: zbus::names::Error| error.to_string())?;
            if !proxy
                .name_has_owner(name)
                .map_err(|error| error.to_string())?
            {
                return Err(Text::NoTrayHost.into());
            }
        }
        let menu = Menu::new();
        let home = MenuItem::with_id("home", locale.text(Text::Home), true, None);
        let help = MenuItem::with_id("help", locale.text(Text::Help), true, None);
        let capture = MenuItem::with_id("capture", locale.text(Text::Capture), true, None);
        let close_pins = MenuItem::with_id("close-pins", locale.text(Text::ClosePins), false, None);
        let unlock_pins =
            MenuItem::with_id("unlock-pins", locale.text(Text::UnlockPins), false, None);
        let quit = MenuItem::with_id("quit", locale.text(Text::Quit), true, None);
        menu.append_items(&[
            &home,
            &capture,
            &close_pins,
            &unlock_pins,
            &PredefinedMenuItem::separator(),
            &help,
            &quit,
        ])
        .map_err(|error| error.to_string())?;
        let icon = icon()?;
        let builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            .with_tooltip("Snip2Pin");
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(icon);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(icon);
        #[cfg(target_os = "windows")]
        let builder = builder
            .with_menu_on_left_click(false)
            .with_menu_on_right_click(false);
        #[cfg(target_os = "windows")]
        {
            let id = builder.id().clone();
            let callback = callback.clone();
            tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
                if windows::requests_menu(&id, &event) {
                    emit(&callback, Action::ShowMenu);
                }
            }));
        }
        let icon = builder.build().map_err(|error| error.to_string())?;
        Ok(Self {
            _icon: icon,
            capture,
            home,
            help,
            close_pins,
            unlock_pins,
            quit,
            state: None,
            callback,
            #[cfg(target_os = "windows")]
            menu,
        })
    }

    /// Capture is returned only after its native popup has been cancelled and unwound.
    #[cfg(target_os = "windows")]
    pub fn show_menu(&self) -> Result<bool, Message> {
        windows::show_menu(&self._icon, &self.menu, self.capture.id())
    }

    pub fn update(&mut self, state: MenuState) {
        if self.state == Some(state) {
            return;
        }
        self.capture.set_enabled(state.capture_enabled);
        self.home.set_text(state.locale.text(Text::Home));
        self.help.set_text(state.locale.text(Text::Help));
        self.close_pins.set_text(state.locale.text(Text::ClosePins));
        self.quit.set_text(state.locale.text(Text::Quit));
        self.capture.set_text(state.locale.text(if state.capturing {
            Text::CancelCapture
        } else {
            Text::Capture
        }));
        self.close_pins.set_enabled(state.has_unlocked_pins);
        self.unlock_pins
            .set_text(state.locale.text(Text::UnlockPins));
        self.unlock_pins.set_enabled(state.has_locked_pins);
        self.state = Some(state);
    }
}

fn icon() -> Result<tray_icon::Icon, Message> {
    #[cfg(target_os = "windows")]
    return windows::icon();

    #[cfg(not(target_os = "windows"))]
    {
        let pixels = if cfg!(target_os = "macos") {
            snip2pin::icon::status()?
        } else {
            snip2pin::icon::render(snip2pin::icon::Variant::Line, snip2pin::icon::STATUS_SIZE)?
        };
        let (width, height) = pixels.dimensions();
        tray_icon::Icon::from_rgba(pixels.into_raw(), width, height)
            .map_err(|error| error.to_string().into())
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        // Both libraries keep their handler in an immutable OnceCell. Revoke the
        // shared application callback instead of pretending set(None) removes it.
        if let Ok(mut callback) = self.callback.lock() {
            callback.take();
        }
    }
}

fn action(id: &str) -> Option<Action> {
    match id {
        "home" => Some(Action::Home),
        "help" => Some(Action::Help),
        "capture" => Some(Action::Capture),
        "close-pins" => Some(Action::ClosePins),
        "unlock-pins" => Some(Action::UnlockPins),
        "quit" => Some(Action::Quit),
        _ => pin_action(id),
    }
}

fn pin_action(id: &str) -> Option<Action> {
    let (command, value) = id.split_once(':')?;
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let id = value.parse().ok()?;
    let command = MenuCommand::ALL
        .into_iter()
        .find(|item| item.key() == command)?;
    Some(Action::Pin { id, command })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_can_revoke_both_routes_without_holding_the_dispatch_lock() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let slot: CallbackSlot = Arc::new(Mutex::new(None));
        let calls = Arc::new(AtomicUsize::new(0));
        let revoke = slot.clone();
        let counted = calls.clone();
        *slot.lock().expect("callback slot") = Some(Arc::new(move |_| {
            counted.fetch_add(1, Ordering::Relaxed);
            revoke.lock().expect("dispatch releases lock").take();
        }));
        emit(&slot, action("capture").expect("menu event route"));
        emit(&slot, Action::Home);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert!(slot.lock().expect("revoked slot").is_none());
    }
    #[test]
    fn menu_dispatches_only_known_application_commands() {
        for (id, expected) in [
            ("home", Action::Home),
            ("help", Action::Help),
            ("capture", Action::Capture),
            ("close-pins", Action::ClosePins),
            ("unlock-pins", Action::UnlockPins),
            ("quit", Action::Quit),
        ] {
            assert_eq!(action(id), Some(expected));
        }
        assert_eq!(action("other-app"), None);
        let icon = snip2pin::icon::status().expect("symbolic tray artwork");
        assert!(icon.pixels().any(|pixel| pixel.0[3] == 0));
        assert!(icon.pixels().any(|pixel| pixel.0[3] > 0));
    }

    #[test]
    fn native_pin_menu_ids_route_only_complete_unsigned_pin_ids() {
        for (id, expected) in [
            (
                "pin-copy:42",
                Action::Pin {
                    id: 42,
                    command: snip2pin::pin::MenuCommand::Copy,
                },
            ),
            (
                "pin-close:42",
                Action::Pin {
                    id: 42,
                    command: snip2pin::pin::MenuCommand::Close,
                },
            ),
            (
                "pin-lock:42",
                Action::Pin {
                    id: 42,
                    command: snip2pin::pin::MenuCommand::ToggleLock,
                },
            ),
            (
                "pin-restore:42",
                Action::Pin {
                    id: 42,
                    command: snip2pin::pin::MenuCommand::RestoreSize,
                },
            ),
            (
                "pin-copy:18446744073709551615",
                Action::Pin {
                    id: u64::MAX,
                    command: snip2pin::pin::MenuCommand::Copy,
                },
            ),
        ] {
            assert_eq!(action(id), Some(expected));
        }
        for id in [
            "pin-copy:",
            "pin-close:",
            "pin-copy:-1",
            "pin-copy:+1",
            "pin-copy:1:2",
            "pin-close:18446744073709551616",
            "pin-lock:",
            "pin-lock:-1",
            "pin-lock:18446744073709551616",
            "pin-copy: 42",
            "other:42",
            "pin-copy:４２",
        ] {
            assert_eq!(action(id), None, "ignore malformed menu ID {id}");
        }
    }
}
