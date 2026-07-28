# Манифест WikiNext-Engine

**Полное имя:** WikiNext-Engine · **Короткое имя / бренд:** WikiNext
**Статус:** проектирование закрыто, код не начат · **Дата документа:** 2026-07-28
**Репозиторий / workspace:** `wikinext-engine` · **Бинарник сервера:** `wikinext`
**Крейты:** `wikinext-core`, `wikinext-store`, `wikinext-render`, `wikinext-infra`, `wikinext-app`, `wikinext-server`

> Замечание по имени: существует старый неактивный проект WikiNEXT (semantic application wiki, JS, 2012–2014). Ниши разные, юридического конфликта нет; суффикс `-Engine` разводит поисковые запросы. Зафиксировано осознанно.

---

## 1. Цель

Быстрый, безопасный, стабильный wiki-движок уровня WikiJump / WikiDot / RuFoundation, но со своим clean-room ядром: git-подобная история страниц, совместимость с Wikidot-семейством достаточная, чтобы перенести Backrooms Wiki RU целиком — со всей историей ревизий, аккаунтами, правами, темами и компонентами. Мы пишем ядро/движок, а не конкретный сайт. Интерфейс и контент — только русский; инфраструктуры локализации (макросы строк UI, поля перевода, селектор языка) нет намеренно.

## 2. Стек

| Компонент | Версия | Лицензия | Примечание |
|---|---|---|---|
| Rust | **1.97.1** | — | `rust-toolchain.toml` |
| Postgres | **18.4** | — | primary storage |
| ftml | **1.41.0** | AGPL-3.0-or-later | парсер/AST Wikidot-разметки |
| wikidot-normalize | **0.12** | MIT | нормализация имён страниц |
| Axum + Tokio + Tower | актуальные стабильные | — | web-слой |
| SQLx | актуальная стабильная | — | async SQL, compile-time проверка |
| Askama + HTMX | актуальные стабильные | — | Askama только для служебных каркасов |
| Redis | актуальный стабильный | — | L2-кэш + pub/sub invalidation + (позже) streams |
| Meilisearch | актуальный стабильный | — | primary search |

Конкретные номера axum / sqlx / askama / redis-rs / meilisearch-rust в манифест не вносятся намеренно — фиксируются в `Cargo.lock` в M0.

## 3. Лицензия и чужой код

Движок open-source; формальное оформление лицензии — ответственность владельца после релиза. Техническое следствие выбора ftml: ftml под AGPL-3.0-or-later с сетевой оговоркой, поэтому при распространении или запуске как сервис движок обязан быть AGPL-совместимым. Это принято. `wikidot-normalize` под MIT — без ограничений. Код WikiJump / WikiDot / RuFoundation не копируется (качество, дыры, оптимизация); RuFoundation (MIT) используется только как референс поведения и источник данных миграции.

## 4. Принципы

- **Clean-room.** От чужих проектов только ftml и wikidot-normalize как зависимости. Всё остальное — своё.
- **Модульность без перегиба.** 6 крейтов. Файл 150–600 строк, жёсткий максимум 800. Папка 4–12 файлов; >15 — подпапки по поддоменам, <3 — укрупняем. Один файл — одна причина для изменения.
- **Only ru.** Строки интерфейса — русские литералы напрямую, без абстракции локализации.
- **Детерминизм.** Воспроизводимость и простота поддержки выше clever-решений. Стандартная библиотека и зрелые решения по умолчанию.
- **Точность выше связности.** Если факт не подтверждён — помечается как неопределённость, а не подаётся как истина.

## 5. Крейты

Граф зависимостей без циклов:

`wikinext-server → wikinext-app → {wikinext-store, wikinext-render, wikinext-infra} → wikinext-core`

| Крейт | Ответственность | Зависимости |
|---|---|---|
| `wikinext-core` | Домен, ошибки, валидация, policy-резолвер, extension-traits, спецификация site-модулей, обёртка над wikidot-normalize | serde, thiserror, time, uuid, wikidot-normalize |
| `wikinext-store` | Репозитории, транзакции, миграции, append-only audit | wikinext-core, sqlx |
| `wikinext-render` | Адаптер ftml, Includer, module-слой, sanitize, diff, plain-text, post-process hooks | wikinext-core, ftml |
| `wikinext-infra` | Redis, Meilisearch, двухуровневый кэш, JobQueue, BlobStore, feature-флаги | wikinext-core, redis, meilisearch-rust |
| `wikinext-app` | Use-cases, координация, проверка прав, include-резолв, кэш-оркестрация, импорт, джобы | core/store/render/infra |
| `wikinext-server` | HTTP, маршруты, шаблоны, middleware, сессии, CSRF, API | wikinext-app, axum, askama |

## 6. Доменная модель

| Сущность | Суть |
|---|---|
| User | id, login, email, password_hash, `rank: u32`, статус; гость = ранг 0 виртуально |
| Group | именованный набор permission; membership многие-ко-многим |
| ACL-запись | `(scope, principal=group/user, action, allow/deny)`, scope = site / namespace / page |
| Page | id, namespace (=Wikidot category), slug, full_name, page_type, current_revision_id, parent_page_id, is_deleted, is_locked, денормализованные счётчики (rating_score, comment_count, children_count), created_by / updated_by |
| Revision | id, page_id, parent_revision_id, author_id, action, message, content_hash, meta_hash, chain_hash, created_at; immutable |
| Blob | sha256, data, size — content-addressed дедуп контента |
| RevisionMeta | снимок title / tags / permissions_version / page_type / is_deleted / is_locked / render-opts |
| Tag / PageTag / RevisionTag | тег; текущие теги страницы; снимок тегов в ревизии |
| Rating | page_id, user_id, value ±1; score кэшируется в Page |
| Comment | threaded (parent_id), body_source / body_html, soft delete, модерация |
| Attachment | page_id, filename, content_type, size, sha256, версии, soft delete; meta JSONB (сюда blurhash) |
| PageLink | from_page_id, from_revision_id, to_page_id, kind — backlinks, строится из AST при сохранении |
| PageWatch | page_id, user_id, mode |
| Redirect | from full_name → page_id |
| AuditEvent | actor, action, target, before/after ref, request_id, ip; append-only |

`page_type`: `page | component | theme | system | redirect`. `theme` и `template` — одна сущность. `component` находится на уровне каркаса: панели сайта — это компоненты, наполняющие DOM-зоны.

## 7. Git-подобная история страниц

- Любое изменение страницы создаёт новую immutable-ревизию; старые ревизии не меняются.
- Хеши целостности: `content_hash = SHA256(source)`, `meta_hash = SHA256(canonical meta)`, `chain_hash = SHA256(parent_chain_hash + page_id + revision_id + action + author + timestamp + content_hash + meta_hash)`. Цепочка проверяема.
- Revert = новая ревизия с parent = current, контентом и метой целевой ревизии, action = revert. Не деструктивно.
- Delete / restore — ревизии с флагом is_deleted; физическое удаление только отдельной админ-операцией, вне обычного потока.
- Rename: page_id стабилен, slug меняется, создаётся Redirect со старого full_name.
- Concurrency: optimistic locking по `base_revision_id`; конфликт → diff-экран. Опциональный page-lock в P1.
- Изменения тегов и прав — тоже ревизии (снимок в meta), откатываются единообразно. Рейтинги и комментарии — не ревизии, а состояние/события с компенсацией при модерации.

## 8. Визуал сайта

- Каркас страницы (header / side / main / footer) — серверный скелет с DOM-зонами; зоны наполняются include'ом страниц пространства `nav:` (`nav:top`, `nav:side`, `nav:foot` — точный набор имён берётся из seed RuFoundation при миграции). Правка панели с сайта = правка страницы `nav:*` со своей историей и откатами.
- `theme` (= template) — CSS + ассеты; применяется сайтом по умолчанию или постранично через `[[include theme:foo]]`. Меняет вид панелей и контента через CSS. Отдельного FTML-layout-документа нет.
- `component` — переиспользуемый блок; параметры include = переменные `{$name}` в теле компонента (механизм ftml). Пример: `[[include component:image-block |name=... |caption=... |align=...]]`.
- Для DOM-совместимости с темами Backrooms RU рендер ведётся в `ftml::Layout::Wikidot` (legacy Wikidot HTML). Если их темы окажутся под Wikijump-HTML — переключим; уточняется по их CSS в M-Import.

## 9. Рендер-пайплайн (wikinext-render)

Поток по пайплайну ftml: наш `Includer` → `ftml::include` → `preprocess` → `tokenize` → `parse` → наш module / post-process слой → `HtmlRender` с `Layout::Wikidot` → sanitize → post-process hooks. Параллельно `TextRender` даёт plain-text для индексации в Meilisearch.

Контракт ftml, который реализуем (публичное API 1.41):

- `trait Includer<'t> { type Error; fn include_pages(&mut self, &[IncludeRef<'t>]) -> Result<Vec<FetchedPage<'t>>, Error>; fn no_such_include(&mut self, &PageRef) -> Result<Cow<'t,str>, Error>; }`, `FetchedPage { page_ref: PageRef, content: Option<Cow<str>> }`. Наш Includer резолвит страницы / компоненты / темы с проверкой прав, лимитами глубины / циклов / размера и кэшем; `content = None` → `no_such_include`.
- `ftml::include` возвращает `(String, Vec<PageRef>)` — список включённых страниц. Используется как граф include-зависимостей для invalidation кэша; include сами не парсим.
- `PageInfo { page, category, site, title, alt_title, score, tags, language }` — мета страницы, отдаваемая ftml; `category` = наш namespace (`None` = default), `language` = "ru". `ScoreValue { Integer(i64) | Float(f64) }` — рейтинг отдаём как `Integer`. `PageRef { site: Option, page, extra }` — on-site резолвим по `page` (full_name); off-site (`:wiki:page`) — interwiki в P2, иначе `no_such_include`.
- `SyntaxTree` сериализуем (serde) → кэш распарсенного AST по `content_hash + ftml_version` без повторного парсинга.

Кэш рендера: ключ = page_id + revision_id + ftml_version + render_policy_version + theme_version + component_set_hash + роль-видимость. Публичные страницы кэшируются aggressively; приватные — private / без shared cache. Invalidation через include-зависимости (PageRef из include) + Redis pub/sub между узлами.

## 10. Модули

ftml 1.41 типизирует только 5 модулей: `Backlinks`, `Categories`, `Join`, `PageTree`, `Rate`. Generic-варианта для произвольных модулей в AST нет. Совместимость по модулям — наш слой.

- Эти 5 исполняем через данные, которые ftml ждёт от хоста (например, Backlinks — из нашей таблицы PageLink).
- Остальные Wikidot-модули (`ListPages`, `Comments`, `CSS`, `Redirect`, `TagCloud`, `Files`, `CountPages`, `SiteChanges`, `Search`, `ListUsers`, форумные и админские) обрабатываем своим `ModuleRegistry` (trait `Module { name; execute(args, ctx) }`). Механизм интеграции с ftml (препроцесс-плейсхолдер до ftml + постпроцесс-подстановка после) выбирается экспериментом в M3: нужно проверить, что делает парсер ftml с неизвестным `[[module X]]` (ошибка / игнор / текст).
- Неизвестный или неподдерживаемый модуль не роняет страницу: рендерит явный placeholder и пишет в лог.
- Приоритеты: P0 — ListPages, Rate, Backlinks, Comments, CSS, Redirect; P1 — PageTree, TagCloud, Files, CountPages, SiteChanges, Search; P2 — ListUsers, Members, Watchers, Gallery, Feed, форумные, админские. Deprecated-модули Wikidot (Pages, ChildPages, PagesByTag, NextPage, PreviousPage) — алиасы.

## 11. Права

- `rank: u32` у пользователя; гость = 0, зарегистрированный по умолчанию = 1, лесенка вверх без потолка. Право с `min_rank` открыто всем рангом не ниже (ранг N наследует всё ниже).
- Группы — именованные наборы permission, union к рангу (для ролей вне линейной шкалы, например «переводчик»).
- ACL `(scope, principal, action, allow/deny)` на уровнях site / namespace / page — для точечных правил и селективного deny, совместимо с Wikidot page-permissions.
- Резолвер — одна чистая функция в `wikinext-core`, без IO, полностью под тестами: цепочка page → namespace → site, ранг даёт базовый допуск, ACL уточняет; при конфликте на уровне **deny бьёт allow** (fail-closed). Точные приоритеты резолвера RuFoundation сверяются по их коду в M-Import, и резолвер подстраивается под источник.
- Проверка прав всегда в `wikinext-app`; UI только отражает результат. Deny-правила не заменяют lock: lock страницы — отдельный флаг.

## 12. Инфраструктура

- Postgres 18.4 — primary storage; миграции forward-only; append-only audit (запрет UPDATE / DELETE, отдельная DB-роль).
- Redis — L2 shared cache + pub/sub invalidation; moka — L1 локальный hot-кэш с коротким TTL.
- Meilisearch — primary search через `SearchBackend` trait (подменяемо); индексация в той же транзакции / джобе; plain-text через ftml `TextRender`; русский поиск из коробки.
- `JobQueue` trait: in-process (dev / P0) + Redis Streams (prod / P1); контракт стабилен сразу, тяжёлая имплементация позже.
- `BlobStore` trait: FS по умолчанию, S3-совместимый опционально (P2).
- Обязательная infra с первого деплоя: Postgres + Redis + Meilisearch.

## 13. Расширяемость

Extension points — traits в `wikinext-core`, реестр в `wikinext-app`. Динамическая загрузка плагинов (dylib / WASM) отвергнута из-за безопасности и стабильности ABI; расширения компилируются в бинарник и включаются feature-флагами в конфиге / админке.

Точки расширения:

- render post-process hook — трансформация итогового HTML;
- attachment pipeline hook — `on_upload(bytes, meta) → extra_meta`;
- site-module registry — кастомные модули / макросы;
- search index hook — дополнительные поля / веса;
- job registry — фоновые задачи расширения.

Пример blurhash: upload-hook считает хеш и пишет в JSONB-мету вложения; render post-process читает мету и ставит `data-blurhash` + CSS-плейсхолдер на локальные `<img>` глобально, без правки разметки авторов. Добавление = модуль + две реализации trait + регистрация + флаг, без правки ядра.

## 14. Безопасность

- Auth: Argon2id; сессии HTTP-only / Secure / SameSite, rotation после входа, idle + absolute timeout, отзыв; rate limit на login / register.
- CSRF: токен на state-changing формы + проверка Origin / Referer.
- Markup: весь HTML через allowlist-sanitizer; запрещены script / iframe (по умолчанию) / event-handlers / javascript: / data: (кроме ограниченного); внешние ссылки с `rel="noopener noreferrer"`. Элементы ftml `Html` / `Iframe` / `Style` — под отдельной политикой (raw HTML по умолчанию выключен, CSS тем через валидацию).
- Include: лимиты глубины / числа / раскрытого размера + timeout + cycle detection (контур защиты поверх ftml include).
- Uploads: ограничение размера, allowlist MIME, content sniffing, хранение вне webroot, случайные имена, отдача через обработчик с проверкой прав.
- Headers: CSP, X-Content-Type-Options, Referrer-Policy, Permissions-Policy, HSTS.
- SQL: только параметризованные запросы SQLx; динамические sort / filter — через whitelist.
- Audit append-only для auth, page-действий, прав, файлов, модерации, админки.

## 15. Стабильность и производительность

- Ошибки: доменные в core, маппинг по слоям, HTTP-ответы в server; никаких unwrap в request path.
- Graceful shutdown, таймауты, pool-лимиты, health `/healthz` + `/readyz`, structured logs с request_id, метрики (latency, render time, DB time, cache hit).
- Миграции forward-only; backups pg_dump + WAL / PITR; периодическая проверка восстановления.
- Производительность: кэш AST + кэш рендера + ETag / 304; CPU-рендер ftml через `spawn_blocking` / render-pool; индексы (unique full_name, page_id + created_at, parent, GIN tags, GIN FTS / Meili); keyset pagination; batch loading без N+1; compression; invalidation через include-зависимости.
- Целевые метрики (ориентир, не гарантия): p95 cached page < 100–150 ms, p95 uncached render < 300–500 ms, p95 search < 200–300 ms, cache hit ratio популярных страниц > 80%.

## 16. Тестирование и CI

- Unit: валидация / slug (через wikidot-normalize), policy-резолвер (включая deny-wins), хеш-цепочка, sanitize, diff, module-args.
- Integration: миграции, репозитории, транзакции, append-only audit, concurrent-edit conflict, rollback, soft delete / restore (testcontainers Postgres).
- Web: auth / CSRF, page CRUD, history / revert, admin, API.
- Security: XSS-пейлоады, include-cycles / bombs, permission bypass, upload abuse.
- Property: slug / full_name uniqueness, hash-chain integrity, PageRef parse.
- Load: чтение популярных страниц, recent changes, search, edit under contention.
- CI: fmt, clippy `-D warnings`, test, deny, audit, проверка миграций, line-budget скрипт, `cargo tree`-контроль тяжёлых зависимостей ftml (icu / wasm под cfg).

## 17. Функционал по приоритетам

### P0 — ядро

- [ ] Workspace, toolchain, CI, linting, formatting, deny, audit.
- [ ] Конфигурация TOML / env; structured logs; request id.
- [ ] Миграции БД; каркас крейтов; `cargo tree`-контроль.
- [ ] Пользователи: регистрация, вход, выход; Argon2id.
- [ ] Сессии, CSRF, rotation, timeout; rate limit на auth.
- [ ] Policy-каркас: rank + groups + ACL + резолвер deny-wins.
- [ ] Страницы: создание, просмотр, редактирование; slug через wikidot-normalize.
- [ ] Namespaces: default, sandbox, component, nav.
- [ ] Ревизии: каждая правка создаёт ревизию; immutable.
- [ ] История, diff, revert; soft delete / restore.
- [ ] Audit log для критичных действий.
- [ ] ftml-рендер через адаптер; Includer; HtmlRender Layout::Wikidot; sanitize.
- [ ] P0-модули: ListPages, Rate, Backlinks, Comments, CSS, Redirect.
- [ ] Sandbox с отдельным namespace.
- [ ] Recent changes; теги.
- [ ] Meilisearch primary search.
- [ ] Двухуровневый кэш (L1 moka + L2 Redis) с invalidation.
- [ ] Security headers; healthcheck / readiness.
- [ ] Feature-flag реестр расширений (каркас).
- [ ] Иерархия страниц (parent).

Критерий готовности P0: регистрация и вход работают; страница создаётся / редактируется; каждая правка = ревизия; история / diff / revert работают; удаление обратимо; аудит пишет; XSS не проходит; concurrent-edit конфликт обнаруживается; поиск находит страницы; кэш попадает.

### P1 — развитие wiki-функций

- [ ] Каркас через `nav:*`-страницы; редактирование панелей с сайта.
- [ ] theme / component; постраничное оформление; theme-редактор с превью и ревизиями.
- [ ] P1-модули: PageTree, TagCloud, Files, CountPages, SiteChanges, Search.
- [ ] Include с аргументами; cycle detection; лимиты глубины / размера.
- [ ] Рейтинги; комментарии; модерация комментариев.
- [ ] Вложения с версиями; page lock.
- [ ] Watch / подписка на страницу; backlinks.
- [ ] Redis Streams job-реализация.
- [ ] blurhash как первое расширение (доказательство механизма).
- [ ] RSS / Atom для recent changes; sitemap.
- [ ] JSON API для чтения.
- [ ] Админ-панель; управление правами namespace; управление пользователями.

### P2 — расширенный функционал

- [ ] P2-модули: ListUsers, Members, Watchers, Gallery, Feed, форумные, админские.
- [ ] Interwiki (off-site PageRef).
- [ ] Экспорт истории в настоящий Git; импорт из Git.
- [ ] Drafts; branches для страниц.
- [ ] Уведомления по watch.
- [ ] S3-совместимый BlobStore.
- [ ] Prometheus metrics; OpenTelemetry.
- [ ] Пользовательские темы; theme override per user / namespace.
- [ ] API write-доступа и token auth.
- [ ] Анти-спам: captcha, link limits, new-user restrictions.
- [ ] Внешние auth-хуки.

## 18. Миграция Backrooms Wiki RU

- Источник — RuFoundation (Django). Экстрактор: Django `dumpdata` и / или их export API; родной Wikidot-снапшот не используется (теряет ревизии и голоса).
- Переносится всё: страницы, вся история ревизий, теги, рейтинги, комментарии, вложения, parent-связи, редиректы, пользователи, группы, ранги, настройки прав, темы, компоненты, `nav:*`-страницы.
- Аккаунты: логины / ранги / группы / авторство переносятся; пароли не переносятся — принудительный сброс при первом входе.
- Схема 1:1 не копируется: reader читает чужую схему, нормализатор маппит в нашу; запись транзакционная, страница за страницей, с пересчётом счётчиков / backlinks / поискового индекса. Импорт идемпотентен.
- full_name и нормализация — через wikidot-normalize, побайтовое совпадение с источником.
- Права: их группы / ранги / page-permissions маппятся в нашу ACL + rank; приоритеты резолвера сверяются с их кодом.
- Критерий готовности: случайная выборка (~100 страниц) рендерится DOM-эквивалентно оригиналу (с учётом `Layout::Wikidot`); ссылки / теги / рейтинги / файлы / история на месте; импорт повторяем без дубликатов.

## 19. Roadmap

| Milestone | Scope | Exit criteria |
|---|---|---|
| M0 | workspace, toolchain, CI, config, logging, ошибки, каркас крейтов, `cargo tree`-контроль | собирается, CI зелёный, бизнес-логики нет |
| M1 | users, sessions, Argon2id, login / register / logout, CSRF, rate limit, policy-каркас (rank + groups + ACL) | вход / выход, сессии живут / истекают, CSRF защищает |
| M2 | pages, revisions, blobs, create / edit / view, history, diff, revert, delete / restore, parent, redirect | git-цикл страницы работает, ревизии immutable, откат создаёт новую |
| M3 | ftml-адаптер: Includer, preprocess / tokenize / parse, module-слой (эксперимент с неизвестными модулями), HtmlRender Layout::Wikidot, sanitize, кэш AST, лимиты | разметка + include + P0-модули рендерятся безопасно, кэш попадает |
| M4 | sandbox, tags, recent changes, Meilisearch, cleanup | sandbox работает, теги / поиск / лента работают |
| M5 | theme / component, каркас nav:*, ratings, comments, attachments, watch, backlinks, blurhash-доказательство расширений, Redis Streams jobs | визуал правится с сайта, include-зоны работают, расширения подключаются |
| M6 | JSON API read, admin-панель, advanced permissions UI, metrics, hardening | API отдаёт страницы / ревизии, админка управляет |
| M-Import | reader под Django RuFoundation, полный импорт, сверка рендера | выборка рендерится эквивалентно, данные полны, импорт идемпотентен |

M3 начинается с прототипа против реального ftml 1.41 (поведение неизвестного `[[module]]`, сигнатуры Render / настроек) — это единственное, что читается только экспериментом и влияет на реализацию рендерера.

## 20. Риски

| Риск | Митигация |
|---|---|
| Форк FTML RuFoundation расходится с upstream 1.41 | Сравнить рендер выборки Backrooms RU; чиним своим слоем (препроцесс / модули); на их форк не переходим без отдельного решения |
| ftml покрывает только 5 модулей | ModuleRegistry + препроцесс; неизвестный модуль = placeholder, не падение |
| Поведение ftml на неизвестном `[[module]]` неизвестно | Эксперимент в M3; две ветки (плейсхолдер vs полное извлечение до ftml) |
| AGPL ftml | Движок open-source; принято |
| Тяжёлое дерево зависимостей ftml (icu, wasm) | `cargo tree`-контроль в M0; проверка, что wasm не тянется в native |
| Include DoS | Лимиты + timeout + cycle detection + кэш |
| Конкурентные правки | Optimistic locking + diff-conflict |
| Русский поиск | Meilisearch primary |
| Резолвер прав RuFoundation не прочитан | Сверка по их коду в M-Import; подстройка под источник |
| RuFoundation активно меняется | Целимся в стабильную Wikidot-семантику + их экспорт, не в их внутренности |
| Коллизия имени WikiNext | Суффикс `-Engine`; ниши разные; принято осознанно |

## 21. Что дочитывается в кодовой фазе (не блокирует манифест)

- ftml: точные сигнатуры `Render` / `HtmlRender` / `TextRender` и `WikitextSettings` / `InterwikiSettings`; поведение парсера на неизвестном модуле.
- RuFoundation: точный набор `nav:*`-страниц и DOM-скелет; Django-модели страница / ревизия / тег / рейтинг / комментарий / файл / юзер / группа; резолвер прав; дельта их форка FTML; формат export API.
- Wikidot-модули: точные параметры каждого модуля из P0 / P1 по их документации и по фактическому использованию в дампе Backrooms RU.
