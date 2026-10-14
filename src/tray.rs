use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Home,
    Capture,
    ClosePins,
    Quit,
}

pub struct Tray {
    _icon: TrayIcon,
    capture: MenuItem,
    close_pins: MenuItem,
    state: Option<(bool, bool)>,
}

impl Tray {
    pub fn new(callback: impl Fn(Action) + Send + Sync + 'static) -> Result<Self, String> {
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
        let builder = if cfg!(target_os = "macos") {
            builder.with_icon_templated(icon)
        } else {
            builder.with_icon(icon)
        };
        let icon = builder.build().map_err(|error| error.to_string())?;
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some(action) = action(event.id.as_ref()) {
                callback(action);
            }
        }));
        Ok(Self {
            _icon: icon,
            capture,
            close_pins,
            state: None,
        })
    }

    pub fn update(&mut self, capture: bool, has_pins: bool) {
        if self.state == Some((capture, has_pins)) {
            return;
        }
        self.capture.set_enabled(capture);
        self.close_pins.set_enabled(has_pins);
        self.state = Some((capture, has_pins));
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
        _ => None,
    }
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
}
