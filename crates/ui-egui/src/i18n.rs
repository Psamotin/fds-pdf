//! Desktop presentation resources. Stable keys are independent of command ids and English copy.
//! Locale is scoped to each app call/frame (and restored on return), never process-global.
//! Document contents, paths, serialized values, engine ids and automation protocol are unchanged.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::LazyLock;

pub const PRODUCT_NAME: &str = "ФДС ПДФ";
pub const PRODUCT_VERSION: &str = "0.2.0-fds.1-rc1";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Ru,
    En,
    Ja,
}

#[doc(hidden)]
pub struct Resource {
    pub formatted: bool,
    pub key: &'static str,
    pub source: &'static str,
    pub en: &'static str,
    pub ru: &'static str,
}

// Catalogue labels are resolved repeatedly while painting large panels. Build
// the immutable source index once rather than scan the whole resource table per row.
static SOURCES: LazyLock<HashMap<&'static str, &'static Resource>> = LazyLock::new(|| {
    let mut index = HashMap::with_capacity(crate::resources::RESOURCES.len());
    for resource in crate::resources::RESOURCES {
        index.entry(resource.source).or_insert(resource);
    }
    index
});

thread_local! {
    static LANGUAGE: Cell<Language> = const { Cell::new(Language::Ru) };
}

pub fn current() -> Language {
    LANGUAGE.with(Cell::get)
}

/// Restores the previous locale even when a guarded operation unwinds.
pub struct LocaleScope(Language);
impl Drop for LocaleScope {
    fn drop(&mut self) {
        LANGUAGE.with(|locale| locale.set(self.0));
    }
}
pub fn scope(language: Language) -> LocaleScope {
    LocaleScope(LANGUAGE.with(|locale| locale.replace(language)))
}

/// Carry the current app's locale into a background job without sharing mutable global state.
pub fn in_locale<F: FnOnce() -> T, T>(work: F) -> impl FnOnce() -> T {
    let language = current();
    move || {
        let _locale = scope(language);
        work()
    }
}

/// Resolve a stable key in the scoped locale. Unknown keys are visible, never silently empty.
pub fn text(key: &str) -> &str {
    let Ok(index) = crate::resources::RESOURCES.binary_search_by_key(&key, |resource| resource.key) else { return key };
    let resource = &crate::resources::RESOURCES[index];
    match current() {
        Language::Ru if !resource.ru.is_empty() => resource.ru,
        Language::Ja => Language::Ja.tr(resource.en),
        _ => resource.en,
    }
}

/// English count suffix. Russian messages use invariant labels followed by counts.
pub fn plural_suffix(count: usize) -> &'static str {
    if current() == Language::Ru || count == 1 { "" } else { "s" }
}

impl Language {
    pub const ALL: [Self; 3] = [Self::Ru, Self::En, Self::Ja];
    pub fn name(self) -> &'static str {
        match self {
            Self::Ru => "Русский",
            Self::En => "English",
            Self::Ja => "日本語",
        }
    }
    pub fn parse(code: &str) -> Option<Self> {
        match code {
            "ru" => Some(Self::Ru),
            "en" => Some(Self::En),
            "ja" => Some(Self::Ja),
            _ => None,
        }
    }

    /// Adapter for upstream catalogue/command labels at the UI boundary only.
    /// Prefer `text(key)` for desktop-owned copy. Never pass document contents here.
    pub fn tr(self, source: &str) -> &str {
        if self == Self::Ja
            && let Some((_, translated)) = JAPANESE.iter().find(|(english, _)| *english == source)
        {
            return translated;
        }
        if let Some(resource) = SOURCES.get(source) {
            return if self == Self::Ru && !resource.ru.is_empty() { resource.ru } else { resource.en };
        }
        source
    }

    pub fn command_label(self, source: &str) -> String {
        for prefix in ["Undo ", "Redo "] {
            if let Some(action) = source.strip_prefix(prefix) {
                return format!("{} {}", self.tr(prefix.trim()), self.history_action_label(action));
            }
        }
        self.history_action_label(source)
    }

    fn history_action_label(self, source: &str) -> String {
        if self == Self::Ru {
            // These prefixes are engine history labels. Field names and filenames
            // after them are user data and must remain byte-for-byte unchanged.
            for prefix in ["Fill in ", "Set the image of ", "Edit script of ", "Insert pages from ", "Import ", "Save as ", "Change "] {
                if let Some(value) = source.strip_prefix(prefix) {
                    return format!("{} {value}", self.tr(prefix.trim()));
                }
            }
            if let Some(status) = source.strip_prefix("Set status ") {
                return format!("{} {}", self.tr("Set status"), self.tr(status));
            }
            if let Some(noun) = source.strip_prefix("Add ") {
                let mut title = noun.to_owned();
                if let Some(first) = title.get_mut(..1) {
                    first.make_ascii_uppercase();
                }
                let translated = self.tr(&title);
                if translated != title {
                    return format!("{} {translated}", self.tr("Add"));
                }
            }
        }
        self.tr(source).to_owned()
    }
}

/// Hide upstream marketing from menus/palette while keeping the engine registry compatible.
pub fn is_marketing_command(id: &str) -> bool {
    matches!(id, "help.discord" | "help.website" | "help.app_page" | "help.github")
}

const JAPANESE: &[(&str, &str)] = &[
    ("Menu", "メニュー"),
    ("File", "ファイル"),
    ("Edit", "編集"),
    ("Pages", "ページ"),
    ("View", "表示"),
    ("Help", "ヘルプ"),
    ("Preferences", "環境設定"),
    ("Preferences…", "環境設定…"),
    ("Interface language", "表示言語"),
    ("Identity", "個人情報"),
    ("Name on new comments", "新しい注釈の作成者名"),
    ("Open…", "開く…"),
    ("New blank PDF", "空白の PDF を作成"),
    ("Create PDF from file…", "ファイルから PDF を作成…"),
    ("Create PDF from images…", "画像から PDF を作成…"),
    ("Create PDF from clipboard", "クリップボードから PDF を作成"),
    ("Combine files…", "ファイルを結合…"),
    ("Save", "保存"),
    ("Save as…", "別名で保存…"),
    ("Close file", "ファイルを閉じる"),
    ("Close all", "すべて閉じる"),
    ("Revert", "保存済みの状態に戻す"),
    ("Print…", "印刷…"),
    ("Document properties…", "文書のプロパティ…"),
    ("Undo", "取り消し"),
    ("Redo", "やり直し"),
    ("Find…", "検索…"),
    ("Advanced search…", "高度な検索…"),
    ("Copy pages", "ページをコピー"),
    ("Cut pages", "ページを切り取り"),
    ("Paste pages", "ページを貼り付け"),
    ("Fit visible", "表示範囲に合わせる"),
    ("Marquee zoom", "範囲指定ズーム"),
    ("Take a snapshot", "スナップショットを作成"),
    ("Full screen mode", "全画面表示"),
    ("Read mode", "閲覧モード"),
    ("Switch light / dark theme", "明るい／暗いテーマを切り替え"),
    ("Comments panel", "コメントパネル"),
    ("Form fields panel", "フォームフィールドパネル"),
    ("Clear form", "フォームをクリア"),
    ("Find tools and commands…", "ツールとコマンドを検索…"),
    ("Zoom", "ズーム"),
    ("Actual size", "実際のサイズ"),
    ("Zoom to page level", "ページ全体を表示"),
    ("Fit to width", "幅に合わせる"),
    ("Display theme", "表示テーマ"),
    ("Side panels", "サイドパネル"),
    ("Enable Acrobat JavaScript", "Acrobat JavaScript を有効にする"),
    ("OK", "OK"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resources_have_unique_stable_keys_and_russian_copy() {
        let mut previous = "";
        for resource in crate::resources::RESOURCES {
            assert!(resource.key > previous, "duplicate or unsorted key: {}", resource.key);
            assert!(!resource.en.is_empty() && !resource.ru.is_empty(), "{}", resource.key);
            previous = resource.key;
        }
        let _locale = scope(Language::Ru);
        assert_eq!(text("ui.open"), "Открыть");
        assert_eq!(text("missing.key"), "missing.key");
        assert_eq!(Language::Ru.tr("Scan & OCR"), "Распознать текст");
        assert_eq!(Language::Ru.tr("file.open"), "file.open");
    }

    #[test]
    fn scoped_locales_restore_and_preserve_document_data() {
        let _russian = scope(Language::Ru);
        {
            let _english = scope(Language::En);
            assert_eq!(text("ui.open"), "Open");
        }
        assert_eq!(text("ui.open"), "Открыть");
        assert_eq!(Language::Ru.tr("日本語の文書.pdf"), "日本語の文書.pdf");
        assert_eq!(Language::Ja.tr("File"), "ファイル");
        assert_eq!(Language::parse("xx"), None);
        assert_eq!(Language::Ru.command_label("Undo Fill in Open"), "Отменить Заполнить поле Open");
        assert_eq!(Language::Ru.command_label("Insert pages from Save.pdf"), "Вставить страницы из Save.pdf");
        assert_eq!(Language::Ru.command_label("Add highlight"), "Добавить Выделение");
        assert_eq!(Language::En.command_label("Undo Fill in Open"), "Undo Fill in Open");
    }

    #[test]
    fn background_jobs_keep_the_callers_locale() {
        let job = {
            let _english = scope(Language::En);
            in_locale(|| text("ui.open").to_owned())
        };
        assert_eq!(std::thread::spawn(job).join().unwrap(), "Open");
        assert_eq!(current(), Language::Ru);
    }

    #[test]
    fn upstream_tool_groups_have_russian_presentation_labels() {
        for group in printcraft_engine::catalog::TOOL_GROUPS {
            assert_ne!(Language::Ru.tr(group.label), group.label, "{}", group.id);
        }
    }

    #[test]
    fn language_persists_and_invalid_input_keeps_current_language() {
        let mut app = crate::PrintCraftApp::default();
        assert_eq!(app.language, Language::Ru);
        app.set_option("language", "en").unwrap();
        let mut restored = crate::PrintCraftApp::default();
        restored.restore(&app.persist());
        assert_eq!(restored.language, Language::En);
        assert!(restored.set_option("language", "xx").is_err());
        assert_eq!(restored.language, Language::En);
        let mut legacy = crate::PrintCraftApp::default();
        legacy.restore("{}");
        assert_eq!(legacy.language, Language::Ru);
    }
}
