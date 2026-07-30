# FTML 1.41.0: ограниченный M0-Compat probe

Статус: проверенный исполняемыми unit-тестами прототип. Это не production
renderer и не утверждение о полной совместимости с RuFoundation.

## Зафиксированный контракт

- зависимость workspace закреплена точно на `ftml = 1.41.0`;
- режим страницы создаётся через `WikitextMode::Page`;
- DOM-профиль по умолчанию — legacy `Layout::Wikidot`;
- последовательность `preprocess → tokenize → parse` успешно обрабатывает
  простую Wikidot-разметку;
- полученный AST рендерится через `HtmlRender` и `TextRender`;
- неизвестный `[[module ListPages]]` не вызывает panic: FTML возвращает
  `ParseErrorKind::NoSuchModule` и сохраняет исходную конструкцию через
  текстовый fallback;
- `SyntaxTree` проходит JSON serde round-trip по структурным полям.

`SyntaxTree::wikitext_len` помечен в FTML как `serde(skip)`. После
десериализации он равен нулю, поэтому будущий AST-кэш должен хранить и
восстанавливать длину исходного wikitext отдельно. Это поле является
оптимизационной подсказкой, а не частью структуры документа.

## Архитектурное решение для неизвестных модулей

FTML 1.41.0 не сохраняет неизвестный модуль как typed AST-узел. Чтобы выполнить
контракт WikiNext с диагностическим placeholder и structured warning, custom
modules следует извлекать до передачи текста в FTML и заменять внутренними
безопасными placeholders. Реализация registry и placeholder pipeline остаётся
за M3; текущий тест только закрепляет наблюдаемое поведение зависимости.

## Границы результата

Probe не проверяет:

- RuFoundation FTML fork delta и реальные golden pages;
- include resolver, лимиты глубины, циклы и ACL включаемых страниц;
- custom module registry;
- sanitizer, DOM hooks и безопасность итогового HTML;
- CSS/DOM-совместимость тем;
- deadline, bounded blocking pool и кэши.

HTML из `HtmlRender` нельзя отдавать пользователю напрямую. Production pipeline
должен завершаться отдельным allowlist sanitizer, как требует основной
архитектурный документ.
