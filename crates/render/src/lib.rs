use ftml::layout::Layout;
use ftml::settings::{WikitextMode, WikitextSettings};

/// Версия FTML, проверенная ограниченным прототипом M0-Compat.
pub const EXPECTED_FTML_VERSION: &str = "1.41.0";

/// Возвращает профиль парсинга wiki-страницы, проверенный M0-Compat.
///
/// Это только настройки FTML. Функция не выполняет include, module dispatch
/// или обязательную финальную очистку HTML.
#[must_use]
pub fn m0_compatibility_settings() -> WikitextSettings {
    WikitextSettings::from_mode(WikitextMode::Page, Layout::Wikidot)
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use ftml::data::{PageInfo, ScoreValue};
    use ftml::layout::Layout;
    use ftml::parsing::ParseErrorKind;
    use ftml::render::Render;
    use ftml::render::html::HtmlRender;
    use ftml::render::text::TextRender;
    use ftml::tree::SyntaxTree;

    use super::*;

    fn page_info() -> PageInfo<'static> {
        PageInfo {
            page: Cow::Borrowed("m0-compat"),
            category: Some(Cow::Borrowed("test")),
            site: Cow::Borrowed("wikinext"),
            title: Cow::Borrowed("M0 compatibility probe"),
            alt_title: None,
            score: ScoreValue::Integer(0),
            tags: vec![Cow::Borrowed("compat")],
            language: Cow::Borrowed("ru"),
        }
    }

    #[test]
    fn pins_ftml_version_and_legacy_layout() {
        assert_eq!(ftml::info::PKG_VERSION, EXPECTED_FTML_VERSION);

        let settings = m0_compatibility_settings();
        assert_eq!(settings.layout, Layout::Wikidot);
        assert!(settings.layout.legacy());
    }

    #[test]
    fn parses_and_renders_simple_wikidot_markup() {
        let mut source = "++ Заголовок\n\nПривет, **WikiNext**.".to_owned();
        ftml::preprocess(&mut source);
        let tokens = ftml::tokenize(&source);
        let settings = m0_compatibility_settings();
        let page_info = page_info();
        let outcome = ftml::parse(&tokens, &page_info, &settings);

        assert!(
            outcome.errors().is_empty(),
            "unexpected parse errors: {:?}",
            outcome.errors()
        );

        let html = HtmlRender.render(outcome.value(), &page_info, &settings);
        let text = TextRender.render(outcome.value(), &page_info, &settings);

        assert!(html.body.contains("<h2"));
        assert!(html.body.contains("Заголовок"));
        assert!(html.body.contains("<strong>WikiNext</strong>"));
        assert!(text.contains("Заголовок"));
        assert!(text.contains("Привет, WikiNext."));
    }

    #[test]
    fn unknown_module_is_a_non_fatal_parse_error() {
        let mut source = "[[module ListPages]]".to_owned();
        ftml::preprocess(&mut source);
        let tokens = ftml::tokenize(&source);
        let settings = m0_compatibility_settings();
        let page_info = page_info();
        let outcome = ftml::parse(&tokens, &page_info, &settings);

        assert!(outcome.errors().iter().any(|error| {
            error.kind() == ParseErrorKind::NoSuchModule && error.rule() == "block-module"
        }));

        let rendered = TextRender.render(outcome.value(), &page_info, &settings);
        assert!(
            rendered.contains("[[module ListPages]]"),
            "unknown module fallback unexpectedly disappeared: {rendered:?}"
        );
    }

    #[test]
    fn syntax_tree_survives_serde_round_trip() {
        let mut source = "+ Заголовок\n\nТекст со **строгим** начертанием и [# якорем].".to_owned();
        ftml::preprocess(&mut source);
        let tokens = ftml::tokenize(&source);
        let settings = m0_compatibility_settings();
        let page_info = page_info();
        let outcome = ftml::parse(&tokens, &page_info, &settings);
        assert!(outcome.errors().is_empty());

        let encoded = serde_json::to_string(outcome.value()).expect("serialize SyntaxTree");
        let mut decoded: SyntaxTree<'_> =
            serde_json::from_str(&encoded).expect("deserialize SyntaxTree");

        // FTML помечает wikitext_len как serde(skip): это оптимизационная
        // подсказка, которую будущий кэш должен восстановить отдельно.
        assert_eq!(decoded.wikitext_len, 0);
        decoded.wikitext_len = outcome.value().wikitext_len;
        assert_eq!(decoded, *outcome.value());
    }
}
