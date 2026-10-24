#[derive(Clone, Copy, Debug, PartialEq)]
enum Language {
    Chinese,
    English,
}

fn language(preferences: impl IntoIterator<Item = impl AsRef<str>>) -> Language {
    for preference in preferences {
        let tag = preference.as_ref().split(['-', '_']).next().unwrap_or("");
        if tag.eq_ignore_ascii_case("zh") {
            return Language::Chinese;
        }
        if tag.eq_ignore_ascii_case("en") {
            return Language::English;
        }
    }
    Language::English
}

#[cfg(all(feature = "desktop", target_os = "macos"))]
pub fn install(app: &tauri::App) -> tauri::Result<()> {
    use objc2_foundation::NSLocale;
    use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem as Item, Submenu};

    // Foundation respects the user's preferred languages, including app-specific overrides.
    let preferences: Vec<_> = NSLocale::preferredLanguages()
        .iter()
        .map(|item| item.to_string())
        .collect();
    let chinese = language(&preferences) == Language::Chinese;
    let text = |zh, en| if chinese { zh } else { en };
    let sep = || Item::separator(app);
    let application = Submenu::with_items(
        app,
        "SourceNeo",
        true,
        &[
            &Item::about(
                app,
                Some(text("关于 SourceNeo", "About SourceNeo")),
                Some(AboutMetadata {
                    name: Some("SourceNeo".into()),
                    version: Some(app.package_info().version.to_string()),
                    ..Default::default()
                }),
            )?,
            &sep()?,
            &Item::services(app, Some(text("服务", "Services")))?,
            &sep()?,
            &Item::hide(app, Some(text("隐藏 SourceNeo", "Hide SourceNeo")))?,
            &Item::hide_others(app, Some(text("隐藏其他", "Hide Others")))?,
            &Item::show_all(app, Some(text("全部显示", "Show All")))?,
            &sep()?,
            &Item::quit(app, Some(text("退出 SourceNeo", "Quit SourceNeo")))?,
        ],
    )?;
    let file = Submenu::with_items(
        app,
        text("文件", "File"),
        true,
        &[&Item::close_window(
            app,
            Some(text("关闭窗口", "Close Window")),
        )?],
    )?;
    let edit = Submenu::with_items(
        app,
        text("编辑", "Edit"),
        true,
        &[
            &Item::undo(app, Some(text("撤销", "Undo")))?,
            &Item::redo(app, Some(text("重做", "Redo")))?,
            &sep()?,
            &Item::cut(app, Some(text("剪切", "Cut")))?,
            &Item::copy(app, Some(text("复制", "Copy")))?,
            &Item::paste(app, Some(text("粘贴", "Paste")))?,
            &Item::select_all(app, Some(text("全选", "Select All")))?,
        ],
    )?;
    let view = Submenu::with_items(
        app,
        text("显示", "View"),
        true,
        &[&Item::fullscreen(
            app,
            Some(text("切换全屏幕", "Toggle Full Screen")),
        )?],
    )?;
    let window = Submenu::with_items(
        app,
        text("窗口", "Window"),
        true,
        &[
            &Item::minimize(app, Some(text("最小化", "Minimize")))?,
            &Item::maximize(app, Some(text("缩放", "Zoom")))?,
            &sep()?,
            &Item::close_window(app, Some(text("关闭窗口", "Close Window")))?,
        ],
    )?;
    let help = Submenu::with_items(app, text("帮助", "Help"), true, &[])?;
    app.set_menu(Menu::with_items(
        app,
        &[&application, &file, &edit, &view, &window, &help],
    )?)?;
    window.set_as_windows_menu_for_nsapp()?;
    help.set_as_help_menu_for_nsapp()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooses_first_supported_preferred_language() {
        assert_eq!(language(["zh-Hans-CN", "en-US"]), Language::Chinese);
        assert_eq!(language(["en-US", "zh-Hans"]), Language::English);
        assert_eq!(language(["fr-FR", "ZH_TW"]), Language::Chinese);
        assert_eq!(language(["de-DE"]), Language::English);
        assert_eq!(language([] as [&str; 0]), Language::English);
    }
}
