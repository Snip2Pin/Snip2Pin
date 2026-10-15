use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Home,
    Capture,
    ClosePins,
    CopyPin(u64),
    ClosePin(u64),
    Quit,
}

pub struct Tray {
    _icon: TrayIcon,
    capture: MenuItem,
    close_pins: MenuItem,
    state: Option<(bool, bool, bool)>,
}

impl Tray {
    pub fn new(callback: impl Fn(Action) + Send + Sync + 'static) -> Result<Self, String> {
        // Pin menus share this route even when the desktop has no tray host.
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some(action) = action(event.id.as_ref()) {
                callback(action);
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
                return Err("桌面未提供 StatusNotifier 托盘，应用将保留主页。".into());
            }
        }
        let menu = Menu::new();
        let home = MenuItem::with_id("home", "打开主页", true, None);
        let capture = MenuItem::with_id("capture", "截图并置顶", true, None);
        let close_pins = MenuItem::with_id("close-pins", "关闭全部贴图", false, None);
        let quit = MenuItem::with_id("quit", "退出 Snip2Pin", true, None);
        menu.append_items(&[
            &home,
            &capture,
            &close_pins,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .map_err(|error| error.to_string())?;
        let pixels = if cfg!(target_os = "macos") {
            snip2pin::icon::status()?
        } else {
            snip2pin::icon::render(snip2pin::icon::Variant::Line, snip2pin::icon::STATUS_SIZE)?
        };
        let (width, height) = pixels.dimensions();
        let icon = tray_icon::Icon::from_rgba(pixels.into_raw(), width, height)
            .map_err(|error| error.to_string())?;
        let builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Snip2Pin");
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(icon);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(icon);
        let icon = builder.build().map_err(|error| error.to_string())?;
        Ok(Self {
            _icon: icon,
            capture,
            close_pins,
            state: None,
        })
    }

    pub fn update(&mut self, capture: bool, has_pins: bool, capturing: bool) {
        if self.state == Some((capture, has_pins, capturing)) {
            return;
        }
        self.capture.set_enabled(capture);
        self.capture.set_text(if capturing {
            "取消截图"
        } else {
            "截图并置顶"
        });
        self.close_pins.set_enabled(has_pins);
        self.state = Some((capture, has_pins, capturing));
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
    }
}

fn action(id: &str) -> Option<Action> {
    match id {
        "home" => Some(Action::Home),
        "capture" => Some(Action::Capture),
        "close-pins" => Some(Action::ClosePins),
        "quit" => Some(Action::Quit),
        _ => pin_action(id),
    }
}

fn pin_action(id: &str) -> Option<Action> {
    let (value, close) = if let Some(value) = id.strip_prefix("pin-copy:") {
        (value, false)
    } else {
        (id.strip_prefix("pin-close:")?, true)
    };
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let id = value.parse().ok()?;
    Some(if close {
        Action::ClosePin(id)
    } else {
        Action::CopyPin(id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn menu_dispatches_only_known_application_commands() {
        for (id, expected) in [
            ("home", Action::Home),
            ("capture", Action::Capture),
            ("close-pins", Action::ClosePins),
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
            ("pin-copy:42", Action::CopyPin(42)),
            ("pin-close:42", Action::ClosePin(42)),
            ("pin-copy:18446744073709551615", Action::CopyPin(u64::MAX)),
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
            "pin-copy: 42",
            "other:42",
            "pin-copy:４２",
        ] {
            assert_eq!(action(id), None, "ignore malformed menu ID {id}");
        }
    }
}
