// i18n.rs — localization of the NATIVE side (NSMenu menus and NSAlert alerts).
// Mirrors src/i18n.js for Rust: the same 8 languages and fallback rules.
//
// Fallback chain (three steps, as in JS):
//   1. unknown system locale              -> "en"
//   2. language known, key missing in it  -> the English value
//   3. key missing in English too         -> the key itself (never empty)
//
// Adding a language = a new table function + a line in the match inside mt().

/// Detects the system language. Returns one of the 8 codes, otherwise "en".
/// Uses the same prefixes as src/i18n.js; the two must stay in sync.
pub fn system_lang() -> &'static str {
    let locale = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    if locale.starts_with("ru") {
        "ru"
    } else if locale.starts_with("es") {
        "es"
    } else if locale.starts_with("de") {
        "de"
    } else if locale.starts_with("fr") {
        "fr"
    } else if locale.starts_with("zh") {
        "zh"
    } else if locale.starts_with("ja") {
        "ja"
    } else if locale.starts_with("pt") {
        "pt"
    } else {
        "en"
    }
}

/// Menu Translate: the menu string for a key in the given language.
pub fn mt(lang: &str, key: &'static str) -> &'static str {
    match lang {
        "ru" => ru(key),
        "es" => es(key),
        "de" => de(key),
        "fr" => fr(key),
        "zh" => zh(key),
        "ja" => ja(key),
        "pt" => pt(key),
        _ => en(key),
    }
}

// ─────────────────────────── English (base) ───────────────────────────
fn en(key: &'static str) -> &'static str {
    match key {
        // App menu
        "about" => "About LikeNotepad.exe",
        "about_comments" => "The same Notepad, but without Windows. No clouds, no subscriptions, no AI. Just text",
        "settings" => "Settings\u{2026}",
        "quit" => "Quit LikeNotepad.exe",
        // File
        "file" => "File",
        "new" => "New",
        "new_window" => "New Window",
        "open" => "Open...",
        "open_recent" => "Open Recent",
        "no_recent" => "No Recent Files",
        "clear_recent" => "Clear Menu",
        "save" => "Save",
        "save_as" => "Save As...",
        "page_setup" => "Page Setup...",
        "print" => "Print...",
        "close" => "Close",
        // Edit
        "edit" => "Edit",
        "undo" => "Undo",
        "redo" => "Redo",
        "cut" => "Cut",
        "copy" => "Copy",
        "paste" => "Paste",
        "select_all" => "Select All",
        "find" => "Find...",
        "find_next" => "Find Next",
        "replace" => "Replace...",
        "goto" => "Go to...",
        "datetime" => "Time/Date",
        // View
        "view" => "View",
        "status_bar" => "Status Bar",
        // Format
        "format" => "Format",
        "word_wrap" => "Word Wrap",
        "font" => "Font\u{2026}",
        "font_title" => "Font",
        "settings_title" => "Settings",
        // Help
        "help" => "Help",
        // Alert buttons
        "alert_save" => "Save\u{2026}",
        "alert_dont_save" => "Don't Save",
        "alert_cancel" => "Cancel",
        // Alert informative text
        "quit_info" => "Do you want to save your changes before quitting? Your changes will be lost if you don't save.",
        "close_info" => "Your changes will be lost if you don't save them.",
        _ => key,
    }
}

// ─────────────────────────── Russian ───────────────────────────
fn ru(key: &'static str) -> &'static str {
    match key {
        "about" => "О программе LikeNotepad.exe",
        "about_comments" => "Тот самый Блокнот, но без Windows. Никаких облаков, подписок и ИИ. Просто текст",
        "settings" => "Настройки\u{2026}",
        "quit" => "Завершить LikeNotepad.exe",
        "file" => "Файл",
        "new" => "Создать",
        "new_window" => "Новое окно",
        "open" => "Открыть...",
        "open_recent" => "Открыть недавние",
        "no_recent" => "Нет недавних файлов",
        "clear_recent" => "Очистить меню",
        "save" => "Сохранить",
        "save_as" => "Сохранить как...",
        "page_setup" => "Параметры страницы...",
        "print" => "Печать...",
        "close" => "Закрыть",
        "edit" => "Правка",
        "undo" => "Отменить",
        "redo" => "Повторить",
        "cut" => "Вырезать",
        "copy" => "Копировать",
        "paste" => "Вставить",
        "select_all" => "Выделить всё",
        "find" => "Найти...",
        "find_next" => "Найти далее",
        "replace" => "Заменить...",
        "goto" => "Перейти...",
        "datetime" => "Время/Дата",
        "view" => "Вид",
        "status_bar" => "Строка состояния",
        "format" => "Формат",
        "word_wrap" => "Перенос по словам",
        "font" => "Шрифт\u{2026}",
        "font_title" => "Шрифт",
        "settings_title" => "Настройки",
        "help" => "Справка",
        "alert_save" => "Сохранить\u{2026}",
        "alert_dont_save" => "Не сохранять",
        "alert_cancel" => "Отмена",
        "quit_info" => "Хотите сохранить изменения перед завершением? Несохранённые изменения будут потеряны.",
        "close_info" => "Если не сохранить, изменения будут потеряны.",
        _ => en(key),
    }
}

// ─────────────────────────── Español ───────────────────────────
fn es(key: &'static str) -> &'static str {
    match key {
        "about" => "Acerca de LikeNotepad.exe",
        "about_comments" => "El mismo Bloc de notas, pero sin Windows. Sin nubes, sin suscripciones, sin IA. Solo texto",
        "settings" => "Ajustes\u{2026}",
        "quit" => "Salir de LikeNotepad.exe",
        "file" => "Archivo",
        "new" => "Nuevo",
        "new_window" => "Nueva ventana",
        "open" => "Abrir...",
        "open_recent" => "Abrir recientes",
        "no_recent" => "No hay documentos recientes",
        "clear_recent" => "Borrar menú",
        "save" => "Guardar",
        "save_as" => "Guardar como...",
        "page_setup" => "Ajustar página...",
        "print" => "Imprimir...",
        "close" => "Cerrar",
        "edit" => "Edición",
        "undo" => "Deshacer",
        "redo" => "Rehacer",
        "cut" => "Cortar",
        "copy" => "Copiar",
        "paste" => "Pegar",
        "select_all" => "Seleccionar todo",
        "find" => "Buscar...",
        "find_next" => "Buscar siguiente",
        "replace" => "Reemplazar...",
        "goto" => "Ir a...",
        "datetime" => "Hora/Fecha",
        "view" => "Visualización",
        "status_bar" => "Barra de estado",
        "format" => "Formato",
        "word_wrap" => "Ajuste de línea",
        "font" => "Tipo de letra\u{2026}",
        "font_title" => "Tipo de letra",
        "settings_title" => "Ajustes",
        "help" => "Ayuda",
        "alert_save" => "Guardar\u{2026}",
        "alert_dont_save" => "No guardar",
        "alert_cancel" => "Cancelar",
        "quit_info" => "¿Quieres guardar los cambios antes de salir? Si no los guardas, se perderán.",
        "close_info" => "Si no guardas los cambios, se perderán.",
        _ => en(key),
    }
}

// ─────────────────────────── Deutsch ───────────────────────────
fn de(key: &'static str) -> &'static str {
    match key {
        "about" => "Über LikeNotepad.exe",
        "about_comments" => "Der gleiche Notepad, aber ohne Windows. Keine Cloud, keine Abos, keine KI. Nur Text",
        "settings" => "Einstellungen\u{2026}",
        "quit" => "LikeNotepad.exe beenden",
        "file" => "Datei",
        "new" => "Neu",
        "new_window" => "Neues Fenster",
        "open" => "Öffnen...",
        "open_recent" => "Benutzte Dokumente",
        "no_recent" => "Keine benutzten Dokumente",
        "clear_recent" => "Menü löschen",
        "save" => "Sichern",
        "save_as" => "Sichern unter...",
        "page_setup" => "Papierformat...",
        "print" => "Drucken...",
        "close" => "Schließen",
        "edit" => "Bearbeiten",
        "undo" => "Widerrufen",
        "redo" => "Wiederholen",
        "cut" => "Ausschneiden",
        "copy" => "Kopieren",
        "paste" => "Einsetzen",
        "select_all" => "Alles auswählen",
        "find" => "Suchen...",
        "find_next" => "Weitersuchen",
        "replace" => "Ersetzen...",
        "goto" => "Gehe zu...",
        "datetime" => "Uhrzeit/Datum",
        "view" => "Darstellung",
        "status_bar" => "Statusleiste",
        "format" => "Format",
        "word_wrap" => "Zeilenumbruch",
        "font" => "Schrift\u{2026}",
        "font_title" => "Schrift",
        "settings_title" => "Einstellungen",
        "help" => "Hilfe",
        "alert_save" => "Sichern\u{2026}",
        "alert_dont_save" => "Nicht sichern",
        "alert_cancel" => "Abbrechen",
        "quit_info" => "Möchtest du die Änderungen vor dem Beenden sichern? Andernfalls gehen sie verloren.",
        "close_info" => "Wenn du nicht sicherst, gehen die Änderungen verloren.",
        _ => en(key),
    }
}

// ─────────────────────────── Français ───────────────────────────
fn fr(key: &'static str) -> &'static str {
    match key {
        "about" => "À propos de LikeNotepad.exe",
        "about_comments" => "Le même Bloc-notes, mais sans Windows. Pas de cloud, pas d'abonnement, pas d'IA. Juste du texte",
        "settings" => "Réglages\u{2026}",
        "quit" => "Quitter LikeNotepad.exe",
        "file" => "Fichier",
        "new" => "Nouveau",
        "new_window" => "Nouvelle fenêtre",
        "open" => "Ouvrir...",
        "open_recent" => "Ouvrir l'élément récent",
        "no_recent" => "Aucun document récent",
        "clear_recent" => "Effacer le menu",
        "save" => "Enregistrer",
        "save_as" => "Enregistrer sous...",
        "page_setup" => "Format d'impression...",
        "print" => "Imprimer...",
        "close" => "Fermer",
        "edit" => "Édition",
        "undo" => "Annuler",
        "redo" => "Rétablir",
        "cut" => "Couper",
        "copy" => "Copier",
        "paste" => "Coller",
        "select_all" => "Tout sélectionner",
        "find" => "Rechercher...",
        "find_next" => "Rechercher le suivant",
        "replace" => "Remplacer...",
        "goto" => "Aller à...",
        "datetime" => "Heure/Date",
        "view" => "Présentation",
        "status_bar" => "Barre d'état",
        "format" => "Format",
        "word_wrap" => "Retour à la ligne automatique",
        "font" => "Police\u{2026}",
        "font_title" => "Police",
        "settings_title" => "Réglages",
        "help" => "Aide",
        "alert_save" => "Enregistrer\u{2026}",
        "alert_dont_save" => "Ne pas enregistrer",
        "alert_cancel" => "Annuler",
        "quit_info" => "Voulez-vous enregistrer les modifications avant de quitter ? Sinon, elles seront perdues.",
        "close_info" => "Vos modifications seront perdues si vous ne les enregistrez pas.",
        _ => en(key),
    }
}

// ─────────────────────────── 中文（简体）───────────────────────────
fn zh(key: &'static str) -> &'static str {
    match key {
        "about" => "关于 LikeNotepad.exe",
        "about_comments" => "还是那个记事本，但没有 Windows。没有云端，没有订阅，没有 AI。只有文本",
        "settings" => "设置\u{2026}",
        "quit" => "退出 LikeNotepad.exe",
        "file" => "文件",
        "new" => "新建",
        "new_window" => "新建窗口",
        "open" => "打开...",
        "open_recent" => "打开最近使用",
        "no_recent" => "没有最近使用的文稿",
        "clear_recent" => "清除菜单",
        "save" => "存储",
        "save_as" => "存储为...",
        "page_setup" => "页面设置...",
        "print" => "打印...",
        "close" => "关闭",
        "edit" => "编辑",
        "undo" => "撤销",
        "redo" => "重做",
        "cut" => "剪切",
        "copy" => "拷贝",
        "paste" => "粘贴",
        "select_all" => "全选",
        "find" => "查找...",
        "find_next" => "查找下一个",
        "replace" => "替换...",
        "goto" => "前往...",
        "datetime" => "时间/日期",
        "view" => "显示",
        "status_bar" => "状态栏",
        "format" => "格式",
        "word_wrap" => "自动换行",
        "font" => "字体\u{2026}",
        "font_title" => "字体",
        "settings_title" => "设置",
        "help" => "帮助",
        "alert_save" => "存储\u{2026}",
        "alert_dont_save" => "不存储",
        "alert_cancel" => "取消",
        "quit_info" => "退出前要存储更改吗？如果不存储，更改将会丢失。",
        "close_info" => "如果不存储，更改将会丢失。",
        _ => en(key),
    }
}

// ─────────────────────────── 日本語 ───────────────────────────
fn ja(key: &'static str) -> &'static str {
    match key {
        "about" => "LikeNotepad.exe について",
        "about_comments" => "あのメモ帳、ただし Windows なし。クラウドなし、サブスクなし、AI なし。ただのテキスト",
        "settings" => "設定\u{2026}",
        "quit" => "LikeNotepad.exe を終了",
        "file" => "ファイル",
        "new" => "新規",
        "new_window" => "新規ウインドウ",
        "open" => "開く...",
        "open_recent" => "最近使った項目を開く",
        "no_recent" => "最近使った書類なし",
        "clear_recent" => "メニューを消去",
        "save" => "保存",
        "save_as" => "別名で保存...",
        "page_setup" => "ページ設定...",
        "print" => "プリント...",
        "close" => "閉じる",
        "edit" => "編集",
        "undo" => "取り消す",
        "redo" => "やり直す",
        "cut" => "カット",
        "copy" => "コピー",
        "paste" => "ペースト",
        "select_all" => "すべてを選択",
        "find" => "検索...",
        "find_next" => "次を検索",
        "replace" => "置換...",
        "goto" => "移動...",
        "datetime" => "時刻/日付",
        "view" => "表示",
        "status_bar" => "ステータスバー",
        "format" => "フォーマット",
        "word_wrap" => "行を折り返す",
        "font" => "フォント\u{2026}",
        "font_title" => "フォント",
        "settings_title" => "設定",
        "help" => "ヘルプ",
        "alert_save" => "保存\u{2026}",
        "alert_dont_save" => "保存しない",
        "alert_cancel" => "キャンセル",
        "quit_info" => "終了する前に変更を保存しますか？保存しないと変更は失われます。",
        "close_info" => "保存しないと、変更内容は失われます。",
        _ => en(key),
    }
}

// ─────────────────────────── Português (BR) ───────────────────────────
fn pt(key: &'static str) -> &'static str {
    match key {
        "about" => "Sobre o LikeNotepad.exe",
        "about_comments" => "O mesmo Bloco de Notas, mas sem Windows. Sem nuvem, sem assinaturas, sem IA. Apenas texto",
        "settings" => "Ajustes\u{2026}",
        "quit" => "Encerrar LikeNotepad.exe",
        "file" => "Arquivo",
        "new" => "Novo",
        "new_window" => "Nova janela",
        "open" => "Abrir...",
        "open_recent" => "Abrir recentes",
        "no_recent" => "Nenhum documento recente",
        "clear_recent" => "Limpar menu",
        "save" => "Salvar",
        "save_as" => "Salvar como...",
        "page_setup" => "Configurar página...",
        "print" => "Imprimir...",
        "close" => "Fechar",
        "edit" => "Editar",
        "undo" => "Desfazer",
        "redo" => "Refazer",
        "cut" => "Recortar",
        "copy" => "Copiar",
        "paste" => "Colar",
        "select_all" => "Selecionar tudo",
        "find" => "Buscar...",
        "find_next" => "Buscar próxima",
        "replace" => "Substituir...",
        "goto" => "Ir para...",
        "datetime" => "Hora/Data",
        "view" => "Visualizar",
        "status_bar" => "Barra de status",
        "format" => "Formato",
        "word_wrap" => "Quebra automática de linha",
        "font" => "Fonte\u{2026}",
        "font_title" => "Fonte",
        "settings_title" => "Ajustes",
        "help" => "Ajuda",
        "alert_save" => "Salvar\u{2026}",
        "alert_dont_save" => "Não salvar",
        "alert_cancel" => "Cancelar",
        "quit_info" => "Deseja salvar as alterações antes de encerrar? Se não salvar, elas serão perdidas.",
        "close_info" => "Suas alterações serão perdidas se você não salvá-las.",
        _ => en(key),
    }
}

// ─────────── Quit alert title (Cmd+Q), with the document count ───────────
// Pluralization inside: three Russian forms, two for en/es/de/fr/pt,
// none for Chinese and Japanese.
pub fn quit_alert_msg(lang: &str, count: usize) -> String {
    match lang {
        "ru" => {
            if count == 1 {
                "Имеется несохранённый документ.".to_string()
            } else {
                let n100 = count % 100;
                let n10 = count % 10;
                let word = if n100 >= 11 && n100 <= 14 {
                    "несохранённых документов"
                } else if n10 == 1 {
                    "несохранённый документ"
                } else if n10 >= 2 && n10 <= 4 {
                    "несохранённых документа"
                } else {
                    "несохранённых документов"
                };
                format!("Имеется {} {}.", count, word)
            }
        }
        "es" => {
            if count == 1 {
                "Tienes un documento con cambios sin guardar.".to_string()
            } else {
                format!("Tienes {} documentos con cambios sin guardar.", count)
            }
        }
        "de" => {
            if count == 1 {
                "Du hast ein Dokument mit ungesicherten Änderungen.".to_string()
            } else {
                format!("Du hast {} Dokumente mit ungesicherten Änderungen.", count)
            }
        }
        "fr" => {
            if count == 1 {
                "Vous avez un document comportant des modifications non enregistrées.".to_string()
            } else {
                format!(
                    "Vous avez {} documents comportant des modifications non enregistrées.",
                    count
                )
            }
        }
        "zh" => format!("有 {} 个文稿包含未存储的更改。", count),
        "ja" => format!("保存されていない書類が {} 件あります。", count),
        "pt" => {
            if count == 1 {
                "Você tem um documento com alterações não salvas.".to_string()
            } else {
                format!("Você tem {} documentos com alterações não salvas.", count)
            }
        }
        _ => {
            if count == 1 {
                "You have one document with unsaved changes.".to_string()
            } else {
                format!("You have {} documents with unsaved changes.", count)
            }
        }
    }
}

// ─────────── Window close alert title (Cmd+W), with the file name ───────────
pub fn close_alert_msg(lang: &str, name: &str) -> String {
    match lang {
        "ru" => format!("Сохранить изменения в «{}»?", name),
        "es" => format!("¿Quieres guardar los cambios realizados en «{}»?", name),
        "de" => format!("Möchtest du die Änderungen an „{}“ sichern?", name),
        "fr" => format!("Voulez-vous enregistrer les modifications apportées à « {} » ?", name),
        "zh" => format!("要存储对“{}”所做的更改吗？", name),
        "ja" => format!("「{}」への変更を保存しますか？", name),
        "pt" => format!("Deseja salvar as alterações feitas em “{}”?", name),
        _ => format!("Do you want to save the changes you made to \"{}\"?", name),
    }
}
