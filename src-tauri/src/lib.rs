mod config;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tauri::{
    menu::{AboutMetadata, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID},
    AppHandle, Emitter, Manager,
};
use tauri_plugin_dialog::{DialogExt, FilePath};

static WINDOW_COUNTER: AtomicU32 = AtomicU32::new(1);

fn emit_to_focused<R: tauri::Runtime>(app: &tauri::AppHandle<R>, event: &str, payload: impl serde::Serialize + Clone) {
    for (_, win) in app.webview_windows() {
        if win.is_focused().unwrap_or(false) {
            let _ = win.emit(event, payload);
            return;
        }
    }
}

fn new_window(app: &tauri::AppHandle) {
    let n = WINDOW_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    let label = format!("win-{}", n);

    let (x, y) = app.webview_windows()
        .into_values()
        .find(|w| w.is_focused().unwrap_or(false))
        .and_then(|w| {
            let scale = w.scale_factor().ok()?;
            let phys = w.outer_position().ok()?;
            let log = phys.to_logical::<f64>(scale);
            Some((log.x + 25.0, log.y + 25.0))
        })
        .unwrap_or((100.0, 100.0));

    if let Err(e) = tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App("index.html".into()))
        .title("LikeNotepad.exe")
        .inner_size(800.0, 600.0)
        .min_inner_size(400.0, 300.0)
        .position(x, y)
        .resizable(true)
        .decorations(true)
        .visible(false)
        .build()
    {
        eprintln!("new_window error: {e}");
    }
}

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
    let last_dir = config::load(&app).last_dir;
    let (tx, rx) = std::sync::mpsc::channel();
    let mut builder = app.dialog()
        .file()
        .add_filter("Text", &["txt", "md", "log", "csv"]);
    if let Some(dir) = &last_dir {
        builder = builder.set_directory(dir);
    }
    builder.pick_file(move |path| {
        let _ = tx.send(path);
    });
    let path = rx.recv().map_err(|e| e.to_string())?;
    match path {
        Some(FilePath::Path(p)) => {
            let path_str = p.to_string_lossy().to_string();
            let content = read_file_content(&path_str)?;
            add_recent(&app, &path_str);
            Ok(Some((path_str, content)))
        }
        _ => Ok(None),
    }
}

fn read_file_content(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let content = String::from_utf8(bytes).unwrap_or_else(|e| {
        let (decoded, _, _) = encoding_rs::WINDOWS_1251.decode(e.as_bytes());
        decoded.into_owned()
    });
    Ok(content)
}

#[tauri::command]
async fn save_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, &content).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_theme(app: tauri::AppHandle) -> String {
    config::load(&app).theme
}

#[tauri::command]
fn set_theme(app: tauri::AppHandle, theme: String) -> Result<(), String> {
    let mut cfg = config::load(&app);
    cfg.theme = theme;
    config::save(&app, &cfg)
}

#[tauri::command]
fn get_word_wrap(app: tauri::AppHandle) -> bool {
    config::load(&app).word_wrap
}

#[tauri::command]
fn set_word_wrap(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let mut cfg = config::load(&app);
    cfg.word_wrap = enabled;
    config::save(&app, &cfg)
}

#[tauri::command]
fn get_status_bar(app: tauri::AppHandle) -> bool {
    config::load(&app).status_bar
}

#[tauri::command]
fn set_status_bar(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let mut cfg = config::load(&app);
    cfg.status_bar = enabled;
    config::save(&app, &cfg)
}

#[tauri::command]
async fn save_file_as(app: AppHandle, content: String, suggested_name: String) -> Result<Option<String>, String> {
    let last_dir = config::load(&app).last_dir;
    let (tx, rx) = std::sync::mpsc::channel();
    let mut builder = app.dialog()
        .file()
        .add_filter("Text", &["txt"])
        .set_file_name(&suggested_name);
    if let Some(dir) = &last_dir {
        builder = builder.set_directory(dir);
    }
    builder.save_file(move |path| {
        let _ = tx.send(path);
    });
    let path = rx.recv().map_err(|e| e.to_string())?;
    match path {
        Some(FilePath::Path(p)) => {
            let path_str = p.to_string_lossy().to_string();
            std::fs::write(&p, &content).map_err(|e| e.to_string())?;
            add_recent(&app, &path_str);
            Ok(Some(path_str))
        }
        _ => Ok(None),
    }
}

struct RecentFilesState(std::sync::Mutex<Vec<String>>);
struct RecentMenuState(std::sync::Mutex<Option<tauri::menu::Submenu<tauri::Wry>>>);

fn add_recent(app: &tauri::AppHandle, path: &str) {
    // обновить state: убрать дубликат, вставить в начало, обрезать до 10
    {
        let state = app.state::<RecentFilesState>();
        let mut guard = state.0.lock().unwrap();
        guard.retain(|p| p != path);
        guard.insert(0, path.to_string());
        guard.truncate(10);
    }
    // обновить last_dir = папка файла
    let last_dir = std::path::Path::new(path)
        .parent()
        .map(|p| p.to_string_lossy().to_string());
    // записать в конфиг (recent + last_dir)
    let mut cfg = config::load(app);
    cfg.recent_files = app.state::<RecentFilesState>().0.lock().unwrap().clone();
    if last_dir.is_some() { cfg.last_dir = last_dir; }
    let _ = config::save(app, &cfg);
    // перестроить меню
    rebuild_recent_menu(app);
}

fn rebuild_recent_menu(app: &tauri::AppHandle) {
    let recent: Vec<String> = {
        let state = app.state::<RecentFilesState>();
        let guard = state.0.lock().unwrap();
        guard.clone()
    };
    let menu_state = app.state::<RecentMenuState>();
    let guard = menu_state.0.lock().unwrap();
    let submenu = match guard.as_ref() { Some(s) => s, None => return };
    // очистить
    while let Ok(Some(_)) = submenu.remove_at(0) {}
    let lang = system_lang();
    if recent.is_empty() {
        let empty = MenuItem::with_id(app, "recent_empty", if lang == "ru" { "Нет недавних файлов" } else { "No Recent Files" }, false, None::<&str>).unwrap();
        let _ = submenu.append(&empty);
        return;
    }
    // пункты recent-0..N с именем файла
    for (i, path) in recent.iter().enumerate() {
        let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.clone());
        let item = MenuItem::with_id(app, format!("recent-{}", i), name, true, None::<&str>).unwrap();
        let _ = submenu.append(&item);
    }
    let sep = PredefinedMenuItem::separator(app).unwrap();
    let _ = submenu.append(&sep);
    let clear = MenuItem::with_id(app, "clear_recent", if lang == "ru" { "Очистить меню" } else { "Clear Menu" }, true, None::<&str>).unwrap();
    let _ = submenu.append(&clear);
}

pub fn run() {
    let lang = system_lang();

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(RecentFilesState(std::sync::Mutex::new(Vec::new())))
        .manage(RecentMenuState(std::sync::Mutex::new(None)))
        .setup(move |app| {
            let handle = app.handle();
            let mut cfg = config::load(handle);
            let saved_theme = cfg.theme.clone();
            let saved_wrap = cfg.word_wrap;
            let saved_status = cfg.status_bar;
            cfg.recent_files.retain(|p| std::path::Path::new(p).exists());
            let _ = config::save(handle, &cfg);
            *app.state::<RecentFilesState>().0.lock().unwrap() = cfg.recent_files.clone();

            // Build native menu
            let about_label = if lang == "ru" { "О программе LikeNotepad.exe" } else { "About LikeNotepad.exe" };
            let comments = if lang == "ru" {
                "Тот самый Блокнот, но без Windows. Никаких облаков, подписок и ИИ. Просто текст"
            } else {
                "The same Notepad, but without Windows. No clouds, no subscriptions, no AI. Just text"
            };
            let about_item = PredefinedMenuItem::about(handle, Some(about_label), Some(AboutMetadata {
                name:          Some("LikeNotepad.exe".to_string()),
                version:       Some("0.1.0".to_string()),
                authors:       Some(vec!["Timur Polatov".to_string()]),
                copyright:     Some("Timur Polatov".to_string()),
                website:       Some("https://polatov.me".to_string()),
                website_label: Some("https://polatov.me".to_string()),
                comments:      Some(comments.to_string()),
                ..Default::default()
            }))?;
            let sep_app = PredefinedMenuItem::separator(handle)?;
            let quit_label = if lang == "ru" { "Завершить LikeNotepad.exe" } else { "Quit LikeNotepad.exe" };
            let quit_item = PredefinedMenuItem::quit(handle, Some(quit_label))?;
            let app_menu = Submenu::with_items(handle, "LikeNotepad.exe", true, &[&about_item, &sep_app, &quit_item])?;

            let new_item = MenuItem::with_id(handle, "new", if lang == "ru" { "Создать" } else { "New" }, true, Some("CmdOrCtrl+N"))?;
            let open_item = MenuItem::with_id(handle, "open", if lang == "ru" { "Открыть..." } else { "Open..." }, true, Some("CmdOrCtrl+O"))?;
            let recent_label = if lang == "ru" { "Открыть недавние" } else { "Open Recent" };
            let recent_submenu = Submenu::with_id(handle, "recent_submenu", recent_label, true)?;
            let save_item = MenuItem::with_id(handle, "save", if lang == "ru" { "Сохранить" } else { "Save" }, true, Some("CmdOrCtrl+S"))?;
            let save_as_item = MenuItem::with_id(handle, "save_as", if lang == "ru" { "Сохранить как..." } else { "Save As..." }, true, Some("CmdOrCtrl+Shift+S"))?;
            let sep = PredefinedMenuItem::separator(handle)?;

            let file_label = if lang == "ru" { "Файл" } else { "File" };
            let file_menu = Submenu::with_items(handle, file_label, true, &[
                &new_item,
                &open_item,
                &recent_submenu,
                &sep,
                &save_item,
                &save_as_item,
            ])?;
            app.state::<RecentMenuState>().0.lock().unwrap().replace(recent_submenu.clone());

            let undo       = MenuItem::with_id(handle, "edit_undo",       if lang == "ru" { "Отменить"     } else { "Undo"        }, true, Some("CmdOrCtrl+Z"))?;
            let redo       = MenuItem::with_id(handle, "edit_redo",       if lang == "ru" { "Повторить"    } else { "Redo"        }, true, Some("CmdOrCtrl+Shift+Z"))?;
            let sep2 = PredefinedMenuItem::separator(handle)?;
            let cut        = MenuItem::with_id(handle, "edit_cut",        if lang == "ru" { "Вырезать"     } else { "Cut"         }, true, Some("CmdOrCtrl+X"))?;
            let copy       = MenuItem::with_id(handle, "edit_copy",       if lang == "ru" { "Копировать"   } else { "Copy"        }, true, Some("CmdOrCtrl+C"))?;
            let paste      = MenuItem::with_id(handle, "edit_paste",      if lang == "ru" { "Вставить"     } else { "Paste"       }, true, Some("CmdOrCtrl+V"))?;
            let sep3 = PredefinedMenuItem::separator(handle)?;
            let select_all = MenuItem::with_id(handle, "edit_select_all", if lang == "ru" { "Выделить всё" } else { "Select All"  }, true, Some("CmdOrCtrl+A"))?;
            let find_item = MenuItem::with_id(handle, "edit_find", if lang == "ru" { "Найти..." } else { "Find..." }, true, Some("CmdOrCtrl+F"))?;
            let find_next_item = MenuItem::with_id(handle, "edit_find_next", if lang == "ru" { "Найти далее" } else { "Find Next" }, true, Some("CmdOrCtrl+G"))?;
            let replace_item = MenuItem::with_id(handle, "edit_replace", if lang == "ru" { "Заменить..." } else { "Replace..." }, true, Some("CmdOrCtrl+Alt+F"))?;
            let sep4 = PredefinedMenuItem::separator(handle)?;
            let datetime_item = MenuItem::with_id(handle, "edit_datetime", if lang == "ru" { "Время/Дата" } else { "Time/Date" }, true, Some("CmdOrCtrl+Shift+T"))?;

            let edit_label = if lang == "ru" { "Правка" } else { "Edit" };
            let edit_menu = Submenu::with_items(handle, edit_label, true, &[
                &undo,
                &redo,
                &sep2,
                &cut,
                &copy,
                &paste,
                &sep3,
                &find_item,
                &find_next_item,
                &replace_item,
                &sep4,
                &select_all,
                &datetime_item,
            ])?;

            let theme_auto  = CheckMenuItem::with_id(handle, "theme_auto",  if lang == "ru" { "Авто"    } else { "Auto"  }, true, saved_theme == "auto",  None::<&str>)?;
            let theme_light = CheckMenuItem::with_id(handle, "theme_light", if lang == "ru" { "Светлая" } else { "Light" }, true, saved_theme == "light", None::<&str>)?;
            let theme_dark  = CheckMenuItem::with_id(handle, "theme_dark",  if lang == "ru" { "Тёмная"  } else { "Dark"  }, true, saved_theme == "dark",  None::<&str>)?;
            let ta = theme_auto.clone();
            let tl = theme_light.clone();
            let td = theme_dark.clone();

            let status_item = CheckMenuItem::with_id(handle, "status_bar", if lang == "ru" { "Строка состояния" } else { "Status Bar" }, true, saved_status, None::<&str>)?;
            let si = status_item.clone();
            let sep_view = PredefinedMenuItem::separator(handle)?;

            let view_label = if lang == "ru" { "Вид" } else { "View" };
            let view_menu = Submenu::with_items(handle, view_label, true, &[
                &theme_auto,
                &theme_light,
                &theme_dark,
                &sep_view,
                &status_item,
            ])?;

            let wrap_label = if lang == "ru" { "Перенос по словам" } else { "Word Wrap" };
            let wrap_item = CheckMenuItem::with_id(handle, "word_wrap", wrap_label, true, saved_wrap, None::<&str>)?;
            let wi = wrap_item.clone();
            let wrap_state = std::sync::Arc::new(AtomicBool::new(saved_wrap));
            let wrap_state_menu = wrap_state.clone();

            let status_state = std::sync::Arc::new(AtomicBool::new(saved_status));
            let status_state_menu = status_state.clone();

            let format_label = if lang == "ru" { "Формат" } else { "Format" };
            let format_menu = Submenu::with_items(handle, format_label, true, &[&wrap_item])?;

            let help_label = if lang == "ru" { "Справка" } else { "Help" };
            let help_menu = Submenu::with_id_and_items(handle, HELP_SUBMENU_ID, help_label, true, &[])?;

            let menu = Menu::with_items(handle, &[&app_menu, &file_menu, &edit_menu, &format_menu, &view_menu, &help_menu])?;
            app.set_menu(menu)?;
            rebuild_recent_menu(handle);

            // Handle menu events
            app.on_menu_event(move |app, event| {
                match event.id().as_ref() {
                    "new" => { new_window(app); }
                    "open" => { emit_to_focused(app, "menu-open", ()); }
                    "save" => { emit_to_focused(app, "menu-save", ()); }
                    "save_as"    => { emit_to_focused(app, "menu-save-as", ()); }
                    "edit_undo"       => { emit_to_focused(app, "menu-edit", "undo"); }
                    "edit_redo"       => { emit_to_focused(app, "menu-edit", "redo"); }
                    "edit_cut"        => { emit_to_focused(app, "menu-edit", "cut"); }
                    "edit_copy"       => { emit_to_focused(app, "menu-edit", "copy"); }
                    "edit_paste"      => { emit_to_focused(app, "menu-edit", "paste"); }
                    "edit_select_all" => { emit_to_focused(app, "menu-edit", "select-all"); }
                    "edit_find" => { emit_to_focused(app, "menu-edit", "find"); }
                    "edit_find_next" => { emit_to_focused(app, "menu-edit", "find-next"); }
                    "edit_replace" => { emit_to_focused(app, "menu-edit", "replace"); }
                    "edit_datetime" => { emit_to_focused(app, "menu-edit", "datetime"); }
                    id if id.starts_with("recent-") => {
                        if let Ok(index) = id["recent-".len()..].parse::<usize>() {
                            let path_opt = {
                                let state = app.state::<RecentFilesState>();
                                let guard = state.0.lock().unwrap();
                                guard.get(index).cloned()
                            };
                            if let Some(path) = path_opt {
                                if std::path::Path::new(&path).exists() {
                                    match read_file_content(&path) {
                                        Ok(content) => {
                                            emit_to_focused(app, "menu-open-recent", (path.clone(), content));
                                            add_recent(app, &path); // поднять наверх списка
                                        }
                                        Err(_) => {}
                                    }
                                } else {
                                    // файл удалён — убрать из списка и перестроить меню
                                    {
                                        let state = app.state::<RecentFilesState>();
                                        let mut guard = state.0.lock().unwrap();
                                        guard.retain(|p| p != &path);
                                    }
                                    let mut cfg = config::load(app);
                                    cfg.recent_files = app.state::<RecentFilesState>().0.lock().unwrap().clone();
                                    let _ = config::save(app, &cfg);
                                    rebuild_recent_menu(app);
                                }
                            }
                        }
                    }
                    "clear_recent" => {
                        {
                            let state = app.state::<RecentFilesState>();
                            let mut guard = state.0.lock().unwrap();
                            guard.clear();
                        }
                        let mut cfg = config::load(app);
                        cfg.recent_files.clear();
                        let _ = config::save(app, &cfg);
                        rebuild_recent_menu(app);
                    }
                    "theme_auto" | "theme_light" | "theme_dark" => {
                        let id = event.id().as_ref();
                        let _ = ta.set_checked(id == "theme_auto");
                        let _ = tl.set_checked(id == "theme_light");
                        let _ = td.set_checked(id == "theme_dark");
                        let payload = if id == "theme_light" { "light" }
                                      else if id == "theme_dark" { "dark" }
                                      else { "auto" };
                        emit_to_focused(app, "menu-theme", payload);
                    }
                    "word_wrap" => {
                        let new_state = !wrap_state_menu.load(Ordering::Relaxed);
                        wrap_state_menu.store(new_state, Ordering::Relaxed);
                        let _ = wi.set_checked(new_state);
                        emit_to_focused(app, "menu-word-wrap", new_state);
                    }
                    "status_bar" => {
                        let new_state = !status_state_menu.load(Ordering::Relaxed);
                        status_state_menu.store(new_state, Ordering::Relaxed);
                        let _ = si.set_checked(new_state);
                        emit_to_focused(app, "menu-status-bar", new_state);
                    }
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
            get_theme,
            set_theme,
            get_word_wrap,
            set_word_wrap,
            get_status_bar,
            set_status_bar,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
