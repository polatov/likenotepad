mod config;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tauri::{
    menu::{AboutMetadata, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID},
    AppHandle, Emitter, Manager,
};
use tauri_plugin_dialog::{DialogExt, FilePath};
use objc2::define_class;
use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::NSObject;
use objc2::{AnyThread, ClassType, MainThreadMarker, MainThreadOnly};
use std::sync::OnceLock;
use objc2_app_kit::{NSAlert, NSAlertStyle, NSApplication, NSFont, NSFontManager, NSPageLayout, NSPrintOperation, NSTextView};
use objc2_foundation::{NSAttributedString, NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString};

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "FontChangeTarget"]
    struct FontChangeTarget;

    impl FontChangeTarget {
        #[unsafe(method(changeFont:))]
        fn change_font(&self, _sender: Option<&NSFontManager>) {
            FONT_CHANGE_CALLBACK.with(|cb| {
                if let Some(f) = cb.borrow().as_ref() {
                    f();
                }
            });
        }

    }
);

unsafe impl Send for FontChangeTarget {}
unsafe impl Sync for FontChangeTarget {}

thread_local! {
    static FONT_CHANGE_CALLBACK: std::cell::RefCell<Option<Box<dyn Fn()>>> =
        std::cell::RefCell::new(None);
}

static FONT_TARGET: OnceLock<std::sync::Mutex<Option<Retained<FontChangeTarget>>>> =
    OnceLock::new();

static WINDOW_COUNTER: AtomicU32 = AtomicU32::new(1);

fn emit_to_focused<R: tauri::Runtime>(app: &tauri::AppHandle<R>, event: &str, payload: impl serde::Serialize + Clone) {
    for (_, win) in app.webview_windows() {
        if win.is_focused().unwrap_or(false) {
            let _ = win.emit(event, payload);
            return;
        }
    }
}

// Показывает трёхкнопочный NSAlert. ДОЛЖЕН вызываться на главном потоке.
// Возврат: 0 = Сохранить, 1 = Не сохранять, 2 = Отмена
fn show_quit_alert(count: usize, lang: &str) -> isize {
    autoreleasepool(|_| {
        let plural_doc = |n: usize| -> &'static str {
            let n100 = n % 100;
            let n10 = n % 10;
            if n100 >= 11 && n100 <= 14 { "документов" }
            else if n10 == 1 { "документ" }
            else if n10 >= 2 && n10 <= 4 { "документа" }
            else { "документов" }
        };
        let (msg, info, b_save, b_dont, b_cancel) = if lang == "ru" {
            let msg = if count == 1 {
                "Имеется несохранённый документ.".to_string()
            } else {
                format!("Имеется {} несохранённых {}.", count, plural_doc(count))
            };
            (
                msg,
                "Хотите сохранить изменения перед завершением? Несохранённые изменения будут потеряны.".to_string(),
                "Сохранить…", "Не сохранять", "Отмена",
            )
        } else {
            let (n_word, doc_word) = if count == 1 { ("one", "document") } else { ("several", "documents") };
            (
                format!("You have {} {} with unsaved changes.", n_word, doc_word),
                "Do you want to save your changes before quitting? Your changes will be lost if you don't save.".to_string(),
                "Save…", "Don't Save", "Cancel",
            )
        };
        let mtm = MainThreadMarker::new().unwrap();
        let alert = NSAlert::new(mtm);
        alert.setAlertStyle(NSAlertStyle::Warning);
        alert.setMessageText(&NSString::from_str(&msg));
        alert.setInformativeText(&NSString::from_str(&info));
        alert.addButtonWithTitle(&NSString::from_str(b_save));
        alert.addButtonWithTitle(&NSString::from_str(b_dont));
        alert.addButtonWithTitle(&NSString::from_str(b_cancel));
        let response = alert.runModal();
        // NSAlertFirstButtonReturn = 1000, второй = 1001, третий = 1002
        response - 1000
    })
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
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut builder = app.dialog()
        .file()
        .add_filter("Text", &["txt", "md", "log", "csv"]);
    if let Some(dir) = &last_dir {
        builder = builder.set_directory(dir);
    }
    builder.pick_file(move |path| {
        let _ = tx.send(path);
    });
    let path = rx.await.map_err(|e| e.to_string())?;
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

#[derive(serde::Serialize, Clone)]
pub struct FontFaceInfo {
    pub name: String,
    pub weight: String,
    pub style: String,
}

#[derive(serde::Serialize, Clone)]
pub struct FontFamilyInfo {
    pub family: String,
    pub faces: Vec<FontFaceInfo>,
    pub monospaced: bool,
}

#[tauri::command]
fn get_font(app: tauri::AppHandle) -> (String, f64, String, String) {
    let cfg = config::load(&app);
    (cfg.font_name, cfg.font_size, cfg.font_weight, cfg.font_style)
}

#[tauri::command]
fn set_font(app: tauri::AppHandle, name: String, size: f64, weight: String, style: String) -> Result<(), String> {
    let mut cfg = config::load(&app);
    cfg.font_name = name.clone();
    cfg.font_size = size;
    cfg.font_weight = weight.clone();
    cfg.font_style = style.clone();
    config::save(&app, &cfg)?;
    app.emit("font-changed", (name, size, weight, style))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn apply_font_and_close(
    app: tauri::AppHandle,
    name: String,
    size: f64,
    weight: String,
    style: String,
) -> Result<(), String> {
    // 1) Сохраняем в конфиг и эмитим font-changed
    {
        let mut cfg = config::load(&app);
        cfg.font_name = name.clone();
        cfg.font_size = size;
        cfg.font_weight = weight.clone();
        cfg.font_style = style.clone();
        config::save(&app, &cfg)?;
        app.emit("font-changed", (name, size, weight, style))
            .map_err(|e| e.to_string())?;
    }

    // 2) Закрываем окно отдельной задачей, чтобы не падать в текущей invoke
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        // Небольшая задержка, чтобы текущий invoke и эмит точно завершились
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if let Some(win) = app_clone.get_webview_window("font-panel") {
            let _ = win.close();
        }
    });
    Ok(())
}

#[tauri::command]
async fn preview_font(
    app: tauri::AppHandle,
    name: String,
    size: f64,
    weight: String,
    style: String,
) -> Result<(), String> {
    app.emit("font-changed", (name, size, weight, style))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_font_list(app: tauri::AppHandle) -> Result<Vec<FontFamilyInfo>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<FontFamilyInfo>>();
    app.run_on_main_thread(move || {
        let result = autoreleasepool(|_| {
            let mtm = unsafe { MainThreadMarker::new_unchecked() };
            let fm = NSFontManager::sharedFontManager(mtm);
            let families = fm.availableFontFamilies();

            let mut list: Vec<FontFamilyInfo> = Vec::new();

            for family_ns in families.iter() {
                let family_str = family_ns.to_string();
                let mut faces: Vec<FontFaceInfo> = Vec::new();
                let mut is_monospaced = false;

                if let Some(members) = fm.availableMembersOfFontFamily(&family_ns) {
                    let count: usize = unsafe { objc2::msg_send![&*members, count] };
                    for i in 0..count {
                        unsafe {
                            let member: *mut objc2::runtime::AnyObject =
                                objc2::msg_send![&*members, objectAtIndex: i];
                            if member.is_null() { continue; }

                            let face_obj: *mut objc2_foundation::NSString =
                                objc2::msg_send![member, objectAtIndex: 1usize];
                            let traits_obj: *mut objc2::runtime::AnyObject =
                                objc2::msg_send![member, objectAtIndex: 3usize];
                            if face_obj.is_null() || traits_obj.is_null() { continue; }

                            let face_name = (*face_obj).to_string();
                            let traits: u32 = objc2::msg_send![traits_obj, unsignedIntValue];

                            if traits & 0x0400 != 0 { is_monospaced = true; }
                            let weight = if traits & 0x0002 != 0 { "bold" } else { "normal" };
                            let style  = if traits & 0x0001 != 0 { "italic" } else { "normal" };

                            faces.push(FontFaceInfo {
                                name: face_name,
                                weight: weight.to_string(),
                                style: style.to_string(),
                            });
                        }
                    }
                }

                if !faces.is_empty() {
                    list.push(FontFamilyInfo {
                        family: family_str,
                        faces,
                        monospaced: is_monospaced,
                    });
                }
            }

            list.sort_by(|a, b| a.family.to_lowercase().cmp(&b.family.to_lowercase()));
            list
        });

        let _ = tx.send(result);
    }).map_err(|e| e.to_string())?;

    rx.await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_font_panel(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("font-panel") {
        win.show().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let theme = config::load(&app).theme;
    let url_str = format!("font-panel.html?theme={}", theme);

    let mut builder = tauri::WebviewWindowBuilder::new(
        &app,
        "font-panel",
        tauri::WebviewUrl::App(std::path::PathBuf::from(url_str)),
    )
    .title("Шрифт")
    .inner_size(460.0, 380.0)
    .resizable(false)
    .minimizable(false)
    .visible(false);

    if let Some(parent) = app.webview_windows()
        .into_iter()
        .find(|(label, w)| label != "font-panel" && w.is_focused().unwrap_or(false))
        .map(|(_, w)| w)
    {
        builder = builder.parent(&parent).map_err(|e| e.to_string())?;
    }

    builder.build().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn close_font_panel(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("font-panel") {
        win.close().map_err(|e| e.to_string())?;
    }
    Ok(())
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
fn set_dirty(window: tauri::Window, dirty: bool) {
    window.state::<DirtyState>()
        .0.lock().unwrap()
        .insert(window.label().to_string(), dirty);
}

#[tauri::command]
fn confirm_close(window: tauri::Window) {
    window.state::<ConfirmedWindowsState>()
        .0.lock().unwrap().insert(window.label().to_string());
    let _ = window.close();
}

fn advance_quit(app: &AppHandle) {
    let next = {
        let state = app.state::<QuitState>();
        let mut guard = state.0.lock().unwrap();
        if guard.cancelled { return; }
        guard.queue.pop()
    };
    match next {
        Some(label) => {
            let app_clone = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(win) = app_clone.webview_windows().get(&label) {
                    let _ = win.set_focus();
                    let _ = app_clone.emit_to(label.as_str(), "quit-save-window", ());
                } else {
                    advance_quit(&app_clone);
                }
            });
        }
        None => {
            let app_clone = app.clone();
            let _ = app.run_on_main_thread(move || {
                app_clone.exit(0);
            });
        }
    }
}

#[tauri::command]
fn confirm_quit_window(app: AppHandle) {
    advance_quit(&app);
}

#[tauri::command]
fn cancel_quit(app: AppHandle) {
    let state = app.state::<QuitState>();
    let mut guard = state.0.lock().unwrap();
    guard.cancelled = true;
    guard.queue.clear();
}

#[tauri::command]
async fn save_file_as(app: AppHandle, content: String, suggested_name: String) -> Result<Option<String>, String> {
    let last_dir = config::load(&app).last_dir;
    let (tx, rx) = tokio::sync::oneshot::channel();
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
    let path = rx.await.map_err(|e| e.to_string())?;
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

#[tauri::command]
fn print_document(app: AppHandle, text: String, filename: String) {
    let _ = app.run_on_main_thread(move || {
        autoreleasepool(|_| {
            let mtm = unsafe { MainThreadMarker::new_unchecked() };
            unsafe {
                let font_name = NSString::from_str("Menlo");
                let font = NSFont::fontWithName_size(&font_name, 12.0)
                    .unwrap_or_else(|| NSFont::userFixedPitchFontOfSize(12.0).unwrap());
                let ns_text = NSString::from_str(&text);
                let attr_str = NSAttributedString::from_nsstring(&ns_text);
                let mut_attr = NSMutableAttributedString::initWithAttributedString(
                    NSMutableAttributedString::alloc(),
                    &attr_str,
                );
                let full_range = objc2_foundation::NSRange::new(0, ns_text.length());
                let font_key = NSString::from_str("NSFont");
                mut_attr.addAttribute_value_range(&font_key, font.as_ref(), full_range);
                let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(468.0, 648.0));
                let text_view = NSTextView::initWithFrame(NSTextView::alloc(mtm), frame);
                text_view.textStorage().unwrap().setAttributedString(&mut_attr);
                let op = NSPrintOperation::printOperationWithView(&text_view);
                let job_title = NSString::from_str(&filename);
                op.setJobTitle(Some(&job_title));
                op.setShowsPrintPanel(true);
                op.setShowsProgressPanel(true);
                let key_win = NSApplication::sharedApplication(mtm).keyWindow();
                if let Some(win) = key_win {
                    op.runOperationModalForWindow_delegate_didRunSelector_contextInfo(
                        &win,
                        None,
                        None,
                        std::ptr::null_mut(),
                    );
                } else {
                    op.runOperation();
                }
            }
        });
    });
}

#[tauri::command]
fn page_setup(app: AppHandle) {
    let _ = app.run_on_main_thread(move || {
        autoreleasepool(|_| {
            let mtm = unsafe { MainThreadMarker::new_unchecked() };
            let layout = NSPageLayout::pageLayout(mtm);
            layout.runModal();
        });
    });
}

#[tauri::command]
fn show_font_panel(app: tauri::AppHandle) {
    let cfg = config::load(&app);
    let font_name = cfg.font_name.clone();
    let font_size = cfg.font_size;
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        autoreleasepool(|_| {
            let mtm = unsafe { MainThreadMarker::new_unchecked() };
            unsafe {
                let fm = NSFontManager::sharedFontManager(mtm);

                let store = FONT_TARGET.get_or_init(|| std::sync::Mutex::new(None));
                let mut guard = store.lock().unwrap();
                if guard.is_none() {
                    *guard = Some(objc2::msg_send![FontChangeTarget::class(), new]);
                }
                let target = guard.as_ref().unwrap();

                fm.setTarget(Some(target.as_ref()));

                let app3 = app2.clone();
                FONT_CHANGE_CALLBACK.with(|cb| {
                    *cb.borrow_mut() = Some(Box::new(move || {
                        let mtm2 = MainThreadMarker::new_unchecked();
                        let fm2 = NSFontManager::sharedFontManager(mtm2);
                        if let Some(font) = fm2.selectedFont() {
                            let new_name = font.familyName()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "Menlo".to_string());
                            let new_size = font.pointSize();
                            let traits = fm2.traitsOfFont(&font);
                            let is_bold = traits & objc2_app_kit::NSFontTraitMask(2) != objc2_app_kit::NSFontTraitMask(0);
                            let is_italic = traits & objc2_app_kit::NSFontTraitMask(1) != objc2_app_kit::NSFontTraitMask(0);
                            let new_weight = if is_bold { "bold" } else { "normal" }.to_string();
                            let new_style = if is_italic { "italic" } else { "normal" }.to_string();
                            let mut cfg2 = config::load(&app3);
                            cfg2.font_name = new_name.clone();
                            cfg2.font_size = new_size;
                            cfg2.font_weight = new_weight.clone();
                            cfg2.font_style = new_style.clone();
                            let _ = config::save(&app3, &cfg2);
                            let _ = app3.emit("font-changed", (&new_name, new_size, &new_weight, &new_style));
                        }
                    }));
                });

                let ns_name = NSString::from_str(&font_name);
                if let Some(font) = NSFont::fontWithName_size(&ns_name, font_size) {
                    fm.setSelectedFont_isMultiple(&font, false);
                }
                fm.orderFrontFontPanel(None);
            }
        });
    });
}

struct RecentFilesState(std::sync::Mutex<Vec<String>>);
struct RecentMenuState(std::sync::Mutex<Option<tauri::menu::Submenu<tauri::Wry>>>);
struct ConfirmedWindowsState(std::sync::Mutex<std::collections::HashSet<String>>);
struct DirtyState(std::sync::Mutex<std::collections::HashMap<String, bool>>);
struct QuitProgress {
    queue: Vec<String>,
    cancelled: bool,
}
struct QuitState(std::sync::Mutex<QuitProgress>);

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
        .manage(ConfirmedWindowsState(std::sync::Mutex::new(std::collections::HashSet::new())))
        .manage(DirtyState(std::sync::Mutex::new(std::collections::HashMap::new())))
        .manage(QuitState(std::sync::Mutex::new(QuitProgress { queue: Vec::new(), cancelled: false })))
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
            let quit_item = MenuItem::with_id(handle, "quit", quit_label, true, Some("CmdOrCtrl+Q"))?;
            let app_menu = Submenu::with_items(handle, "LikeNotepad.exe", true, &[&about_item, &sep_app, &quit_item])?;

            let new_item = MenuItem::with_id(handle, "new", if lang == "ru" { "Создать" } else { "New" }, true, Some("CmdOrCtrl+N"))?;
            let open_item = MenuItem::with_id(handle, "open", if lang == "ru" { "Открыть..." } else { "Open..." }, true, Some("CmdOrCtrl+O"))?;
            let recent_label = if lang == "ru" { "Открыть недавние" } else { "Open Recent" };
            let recent_submenu = Submenu::with_id(handle, "recent_submenu", recent_label, true)?;
            let save_item = MenuItem::with_id(handle, "save", if lang == "ru" { "Сохранить" } else { "Save" }, true, Some("CmdOrCtrl+S"))?;
            let save_as_item = MenuItem::with_id(handle, "save_as", if lang == "ru" { "Сохранить как..." } else { "Save As..." }, true, Some("CmdOrCtrl+Shift+S"))?;
            let sep = PredefinedMenuItem::separator(handle)?;
            let sep_before_print = PredefinedMenuItem::separator(handle)?;
            let page_setup_item = MenuItem::with_id(handle, "page_setup", if lang == "ru" { "Параметры страницы..." } else { "Page Setup..." }, true, Some("CmdOrCtrl+Shift+P"))?;
            let print_item = MenuItem::with_id(handle, "print", if lang == "ru" { "Печать..." } else { "Print..." }, true, Some("CmdOrCtrl+P"))?;
            let sep_close = PredefinedMenuItem::separator(handle)?;
            let close_item = MenuItem::with_id(handle, "close_window", if lang == "ru" { "Закрыть" } else { "Close" }, true, Some("CmdOrCtrl+W"))?;

            let file_label = if lang == "ru" { "Файл" } else { "File" };
            let file_menu = Submenu::with_items(handle, file_label, true, &[
                &new_item,
                &open_item,
                &recent_submenu,
                &sep,
                &save_item,
                &save_as_item,
                &sep_before_print,
                &print_item,
                &page_setup_item,
                &sep_close,
                &close_item,
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
            let goto_item = MenuItem::with_id(handle, "edit_goto", if lang == "ru" { "Перейти..." } else { "Go to..." }, true, Some("CmdOrCtrl+L"))?;
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
                &goto_item,
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

            let font_panel_item = MenuItem::with_id(handle, "font_panel", if lang == "ru" { "Шрифт\u{2026}" } else { "Font\u{2026}" }, true, Some("cmd+t"))?;
            let sep_format = PredefinedMenuItem::separator(handle)?;
            let format_label = if lang == "ru" { "Формат" } else { "Format" };
            let format_menu = Submenu::with_items(handle, format_label, true, &[&font_panel_item, &sep_format, &wrap_item])?;

            let help_label = if lang == "ru" { "Справка" } else { "Help" };
            let help_menu = Submenu::with_id_and_items(handle, HELP_SUBMENU_ID, help_label, true, &[])?;

            let menu = Menu::with_items(handle, &[&app_menu, &file_menu, &edit_menu, &format_menu, &view_menu, &help_menu])?;
            app.set_menu(menu)?;
            rebuild_recent_menu(handle);

            // Handle menu events
            app.on_menu_event(move |app, event| {
                match event.id().as_ref() {
                    "quit" => {
                        let dirty_labels: Vec<String> = {
                            let dirty = app.state::<DirtyState>();
                            let guard = dirty.0.lock().unwrap();
                            let open: std::collections::HashSet<String> =
                                app.webview_windows().keys().cloned().collect();
                            let mut labels: Vec<String> = guard.iter()
                                .filter(|(label, &is_dirty)| is_dirty && open.contains(*label))
                                .map(|(label, _)| label.clone())
                                .collect();
                            let order = |l: &str| -> u32 {
                                if l == "main" { 0 }
                                else { l.strip_prefix("win-").and_then(|n| n.parse().ok()).unwrap_or(u32::MAX) }
                            };
                            labels.sort_by_key(|l| order(l));
                            labels.reverse();
                            labels
                        };

                        if dirty_labels.is_empty() {
                            app.exit(0);
                            return;
                        }

                        let count = dirty_labels.len();
                        let lang = system_lang();
                        let app_clone = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            let choice = show_quit_alert(count, lang);
                            match choice {
                                0 => {
                                    // Сохранить — обойти грязные окна по очереди
                                    let rest: Vec<String> = dirty_labels[1..].iter().cloned().rev().collect();
                                    {
                                        let state = app_clone.state::<QuitState>();
                                        let mut guard = state.0.lock().unwrap();
                                        *guard = QuitProgress { queue: rest, cancelled: false };
                                    }
                                    if let Some(win) = app_clone.webview_windows().get(&dirty_labels[0]) {
                                        let _ = win.set_focus();
                                        let _ = app_clone.emit_to(dirty_labels[0].as_str(), "quit-save-window", ());
                                    }
                                }
                                1 => { app_clone.exit(0); }
                                _ => { /* Отмена — ничего */ }
                            }
                        });
                    }
                    "new" => { new_window(app); }
                    "open" => { emit_to_focused(app, "menu-open", ()); }
                    "save" => { emit_to_focused(app, "menu-save", ()); }
                    "save_as"    => { emit_to_focused(app, "menu-save-as", ()); }
                    "page_setup" => { emit_to_focused(app, "menu-page-setup", ()); }
                    "print"      => { emit_to_focused(app, "menu-print", ()); }
                    "close_window" => { emit_to_focused(app, "menu-close-window", ()); }
                    "font_panel" => { emit_to_focused(app, "menu-font-panel", ()); }
                    "edit_undo"       => { emit_to_focused(app, "menu-edit", "undo"); }
                    "edit_redo"       => { emit_to_focused(app, "menu-edit", "redo"); }
                    "edit_cut"        => { emit_to_focused(app, "menu-edit", "cut"); }
                    "edit_copy"       => { emit_to_focused(app, "menu-edit", "copy"); }
                    "edit_paste"      => { emit_to_focused(app, "menu-edit", "paste"); }
                    "edit_select_all" => { emit_to_focused(app, "menu-edit", "select-all"); }
                    "edit_find" => { emit_to_focused(app, "menu-edit", "find"); }
                    "edit_find_next" => { emit_to_focused(app, "menu-edit", "find-next"); }
                    "edit_replace" => { emit_to_focused(app, "menu-edit", "replace"); }
                    "edit_goto" => { emit_to_focused(app, "menu-edit", "goto"); }
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
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "font-panel" {
                    return;
                }
                let label = window.label().to_string();
                let confirmed = window.state::<ConfirmedWindowsState>()
                    .0.lock().unwrap().contains(&label);
                if confirmed { return; }
                api.prevent_close();
                let _ = window.emit("close-requested", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_lang,
            open_file,
            save_file,
            save_file_as,
            get_theme,
            set_theme,
            get_font,
            set_font,
            apply_font_and_close,
            preview_font,
            get_font_list,
            open_font_panel,
            close_font_panel,
            get_word_wrap,
            set_word_wrap,
            get_status_bar,
            set_status_bar,
            set_dirty,
            confirm_close,
            print_document,
            page_setup,
            show_font_panel,
            confirm_quit_window,
            cancel_quit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
