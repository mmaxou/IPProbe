//! 开机自启动:通过 Windows 注册表 HKCU\Software\Microsoft\Windows\CurrentVersion\Run 实现。
//! 注意:这里只负责“写入/删除/查询”运行键,不负责创建应用目录、资源等。
//! 非 Windows 平台为空操作,便于跨平台开发与测试。

/// 注册表 Run 键的完整路径。
pub const RUN_KEY_PATH: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Run";

/// 当前用户注册表值名称。
pub const VALUE_NAME: &str = "IPProbe";

/// 写入开机自启动注册表项。
///
/// # Arguments
/// - `exe_path`: 程序可执行文件完整路径,如 `C:\Program Files\IPProbe\IPProbe.exe`。
/// - `enable`: `true` 写入,`false` 删除。
pub fn set_autostart(exe_path: &str, enable: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run_key = hkcu
            .open_subkey_with_flags(
                RUN_KEY_PATH,
                winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_QUERY_VALUE,
            )
            .map_err(|e| format!("打开注册表 Run 键失败: {e}"))?;

        if enable {
            run_key
                .set_value(VALUE_NAME, &exe_path)
                .map_err(|e| format!("写入注册表失败: {e}"))?;
        } else {
            // 删除时如果值不存在会返回 NotFound,可忽略。
            match run_key.delete_value(VALUE_NAME) {
                Ok(()) => {}
                Err(e)
                    if e.kind() == std::io::ErrorKind::NotFound =>
                {
                    // 本来就不存在,视为成功。
                }
                Err(e) => return Err(format!("删除注册表值失败: {e}")),
            }
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = (exe_path, enable);
        Ok(())
    }
}

/// 查询当前是否已启用开机自启动。
pub fn autostart_enabled() -> bool {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        match hkcu.open_subkey_with_flags(
            RUN_KEY_PATH,
            winreg::enums::KEY_QUERY_VALUE,
        ) {
            Ok(key) => match key.get_value::<String, _>(VALUE_NAME) {
                Ok(_) => true,
                Err(_) => false,
            },
            Err(_) => false,
        }
    }

    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_key_path_is_valid() {
        // 仅验证常量完整性
        assert!(!RUN_KEY_PATH.is_empty());
        assert!(RUN_KEY_PATH.contains("CurrentVersion"));
        assert_eq!(VALUE_NAME, "IPProbe");
    }
}
