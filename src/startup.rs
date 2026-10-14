//! Current-user login startup. Construction/read never enables startup.
#[cfg(not(target_os = "windows"))]
use std::io::Write;
#[cfg(not(target_os = "windows"))]
use std::path::{Path, PathBuf};

pub struct Startup {
    #[cfg(not(target_os = "windows"))]
    path: PathBuf,
    #[cfg(not(target_os = "windows"))]
    data: Vec<u8>,
    #[cfg(target_os = "windows")]
    command: String,
}

impl Startup {
    pub fn new() -> Result<Self, String> {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let executable_text = executable.to_str().ok_or("应用路径不是有效 Unicode。")?;
        #[cfg(target_os = "macos")]
        if !executable
            .ancestors()
            .any(|path| path.extension().is_some_and(|ext| ext == "app"))
        {
            return Err("请先安装应用，再开启开机自启。".into());
        }
        #[cfg(not(target_os = "windows"))]
        {
            let dirs = directories::BaseDirs::new().ok_or("无法找到用户配置目录。")?;
            #[cfg(target_os = "macos")]
            let path = dirs
                .home_dir()
                .join("Library/LaunchAgents")
                .join(format!("{}.plist", snip2pin::APP_ID));
            #[cfg(target_os = "linux")]
            let path = dirs.config_dir().join("autostart/snip2pin.desktop");
            Ok(Self {
                path,
                data: entry(executable_text)?,
            })
        }
        #[cfg(target_os = "windows")]
        Ok(Self {
            command: format!("\"{executable_text}\" --background"),
        })
    }

    pub fn enabled(&self) -> Result<bool, String> {
        #[cfg(not(target_os = "windows"))]
        return self.path.try_exists().map_err(|error| error.to_string());
        #[cfg(target_os = "windows")]
        {
            let key = match windows_registry::CURRENT_USER.open(RUN_KEY) {
                Ok(key) => key,
                Err(error) if missing(&error) => return Ok(false),
                Err(error) => return Err(error.to_string()),
            };
            match key.get_string("Snip2Pin") {
                Ok(_) => Ok(true),
                Err(error) if missing(&error) => Ok(false),
                Err(error) => Err(error.to_string()),
            }
        }
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<bool, String> {
        #[cfg(not(target_os = "windows"))]
        write_entry(&self.path, &self.data, enabled)?;
        #[cfg(target_os = "windows")]
        {
            if enabled {
                windows_registry::CURRENT_USER
                    .create(RUN_KEY)
                    .and_then(|key| key.set_string("Snip2Pin", &self.command))
                    .map_err(|error| error.to_string())?;
            } else {
                match windows_registry::CURRENT_USER
                    .options()
                    .write()
                    .open(RUN_KEY)
                    .and_then(|key| key.remove_value("Snip2Pin"))
                {
                    Ok(()) => {}
                    Err(error) if missing(&error) => {}
                    Err(error) => return Err(error.to_string()),
                }
            }
        }
        self.enabled()
    }
}

#[cfg(target_os = "windows")]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[cfg(target_os = "windows")]
fn missing(error: &windows_result::Error) -> bool {
    error.code()
        == windows_result::HRESULT::from_win32(windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND)
}

#[cfg(not(target_os = "windows"))]
fn write_entry(path: &Path, data: &[u8], enabled: bool) -> Result<(), String> {
    let result = || -> Result<(), Box<dyn std::error::Error>> {
        if enabled {
            let parent = path.parent().ok_or("自启配置没有父目录。")?;
            std::fs::create_dir_all(parent)?;
            let mut file = tempfile::NamedTempFile::new_in(parent)?;
            file.write_all(data)?;
            file.as_file().sync_all()?;
            file.persist(path)?;
        } else {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    };
    result().map_err(|error| format!("修改开机自启失败：{error}"))
}

#[cfg(target_os = "macos")]
fn entry(executable: &str) -> Result<Vec<u8>, String> {
    let mut fields = plist::Dictionary::new();
    fields.insert("Label".into(), snip2pin::APP_ID.into());
    fields.insert(
        "ProgramArguments".into(),
        plist::Value::Array(vec![executable.into(), "--background".into()]),
    );
    fields.insert(
        "AssociatedBundleIdentifiers".into(),
        plist::Value::Array(vec![snip2pin::APP_ID.into()]),
    );
    fields.insert("RunAtLoad".into(), true.into());
    fields.insert("ProcessType".into(), "Interactive".into());
    let mut data = Vec::new();
    plist::Value::Dictionary(fields)
        .to_writer_xml(&mut data)
        .map_err(|error| error.to_string())?;
    Ok(data)
}

#[cfg(target_os = "linux")]
fn entry(executable: &str) -> Result<Vec<u8>, String> {
    if executable.contains(['\n', '\r', '=']) {
        return Err("此路径不能用于桌面自启入口。".into());
    }
    let mut escaped = String::new();
    for character in executable.chars() {
        match character {
            '\\' => escaped.push_str("\\\\\\\\"),
            '"' | '$' | '`' => {
                escaped.push_str("\\\\");
                escaped.push(character);
            }
            '%' => escaped.push_str("%%"),
            other => escaped.push(other),
        }
    }
    Ok(format!("[Desktop Entry]\nType=Application\nName=Snip2Pin\nExec=\"{escaped}\" --background\nTerminal=false\n").into_bytes())
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;
    #[test]
    fn startup_is_disabled_until_explicitly_enabled_and_can_be_removed() {
        let directory = tempfile::tempdir().expect("isolated startup fixture");
        let startup = Startup {
            path: directory.path().join("login/entry"),
            data: entry("/Applications/A&B.app/Contents/MacOS/snip2pin").expect("encode path"),
        };
        assert!(!startup.enabled().expect("default off"));
        assert!(startup.set_enabled(true).expect("enable temporary fixture"));
        assert_eq!(std::fs::read(&startup.path).expect("entry"), startup.data);
        assert!(
            !startup
                .set_enabled(false)
                .expect("disable temporary fixture")
        );
        assert!(!startup.set_enabled(false).expect("idempotent disable"));
        #[cfg(target_os = "macos")]
        {
            let value =
                plist::Value::from_reader_xml(startup.data.as_slice()).expect("valid plist");
            let args = value.as_dictionary().expect("dictionary")["ProgramArguments"]
                .as_array()
                .expect("argv");
            assert_eq!(
                args[0].as_string(),
                Some("/Applications/A&B.app/Contents/MacOS/snip2pin")
            );
            assert_eq!(args[1].as_string(), Some("--background"));
        }
    }
}
