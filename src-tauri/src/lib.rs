mod config;
mod i18n;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tauri::{
    menu::{AboutMetadata, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID},
    AppHandle, Emitter, Manager,
};
use tauri_plugin_dialog::{DialogExt, FilePath};
use objc2::define_class;
use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::{AnyClass, AnyObject, NSObject};
use objc2::{class, msg_send, sel};
use block2::Block;
use objc2::{AnyThread, ClassType, MainThreadMarker, MainThreadOnly};
use std::sync::OnceLock;
use objc2_app_kit::{NSAlert, NSAlertStyle, NSApplication, NSFont, NSFontManager, NSPageLayout, NSPrintOperation, NSTextView, NSWindow, NSWindowTabbingMode, NSWindowOrderingMode};
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
            let label = win.label().to_string();
            let _ = win.emit_to(&label, event, payload);
            return;
        }
    }
}

// Shows a three-button NSAlert. MUST be called on the main thread.
// Returns: 0 = Save, 1 = Don't Save, 2 = Cancel
fn show_quit_alert(count: usize, lang: &str) -> isize {
    autoreleasepool(|_| {
        let msg = i18n::quit_alert_msg(lang, count);
        let info = i18n::mt(lang, "quit_info");
        let b_save = i18n::mt(lang, "alert_save");
        let b_dont = i18n::mt(lang, "alert_dont_save");
        let b_cancel = i18n::mt(lang, "alert_cancel");
        let mtm = MainThreadMarker::new().unwrap();
        let alert = NSAlert::new(mtm);
        alert.setAlertStyle(NSAlertStyle::Warning);
        alert.setMessageText(&NSString::from_str(&msg));
        alert.setInformativeText(&NSString::from_str(info));
        alert.addButtonWithTitle(&NSString::from_str(b_save));
        alert.addButtonWithTitle(&NSString::from_str(b_dont));
        let cancel_btn = alert.addButtonWithTitle(&NSString::from_str(b_cancel));
        cancel_btn.setKeyEquivalent(&NSString::from_str("\u{1b}"));
        let response = alert.runModal();
        // NSAlertFirstButtonReturn = 1000, second = 1001, third = 1002
        response - 1000
    })
}

// Shows a three-button NSAlert for closing ONE window (TextEdit style).
// MUST be called on the main thread.
// Returns: 0 = Save, 1 = Don't Save, 2 = Cancel
fn show_close_alert(name: &str, lang: &str) -> isize {
    autoreleasepool(|_| {
        let msg = i18n::close_alert_msg(lang, name);
        let info = i18n::mt(lang, "close_info");
        let b_save = i18n::mt(lang, "alert_save");
        let b_dont = i18n::mt(lang, "alert_dont_save");
        let b_cancel = i18n::mt(lang, "alert_cancel");
        let mtm = MainThreadMarker::new().unwrap();
        let alert = NSAlert::new(mtm);
        alert.setAlertStyle(NSAlertStyle::Warning);
        alert.setMessageText(&NSString::from_str(&msg));
        alert.setInformativeText(&NSString::from_str(info));
        alert.addButtonWithTitle(&NSString::from_str(b_save));
        alert.addButtonWithTitle(&NSString::from_str(b_dont));
        let cancel_btn = alert.addButtonWithTitle(&NSString::from_str(b_cancel));
        cancel_btn.setKeyEquivalent(&NSString::from_str("\u{1b}"));
        let response = alert.runModal();
        response - 1000
    })
}

// ---- One-time offer to open .txt files with LikeNotepad.exe ----
// No alert of our own: macOS itself asks "Open .txt with LikeNotepad.exe or keep
// <current app>?" when an app requests to become the default for a type another
// app owns. Asked once per install, only from a real .app bundle (a dev binary
// would register itself).

#[link(name = "UniformTypeIdentifiers", kind = "framework")]
extern "C" {}

fn offer_default_for_txt(app: &AppHandle) {
    let mut cfg = config::load(app);
    if cfg.default_txt_asked { return; }
    autoreleasepool(|_| unsafe {
        let bundle: Retained<AnyObject> = msg_send![class!(NSBundle), mainBundle];
        let app_url: Retained<AnyObject> = msg_send![&*bundle, bundleURL];
        let app_path: Retained<NSString> = msg_send![&*app_url, path];
        let app_path = app_path.to_string();
        if !app_path.ends_with(".app") { return; }
        let ws: Retained<AnyObject> = msg_send![class!(NSWorkspace), sharedWorkspace];
        let supported: bool = msg_send![
            &*ws,
            respondsToSelector: sel!(setDefaultApplicationAtURL:toOpenContentType:completionHandler:)
        ];
        let Some(ut_class) = AnyClass::get(c"UTType") else { return };
        if !supported { return; }

        cfg.default_txt_asked = true;
        let _ = config::save(app, &cfg);

        let ext = NSString::from_str("txt");
        let ty: Option<Retained<AnyObject>> = msg_send![ut_class, typeWithFilenameExtension: &*ext];
        let Some(ty) = ty else { return };
        let handler: Option<Retained<AnyObject>> = msg_send![&*ws, URLForApplicationToOpenContentType: &*ty];
        let handler_path = handler.and_then(|url| {
            let p: Option<Retained<NSString>> = msg_send![&*url, path];
            p.map(|p| p.to_string())
        });
        if handler_path.as_deref() == Some(app_path.as_str()) { return; }

        let no_handler: Option<&Block<dyn Fn(*mut AnyObject)>> = None;
        let _: () = msg_send![
            &*ws,
            setDefaultApplicationAtURL: &*app_url,
            toOpenContentType: &*ty,
            completionHandler: no_handler
        ];
    });
}

fn disable_tabbing(window: &tauri::WebviewWindow) {
    if let Ok(ptr) = window.ns_window() {
        unsafe {
            let ns_window = &*(ptr as *const NSWindow);
            ns_window.setTabbingMode(NSWindowTabbingMode::Disallowed);
        }
    }
}

#[tauri::command]
fn set_tab_title(window: tauri::Window, title: String) {
    if let Ok(ptr) = window.ns_window() {
        unsafe {
            let ns_window = &*(ptr as *const NSWindow);
            let tab = &ns_window.tab();
            let ns_title = objc2_foundation::NSString::from_str(&title);
            tab.setTitle(Some(&ns_title));
        }
    }
}

// Proxy icon in the title bar (drag the file, Cmd-click for its folders), like
// TextEdit: shown for a saved file, removed for an untitled document.
#[tauri::command]
fn set_represented_file(window: tauri::Window, path: Option<String>) {
    if let Ok(ptr) = window.ns_window() {
        unsafe {
            let ns_window = &*(ptr as *const NSWindow);
            let url: Option<Retained<AnyObject>> = path.map(|p| {
                let ns_path = NSString::from_str(&p);
                msg_send![class!(NSURL), fileURLWithPath: &*ns_path]
            });
            let _: () = msg_send![ns_window, setRepresentedURL: url.as_deref()];
        }
    }
}

fn set_tabbing_preferred(window: &tauri::WebviewWindow) {
    if let Ok(ptr) = window.ns_window() {
        unsafe {
            let ns_window = &*(ptr as *const NSWindow);
            ns_window.setTabbingMode(NSWindowTabbingMode::Preferred);
        }
    }
}

fn attach_as_tab(host: &tauri::WebviewWindow, new_win: &tauri::WebviewWindow) {
    if let (Ok(hptr), Ok(nptr)) = (host.ns_window(), new_win.ns_window()) {
        unsafe {
            let host_win = &*(hptr as *const NSWindow);
            let new_win_ns = &*(nptr as *const NSWindow);
            host_win.addTabbedWindow_ordered(new_win_ns, NSWindowOrderingMode::Above);
        }
    }
}

fn cascade_window(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    if let Ok(ptr) = window.ns_window() {
        unsafe {
            let ns_window = &*(ptr as *const NSWindow);
            let state = app.state::<CascadeState>();
            let mut guard = state.0.lock().unwrap();

            let start = match *guard {
                // continue the cascade from the saved point
                Some((x, y)) => NSPoint::new(x, y),
                // first point of the stack: the main window's top-left in Cocoa coordinates
                None => {
                    let base_frame = if let Some(main) = app.get_webview_window("main") {
                        main.ns_window().ok().map(|mptr| (*(mptr as *const NSWindow)).frame())
                    } else {
                        None
                    };
                    // If main is unavailable (cold start from Finder: main is not
                    // created yet), use the NEW window's own frame as the base: it
                    // sits at the same default position where the future main
                    // will appear. NSPoint(0,0) does not work: cascadeTopLeftFromPoint
                    // with a zero point does not move the window at all.
                    let f = base_frame.unwrap_or_else(|| ns_window.frame());
                    // frame.origin = bottom-left; top-left.y = origin.y + height.
                    // Offset by one cascade step (+25,-25 in Cocoa: right and down
                    // on screen), otherwise the first window lands exactly on top of the base one.
                    NSPoint::new(f.origin.x + 25.0, f.origin.y + f.size.height - 25.0)
                }
            };

            let next = ns_window.cascadeTopLeftFromPoint(start);
            *guard = Some((next.x, next.y));
        }
    }
}

fn new_window(app: &tauri::AppHandle) {
    let use_tabs = crate::config::load(app).use_tabs;

    // Find the host window BEFORE build: focus moves once the new window is created.
    let host = if use_tabs {
        app.webview_windows()
            .into_iter()
            .find(|(_, w)| w.is_focused().unwrap_or(false))
            .map(|(_, w)| w)
    } else {
        None
    };

    let n = WINDOW_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    let label = format!("win-{}", n);

    match tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App("index.html".into()))
        .title("LikeNotepad.exe")
        .inner_size(800.0, 600.0)
        .min_inner_size(400.0, 300.0)
        .resizable(true)
        .decorations(true)
        .visible(false)
        .build()
    {
        Ok(win) => {
            if use_tabs {
                set_tabbing_preferred(&win);
                match host {
                    Some(h) => {
                        set_tabbing_preferred(&h);
                        attach_as_tab(&h, &win);
                    }
                    // no focused window (first window): standalone, it becomes the host
                    None => {}
                }
            } else {
                disable_tabbing(&win);
                cascade_window(app, &win);
            }
        }
        Err(e) => eprintln!("new_window error: {e}"),
    }
}

fn new_standalone_window(app: &tauri::AppHandle) {
    let n = WINDOW_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    let label = format!("win-{}", n);

    match tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App("index.html".into()))
        .title("LikeNotepad.exe")
        .inner_size(800.0, 600.0)
        .min_inner_size(400.0, 300.0)
        .resizable(true)
        .decorations(true)
        .visible(false)
        .build()
    {
        Ok(win) => {
            // Preferred (not Disallowed): the window can accept tabs as
            // the host of its group. On its own the tab bar is hidden.
            set_tabbing_preferred(&win);
            cascade_window(app, &win);
        }
        Err(e) => eprintln!("new_standalone_window error: {e}"),
    }
}

fn open_file_in_new_window(app: &tauri::AppHandle, path: String, content: String) {
    let use_tabs = crate::config::load(app).use_tabs;
    let host = if use_tabs {
        app.webview_windows()
            .into_iter()
            .find(|(_, w)| w.is_focused().unwrap_or(false))
            .map(|(_, w)| w)
    } else {
        None
    };

    let n = WINDOW_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    let label = format!("win-{}", n);

    match tauri::WebviewWindowBuilder::new(app, label.clone(), tauri::WebviewUrl::App("index.html".into()))
        .title("LikeNotepad.exe")
        .inner_size(800.0, 600.0)
        .min_inner_size(400.0, 300.0)
        .resizable(true)
        .decorations(true)
        .visible(false)
        .build()
    {
        Ok(win) => {
            app.state::<PendingFileState>().0.lock().unwrap().insert(label, (path, content));
            if use_tabs {
                set_tabbing_preferred(&win);
                if let Some(h) = host {
                    set_tabbing_preferred(&h);
                    attach_as_tab(&h, &win);
                }
            } else {
                disable_tabbing(&win);
                cascade_window(app, &win);
            }
        }
        Err(e) => eprintln!("open_file_in_new_window error: {e}"),
    }
}

fn handle_opened_urls(app: &tauri::AppHandle, urls: Vec<tauri::Url>) {
    let mut first = true;
    for url in urls {
        let Ok(path) = url.to_file_path() else {
            eprintln!("[opened] skip non-file url: {url}");
            continue;
        };
        let path_str = path.to_string_lossy().to_string();
        let content = match read_file_content(&path_str) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[opened] read error: {e}");
                continue;
            }
        };
        add_recent(app, &path_str);

        if first {
            first = false;
            // Only a cold start (main does not exist yet) loads
            // the file straight into main. If main is already open (even empty), always
            // a new window, like TextEdit. An empty main is left alone on purpose.
            if app.webview_windows().is_empty() {
                app.state::<PendingFileState>()
                    .0
                    .lock()
                    .unwrap()
                    .insert("main".to_string(), (path_str, content));
                continue;
            }
        }
        open_file_in_new_window(app, path_str, content);
    }
}

// Language detection lives in i18n.rs, next to the translation tables.
use i18n::system_lang;

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

#[tauri::command]
async fn open_file_new_window(app: AppHandle) -> Result<(), String> {
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
    if let Some(FilePath::Path(p)) = path {
        let path_str = p.to_string_lossy().to_string();
        let content = read_file_content(&path_str)?;
        add_recent(&app, &path_str);
        let app2 = app.clone();
        app.run_on_main_thread(move || {
            open_file_in_new_window(&app2, path_str, content);
        }).map_err(|e| e.to_string())?;
    }
    Ok(())
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
    // 1) Save to the config and emit font-changed
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

    // 2) Close the window in a separate task so the current invoke does not crash
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        // A short delay so the current invoke and emit are sure to finish
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
    .title(i18n::mt(system_lang(), "font_title"))
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

fn open_settings_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(win) = app.get_webview_window("settings") {
        win.show()?;
        win.set_focus()?;
        return Ok(());
    }
    let theme = config::load(app).theme;
    let url_str = format!("settings.html?theme={}", theme);
    let mut builder = tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App(std::path::PathBuf::from(url_str)),
    )
    .title(i18n::mt(system_lang(), "settings_title"))
    .inner_size(440.0, 320.0)
    .resizable(false)
    .minimizable(false)
    .visible(false);

    if let Some(parent) = app.webview_windows()
        .into_iter()
        .find(|(label, w)| label != "settings" && w.is_focused().unwrap_or(false))
        .map(|(_, w)| w)
    {
        builder = builder.parent(&parent)?;
    }

    builder.build()?;
    Ok(())
}

#[tauri::command]
fn take_pending_file(window: tauri::WebviewWindow) -> Option<(String, String)> {
    let app = window.app_handle();
    app.state::<PendingFileState>().0.lock().unwrap().remove(window.label())
}

#[tauri::command]
async fn close_settings(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("settings") {
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
fn get_use_tabs(app: tauri::AppHandle) -> bool {
    config::load(&app).use_tabs
}

#[tauri::command]
fn set_use_tabs(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let mut cfg = config::load(&app);
    cfg.use_tabs = enabled;
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
fn get_show_counter(app: tauri::AppHandle) -> bool {
    config::load(&app).show_counter
}

#[tauri::command]
fn set_show_counter(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let mut cfg = config::load(&app);
    cfg.show_counter = enabled;
    if enabled && !cfg.status_bar {
        cfg.status_bar = true;
        config::save(&app, &cfg)?;
        app.state::<StatusMenuState>().flag.store(true, Ordering::Relaxed);
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            let sms = app2.state::<StatusMenuState>();
            if let Some(item) = sms.item.lock().unwrap().as_ref() {
                let _ = item.set_checked(true);
            };
        });
        emit_to_focused(&app, "menu-status-bar", true);
    } else {
        config::save(&app, &cfg)?;
    }
    emit_to_focused(&app, "menu-show-counter", enabled);
    Ok(())
}

#[tauri::command]
fn get_auto_name(app: tauri::AppHandle) -> bool {
    config::load(&app).auto_name
}

#[tauri::command]
fn set_auto_name(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let mut cfg = config::load(&app);
    cfg.auto_name = enabled;
    if enabled && !cfg.status_bar {
        cfg.status_bar = true;
        config::save(&app, &cfg)?;
        app.state::<StatusMenuState>().flag.store(true, Ordering::Relaxed);
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            let sms = app2.state::<StatusMenuState>();
            if let Some(item) = sms.item.lock().unwrap().as_ref() {
                let _ = item.set_checked(true);
            };
        });
        emit_to_focused(&app, "menu-status-bar", true);
    } else {
        config::save(&app, &cfg)?;
    }
    emit_to_focused(&app, "menu-auto-name", enabled);
    Ok(())
}

#[tauri::command]
fn set_dirty(window: tauri::Window, dirty: bool) {
    window.state::<DirtyState>()
        .0.lock().unwrap()
        .insert(window.label().to_string(), dirty);
    if let Ok(ptr) = window.ns_window() {
        unsafe {
            let ns_window = &*(ptr as *const NSWindow);
            ns_window.setDocumentEdited(dirty);
        }
    }
}

#[tauri::command]
fn claim_untitled_number(window: tauri::Window) -> u32 {
    let state = window.state::<UntitledState>();
    let mut guard = state.0.lock().unwrap();
    let label = window.label().to_string();
    // if the window already has a number, return it (idempotent)
    if let Some(n) = guard.get(&label) {
        return *n;
    }
    // find the smallest free number starting from 1
    let used: std::collections::HashSet<u32> = guard.values().copied().collect();
    let mut n = 1;
    while used.contains(&n) {
        n += 1;
    }
    guard.insert(label, n);
    n
}

#[tauri::command]
fn release_untitled_number(window: tauri::Window) {
    let state = window.state::<UntitledState>();
    state.0.lock().unwrap().remove(window.label());
}

#[tauri::command]
async fn confirm_close_dirty(app: AppHandle, name: String, lang: String) -> Result<isize, String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<isize>();
    let _ = app.run_on_main_thread(move || {
        let choice = show_close_alert(&name, &lang);
        let _ = tx.send(choice);
    });
    rx.await.map_err(|e| e.to_string())
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

struct UntitledState(std::sync::Mutex<std::collections::HashMap<String, u32>>);
struct CascadeState(std::sync::Mutex<Option<(f64, f64)>>);
struct PendingFileState(std::sync::Mutex<std::collections::HashMap<String, (String, String)>>);
struct QuitProgress {
    queue: Vec<String>,
    cancelled: bool,
}
struct QuitState(std::sync::Mutex<QuitProgress>);
struct StatusMenuState {
    item: std::sync::Mutex<Option<CheckMenuItem<tauri::Wry>>>,
    flag: AtomicBool,
}

fn add_recent(app: &tauri::AppHandle, path: &str) {
    // update state: drop the duplicate, insert at the front, cap at 10
    {
        let state = app.state::<RecentFilesState>();
        let mut guard = state.0.lock().unwrap();
        guard.retain(|p| p != path);
        guard.insert(0, path.to_string());
        guard.truncate(10);
    }
    // update last_dir to the file's folder
    let last_dir = std::path::Path::new(path)
        .parent()
        .map(|p| p.to_string_lossy().to_string());
    // write to the config (recent + last_dir)
    let mut cfg = config::load(app);
    cfg.recent_files = app.state::<RecentFilesState>().0.lock().unwrap().clone();
    if last_dir.is_some() { cfg.last_dir = last_dir; }
    let _ = config::save(app, &cfg);
    // rebuild the menu
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
    // clear
    while let Ok(Some(_)) = submenu.remove_at(0) {}
    let lang = system_lang();
    if recent.is_empty() {
        let empty = MenuItem::with_id(app, "recent_empty", i18n::mt(lang, "no_recent"), false, None::<&str>).unwrap();
        let _ = submenu.append(&empty);
        return;
    }
    // items recent-0..N with the file name
    for (i, path) in recent.iter().enumerate() {
        let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.clone());
        let item = MenuItem::with_id(app, format!("recent-{}", i), name, true, None::<&str>).unwrap();
        let _ = submenu.append(&item);
    }
    let sep = PredefinedMenuItem::separator(app).unwrap();
    let _ = submenu.append(&sep);
    let clear = MenuItem::with_id(app, "clear_recent", i18n::mt(lang, "clear_recent"), true, None::<&str>).unwrap();
    let _ = submenu.append(&clear);
}

// UI tests (tests/ui): in debug builds only, LIKENOTEPAD_TEST_HOOK names a JS file that
// runs in every webview before the page's own scripts, with __LIKENOTEPAD_TEST__ set so
// app.js exposes the few internals the tests drive. Release builds never read it.
#[cfg(debug_assertions)]
fn with_test_hook(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    let Ok(path) = std::env::var("LIKENOTEPAD_TEST_HOOK") else { return builder };
    let hook = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("LIKENOTEPAD_TEST_HOOK {path}: {e}"));
    builder.plugin(
        tauri::plugin::Builder::<tauri::Wry>::new("test-hook")
            .js_init_script(format!("window.__LIKENOTEPAD_TEST__ = true;\n{hook}"))
            .build(),
    )
}

pub fn run() {
    let lang = system_lang();

    let builder = tauri::Builder::default();
    #[cfg(debug_assertions)]
    let builder = with_test_hook(builder);

    builder
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(RecentFilesState(std::sync::Mutex::new(Vec::new())))
        .manage(RecentMenuState(std::sync::Mutex::new(None)))
        .manage(ConfirmedWindowsState(std::sync::Mutex::new(std::collections::HashSet::new())))
        .manage(DirtyState(std::sync::Mutex::new(std::collections::HashMap::new())))
        .manage(UntitledState(std::sync::Mutex::new(std::collections::HashMap::new())))
        .manage(CascadeState(std::sync::Mutex::new(None)))
        .manage(PendingFileState(std::sync::Mutex::new(std::collections::HashMap::new())))
        .manage(QuitState(std::sync::Mutex::new(QuitProgress { queue: Vec::new(), cancelled: false })))
            .manage(StatusMenuState { item: std::sync::Mutex::new(None), flag: AtomicBool::new(false) })
        .setup(move |app| {
            let handle = app.handle();
            if let Some(main) = app.get_webview_window("main") {
                disable_tabbing(&main);
            }
            let mut cfg = config::load(handle);
            let saved_wrap = cfg.word_wrap;
            let saved_status = cfg.status_bar;
            cfg.recent_files.retain(|p| std::path::Path::new(p).exists());
            let _ = config::save(handle, &cfg);
            *app.state::<RecentFilesState>().0.lock().unwrap() = cfg.recent_files.clone();

            // Build native menu
            let about_label = i18n::mt(lang, "about");
            let comments = i18n::mt(lang, "about_comments");
            let about_item = PredefinedMenuItem::about(handle, Some(about_label), Some(AboutMetadata {
                name:          Some("LikeNotepad.exe".to_string()),
                version:       Some("0.1.0".to_string()),
                authors:       Some(vec!["Timur Polatov".to_string()]),
                copyright:     Some("Timur Polatov".to_string()),
                website:       Some("https://polatov.me".to_string()),
                website_label: Some("https://polatov.me".to_string()),
                comments:      Some(comments.to_string()),
                credits:       Some(comments.to_string()),
                ..Default::default()
            }))?;
            let sep_app = PredefinedMenuItem::separator(handle)?;
            let quit_label = i18n::mt(lang, "quit");
            let quit_item = MenuItem::with_id(handle, "quit", quit_label, true, Some("CmdOrCtrl+Q"))?;
            let settings_label = i18n::mt(lang, "settings");
            let settings_item = MenuItem::with_id(handle, "settings", settings_label, true, Some("CmdOrCtrl+Comma"))?;
            let sep_settings = PredefinedMenuItem::separator(handle)?;
            let app_menu = Submenu::with_items(handle, "LikeNotepad.exe", true, &[&about_item, &sep_app, &settings_item, &sep_settings, &quit_item])?;

            let new_item = MenuItem::with_id(handle, "new", i18n::mt(lang, "new"), true, Some("CmdOrCtrl+N"))?;
            let new_window_item = MenuItem::with_id(handle, "new_window", i18n::mt(lang, "new_window"), true, Some("CmdOrCtrl+Shift+N"))?;
            let open_item = MenuItem::with_id(handle, "open", i18n::mt(lang, "open"), true, Some("CmdOrCtrl+O"))?;
            let recent_label = i18n::mt(lang, "open_recent");
            let recent_submenu = Submenu::with_id(handle, "recent_submenu", recent_label, true)?;
            let save_item = MenuItem::with_id(handle, "save", i18n::mt(lang, "save"), true, Some("CmdOrCtrl+S"))?;
            let save_as_item = MenuItem::with_id(handle, "save_as", i18n::mt(lang, "save_as"), true, Some("CmdOrCtrl+Shift+S"))?;
            let sep = PredefinedMenuItem::separator(handle)?;
            let sep_before_print = PredefinedMenuItem::separator(handle)?;
            let page_setup_item = MenuItem::with_id(handle, "page_setup", i18n::mt(lang, "page_setup"), true, Some("CmdOrCtrl+Shift+P"))?;
            let print_item = MenuItem::with_id(handle, "print", i18n::mt(lang, "print"), true, Some("CmdOrCtrl+P"))?;
            let sep_close = PredefinedMenuItem::separator(handle)?;
            let close_item = MenuItem::with_id(handle, "close_window", i18n::mt(lang, "close"), true, Some("CmdOrCtrl+W"))?;

            let file_label = i18n::mt(lang, "file");
            let file_menu = Submenu::with_items(handle, file_label, true, &[
                &new_item,
                &new_window_item,
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

            let undo       = MenuItem::with_id(handle, "edit_undo",       i18n::mt(lang, "undo"), true, Some("CmdOrCtrl+Z"))?;
            let redo       = MenuItem::with_id(handle, "edit_redo",       i18n::mt(lang, "redo"), true, Some("CmdOrCtrl+Shift+Z"))?;
            let sep2 = PredefinedMenuItem::separator(handle)?;
            let cut        = PredefinedMenuItem::cut(handle, Some(i18n::mt(lang, "cut")))?;
            let copy       = PredefinedMenuItem::copy(handle, Some(i18n::mt(lang, "copy")))?;
            let paste      = PredefinedMenuItem::paste(handle, Some(i18n::mt(lang, "paste")))?;
            let sep3 = PredefinedMenuItem::separator(handle)?;
            let select_all = PredefinedMenuItem::select_all(handle, Some(i18n::mt(lang, "select_all")))?;
            let find_item = MenuItem::with_id(handle, "edit_find", i18n::mt(lang, "find"), true, Some("CmdOrCtrl+F"))?;
            let find_next_item = MenuItem::with_id(handle, "edit_find_next", i18n::mt(lang, "find_next"), true, Some("CmdOrCtrl+G"))?;
            let replace_item = MenuItem::with_id(handle, "edit_replace", i18n::mt(lang, "replace"), true, Some("CmdOrCtrl+Alt+F"))?;
            let goto_item = MenuItem::with_id(handle, "edit_goto", i18n::mt(lang, "goto"), true, Some("CmdOrCtrl+L"))?;
            let sep4 = PredefinedMenuItem::separator(handle)?;
            let datetime_item = MenuItem::with_id(handle, "edit_datetime", i18n::mt(lang, "datetime"), true, Some("CmdOrCtrl+Shift+T"))?;

            let edit_label = i18n::mt(lang, "edit");
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

            let status_item = CheckMenuItem::with_id(handle, "status_bar", i18n::mt(lang, "status_bar"), true, saved_status, None::<&str>)?;
            {
                let sms = app.state::<StatusMenuState>();
                sms.flag.store(saved_status, Ordering::Relaxed);
                *sms.item.lock().unwrap() = Some(status_item.clone());
            }

            let view_label = i18n::mt(lang, "view");
            let view_menu = Submenu::with_items(handle, view_label, true, &[
                &status_item,
            ])?;

            let wrap_label = i18n::mt(lang, "word_wrap");
            let wrap_item = CheckMenuItem::with_id(handle, "word_wrap", wrap_label, true, saved_wrap, None::<&str>)?;
            let wi = wrap_item.clone();
            let wrap_state = std::sync::Arc::new(AtomicBool::new(saved_wrap));
            let wrap_state_menu = wrap_state.clone();



            let font_panel_item = MenuItem::with_id(handle, "font_panel", i18n::mt(lang, "font"), true, Some("cmd+t"))?;
            let format_label = i18n::mt(lang, "format");
            let format_menu = Submenu::with_items(handle, format_label, true, &[&wrap_item, &font_panel_item])?;

            let help_label = i18n::mt(lang, "help");
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
                                    // Save: go through the dirty windows one by one
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
                                _ => { /* Cancel: nothing */ }
                            }
                        });
                    }
                    "new" => { new_window(app); }
                    "new_window" => { new_standalone_window(app); }
                    "open" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = open_file_new_window(app).await;
                        });
                    }
                    "save" => { emit_to_focused(app, "menu-save", ()); }
                    "save_as"    => { emit_to_focused(app, "menu-save-as", ()); }
                    "page_setup" => { emit_to_focused(app, "menu-page-setup", ()); }
                    "print"      => { emit_to_focused(app, "menu-print", ()); }
                    "close_window" => { emit_to_focused(app, "menu-close-window", ()); }
                    "font_panel" => { emit_to_focused(app, "menu-font-panel", ()); }
                    "settings" => { let _ = open_settings_window(&app); }
                    "edit_undo"       => { emit_to_focused(app, "menu-edit", "undo"); }
                    "edit_redo"       => { emit_to_focused(app, "menu-edit", "redo"); }

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
                                            add_recent(app, &path); // move to the top of the list
                                        }
                                        Err(_) => {}
                                    }
                                } else {
                                    // file was deleted: remove it from the list and rebuild the menu
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
                    "word_wrap" => {
                        let new_state = !wrap_state_menu.load(Ordering::Relaxed);
                        wrap_state_menu.store(new_state, Ordering::Relaxed);
                        let _ = wi.set_checked(new_state);
                        emit_to_focused(app, "menu-word-wrap", new_state);
                    }
                    "status_bar" => {
                        let sms = app.state::<StatusMenuState>();
                        let new_state = !sms.flag.load(Ordering::Relaxed);
                        sms.flag.store(new_state, Ordering::Relaxed);
                        if let Some(item) = sms.item.lock().unwrap().as_ref() {
                            let _ = item.set_checked(new_state);
                        }
                        emit_to_focused(app, "menu-status-bar", new_state);
                    }
                    _ => {}
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "font-panel" || window.label() == "settings" {
                    return;
                }
                let label = window.label().to_string();
                let confirmed = window.state::<ConfirmedWindowsState>()
                    .0.lock().unwrap().contains(&label);
                if confirmed { return; }
                api.prevent_close();
                let _ = window.emit_to(&label, "close-requested", ());
            }
            if let tauri::WindowEvent::Destroyed = event {
                if window.label() == "font-panel" || window.label() == "settings" {
                    return;
                }
                window.state::<UntitledState>()
                    .0.lock().unwrap().remove(window.label());
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_lang,
            open_file,
            open_file_new_window,
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
            close_settings,
            take_pending_file,
            get_word_wrap,
            set_word_wrap,
            get_use_tabs,
            set_use_tabs,
            get_status_bar,
            set_status_bar,
            get_show_counter,
            set_show_counter,
            get_auto_name,
            set_auto_name,
            set_dirty,
            set_tab_title,
            set_represented_file,
            claim_untitled_number,
            release_untitled_number,
            confirm_close,
            confirm_close_dirty,
            print_document,
            page_setup,
            show_font_panel,
            confirm_quit_window,
            cancel_quit,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| match event {
            tauri::RunEvent::Opened { urls } => handle_opened_urls(app_handle, urls),
            tauri::RunEvent::Ready => {
                // Let the first window appear before the alert.
                let handle = app_handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    let h = handle.clone();
                    let _ = handle.run_on_main_thread(move || offer_default_for_txt(&h));
                });
            }
            _ => {}
        });
}
