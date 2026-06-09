use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    AppHandle, Emitter, Manager,
};
use tauri_plugin_dialog::{DialogExt, FilePath};

// Detect system language: returns "ru" or "en"
fn system_lang() -> &'static str {
    let locale = sys_locale::get_locale().unwrap_or_default();
    if locale.to_lowercase().starts_with("ru") {
        "ru"
    } else {
        "en"
    }
}

#[tauri::command]
fn get_lang() -> String {
    system_lang().to_string()
}

#[tauri::command]
async fn open_file(app: AppHandle) -> Result<Option<(String, String)>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .add_filter("Text", &["txt", "md", "log", "csv"])
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let path = rx.recv().map_err(|e| e.to_string())?;
    match path {
        Some(FilePath::Path(p)) => {
            let content = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            Ok(Some((p.to_string_lossy().to_string(), content)))
        }
        _ => Ok(None),
    }
}

#[tauri::command]
async fn save_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, &content).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_file_as(app: AppHandle, content: String) -> Result<Option<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .add_filter("Text", &["txt"])
        .set_file_name("Untitled.txt")
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let path = rx.recv().map_err(|e| e.to_string())?;
    match path {
        Some(FilePath::Path(p)) => {
            let path_str = p.to_string_lossy().to_string();
            std::fs::write(&p, &content).map_err(|e| e.to_string())?;
            Ok(Some(path_str))
        }
        _ => Ok(None),
    }
}

pub fn run() {
    let lang = system_lang();

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(move |app| {
            let handle = app.handle();

            // Build native menu
            let new_item = MenuItem::with_id(handle, "new", if lang == "ru" { "Новый" } else { "New" }, true, Some("CmdOrCtrl+N"))?;
            let open_item = MenuItem::with_id(handle, "open", if lang == "ru" { "Открыть..." } else { "Open..." }, true, Some("CmdOrCtrl+O"))?;
            let save_item = MenuItem::with_id(handle, "save", if lang == "ru" { "Сохранить" } else { "Save" }, true, Some("CmdOrCtrl+S"))?;
            let save_as_item = MenuItem::with_id(handle, "save_as", if lang == "ru" { "Сохранить как..." } else { "Save As..." }, true, Some("CmdOrCtrl+Shift+S"))?;
            let sep = PredefinedMenuItem::separator(handle)?;

            let file_label = if lang == "ru" { "Файл" } else { "File" };
            let file_menu = Submenu::with_items(handle, file_label, true, &[
                &new_item,
                &open_item,
                &sep,
                &save_item,
                &save_as_item,
            ])?;

            let undo       = MenuItem::with_id(handle, "edit_undo",       if lang == "ru" { "Отменить"     } else { "Undo"        }, true, Some("CmdOrCtrl+Z"))?;
            let redo       = MenuItem::with_id(handle, "edit_redo",       if lang == "ru" { "Повторить"    } else { "Redo"        }, true, Some("CmdOrCtrl+Shift+Z"))?;
            let sep2 = PredefinedMenuItem::separator(handle)?;
            let cut        = MenuItem::with_id(handle, "edit_cut",        if lang == "ru" { "Вырезать"     } else { "Cut"         }, true, Some("CmdOrCtrl+X"))?;
            let copy       = MenuItem::with_id(handle, "edit_copy",       if lang == "ru" { "Копировать"   } else { "Copy"        }, true, Some("CmdOrCtrl+C"))?;
            let paste      = MenuItem::with_id(handle, "edit_paste",      if lang == "ru" { "Вставить"     } else { "Paste"       }, true, Some("CmdOrCtrl+V"))?;
            let sep3 = PredefinedMenuItem::separator(handle)?;
            let select_all = MenuItem::with_id(handle, "edit_select_all", if lang == "ru" { "Выделить всё" } else { "Select All"  }, true, Some("CmdOrCtrl+A"))?;

            let edit_label = if lang == "ru" { "Правка" } else { "Edit" };
            let edit_menu = Submenu::with_items(handle, edit_label, true, &[
                &undo,
                &redo,
                &sep2,
                &cut,
                &copy,
                &paste,
                &sep3,
                &select_all,
            ])?;

            let theme_auto  = MenuItem::with_id(handle, "theme_auto",  if lang == "ru" { "Авто"     } else { "Auto"  }, true, None::<&str>)?;
            let theme_light = MenuItem::with_id(handle, "theme_light", if lang == "ru" { "Светлая"  } else { "Light" }, true, None::<&str>)?;
            let theme_dark  = MenuItem::with_id(handle, "theme_dark",  if lang == "ru" { "Тёмная"   } else { "Dark"  }, true, None::<&str>)?;

            let view_label = if lang == "ru" { "Вид" } else { "View" };
            let view_menu = Submenu::with_items(handle, view_label, true, &[
                &theme_auto,
                &theme_light,
                &theme_dark,
            ])?;

            let menu = Menu::with_items(handle, &[&file_menu, &edit_menu, &view_menu])?;
            app.set_menu(menu)?;

            // Handle menu events
            app.on_menu_event(|app, event| {
                match event.id().as_ref() {
                    "new" => { let _ = app.emit("menu-new", ()); }
                    "open" => { let _ = app.emit("menu-open", ()); }
                    "save" => { let _ = app.emit("menu-save", ()); }
                    "save_as"    => { let _ = app.emit("menu-save-as", ()); }
                    "edit_undo"       => { let _ = app.emit("menu-edit", "undo"); }
                    "edit_redo"       => { let _ = app.emit("menu-edit", "redo"); }
                    "edit_cut"        => { let _ = app.emit("menu-edit", "cut"); }
                    "edit_copy"       => { let _ = app.emit("menu-edit", "copy"); }
                    "edit_paste"      => { let _ = app.emit("menu-edit", "paste"); }
                    "edit_select_all" => { let _ = app.emit("menu-edit", "select-all"); }
                    "theme_auto"  => { let _ = app.emit("menu-theme", "auto"); }
                    "theme_light" => { let _ = app.emit("menu-theme", "light"); }
                    "theme_dark"  => { let _ = app.emit("menu-theme", "dark"); }
                    _ => {}
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_lang,
            open_file,
            save_file,
            save_file_as,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
