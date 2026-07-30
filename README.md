# Манифест WikiNEXT

**Название:** WikiNEXT

**Репозиторий / workspace:** `wikinext-engine`

**Основной бинарник:** `wikinext`

**Статус:** архитектура зафиксирована, реализуется M0

**Дата документа:** 2026-07-30

**Лицензия:** AGPL-3.0-or-later

> Название WikiNEXT сохраняется осознанно. Существует старый неактивный проект WikiNEXT из другой ниши; это известно и не является причиной менять бренд. До публичного коммерческого использования отдельно проверяются домены и товарные знаки, но переименование проекта не планируется.

---

## Навигация для разработчика

Этот файл — архитектурный манифест и целевое состояние WikiNEXT. Чтобы
запустить текущую реализацию или начать писать код, откройте
[`docs/README.md`](docs/README.md).

- [быстрый локальный старт](docs/development.md);
- [архитектура и границы шести крейтов](docs/architecture.md);
- [работа с типизированными ID и ACL в `wikinext-core`](docs/core-development.md);
- [тесты и Definition of Done](docs/testing.md);
- [заметки о совместимости](docs/compat/).

В руководствах явно отмечено, какой API уже существует, а что пока является
roadmap. Это важно: описанная ниже целевая wiki значительно шире текущего
operational slice M0.

---

## 1. Цель

WikiNEXT — быстрый, безопасный и стабильный wiki-движок с clean-room ядром, ориентированный на совместимость с Wikidot-семейством и полный перенос Backrooms Wiki RU с RuFoundation.

Движок должен перенести и сохранить:

- страницы и их полную историю ревизий;
- исходную Wikidot-разметку;
- теги, рейтинги и комментарии;
- вложения и их версии;
- parent-связи и редиректы;
- аккаунты, авторство, группы, ранги и права;
- темы, компоненты и страницы навигации;
- совместимое HTML-представление, достаточное для существующих CSS-тем.

WikiNEXT — движок, а не конкретная тематическая wiki. При этом одно развёртывание WikiNEXT обслуживает один wiki-сайт. Для нескольких независимых wiki поднимаются отдельные экземпляры.

Интерфейс и системный контент только русские. Инфраструктуры локализации нет намеренно.

---

## 2. Жёстко зафиксированная модель развёртывания

WikiNEXT всегда работает как single-node система:

- один физический или виртуальный сервер;
- один экземпляр приложения `wikinext`;
- один PostgreSQL;
- один Meilisearch;
- локальное файловое хранилище вложений;
- фоновые воркеры запускаются внутри процесса `wikinext`;
- локальные in-process кэши;
- один wiki-сайт на одно развёртывание.

Не поддерживаются и не проектируются заранее:

- горизонтальное масштабирование приложения;
- несколько одновременно работающих экземпляров `wikinext`;
- распределённая инвалидация кэша;
- Redis;
- Redis Streams;
- Kafka, RabbitMQ и другие внешние брокеры;
- S3 как основное хранилище;
- multi-site SaaS в одном процессе;
- распределённые блокировки;
- shared sessions между узлами.

PostgreSQL и Meilisearch могут работать отдельными системными сервисами, но находятся на том же сервере или в той же доверенной локальной сети. Meilisearch не выставляется напрямую в интернет.

---

## 3. Не-цели

В первую очередь не делаем:

- визуальный SPA-конструктор;
- универсальную CMS;
- маркетплейс плагинов;
- динамическую загрузку `dylib` или WASM-плагинов;
- полную совместимость со всеми историческими багами Wikidot;
- поддержку нескольких языков интерфейса;
- федерацию wiki;
- real-time совместное редактирование;
- распределённую архитектуру «на будущее».

Совместимость реализуется там, где она нужна для реального контента Backrooms Wiki RU и стабильной Wikidot-семантики.

---

## 4. Стек

| Компонент | Зафиксированная версия / правило | Назначение |
| --- | ---: | --- |
| Rust | **1.97.1** | основной язык, фиксируется в `rust-toolchain.toml` |
| PostgreSQL | **18.4** | единственный источник истины для структурированных данных |
| ftml | **1.41.0** | Wikidot parser, AST, include pipeline, HTML/Text render |
| wikidot-normalize | **0.12.0** | нормализация имён страниц |
| Meilisearch | **1.45.1** | основной полнотекстовый поиск с P0 |
| Axum + Tokio + Tower | стабильные, зафиксированные в `Cargo.lock` | HTTP и async runtime |
| SQLx | стабильная, зафиксированная в `Cargo.lock` | PostgreSQL, compile-time SQL checks |
| Askama | стабильная, зафиксированная в `Cargo.lock` | серверные HTML-каркасы и служебный UI |
| HTMX | зафиксированный локальный asset | интерактивность без SPA |
| moka | стабильная, зафиксированная в `Cargo.lock` | локальные AST- и render-кэши |
| Argon2id | через Rust crate `argon2` | пароли |
| tracing | стабильная | structured logs и spans |

Точные номера обычных Rust-зависимостей фиксируются в `Cargo.lock` в M0. Манифест фиксирует архитектурно значимые внешние версии, а не копирует весь lockfile.

PostgreSQL является каноническим хранилищем. Meilisearch — производный и полностью перестраиваемый индекс. Кэш — одноразовое состояние процесса.

---

## 5. Лицензия и чужой код

WikiNEXT с первого коммита лицензируется под **AGPL-3.0-or-later**.

Обязательно присутствуют:

- `LICENSE`;
- SPDX `AGPL-3.0-or-later` в workspace и публикуемых crate;
- `THIRD_PARTY_LICENSES.md`;
- автоматическая проверка лицензий зависимостей;
- ссылка на исходный код работающей версии сервиса;
- инструкция сборки и развёртывания.

Причина выбора — зависимость `ftml`, распространяемая под AGPL-3.0-or-later. Это принято как сознательное ограничение проекта.

`wikidot-normalize` используется по MIT.

Код WikiJump, Wikidot и RuFoundation не копируется. Допустимы:

- изучение наблюдаемого поведения;
- чтение открытого кода для документирования совместимости;
- сравнение входов и выходов;
- собственные golden tests;
- собственные импортёры;
- использование RuFoundation как источника данных миграции.

Нельзя переносить чужие реализации механически или маскировать скопированный код как clean-room.

---

## 6. Основные принципы

### 6.1. Single-node простота

Не вводим распределённые компоненты и абстракции, которые не нужны одному серверу. Локальная функция предпочтительнее внешнего сервиса, если внешний сервис не даёт необходимого функционала.

Meilisearch является исключением, потому что полнотекстовый поиск нужен сразу и сознательно вынесен в специализированный движок.

### 6.2. PostgreSQL — источник истины

Страница, ревизия, права, теги, задания, аудит и состояние индексации считаются сохранёнными только после успешного commit в PostgreSQL.

Meilisearch, локальные кэши и временные файлы можно удалить и восстановить.

### 6.3. Совместимость проверяется тестами

Совместимость определяется не заявлениями, а сравнением:

- исходной разметки;
- AST;
- итогового DOM;
- поведения include;
- модулей;
- прав;
- импорта и повторного импорта.

### 6.4. Безопасность — финальная граница

Любая цепочка рендера заканчивается проверяемой политикой безопасности. Расширение не может обойти sanitizer, ACL или проверку файлов.

### 6.5. Минимум speculative abstractions

Trait вводится, когда:

- реально существует несколько реализаций;
- требуется тестовый fake;
- граница отделяет внешний сервис;
- без trait возникает нежелательная связность.

Не создаём интерфейс только потому, что «когда-нибудь может появиться второй backend».

### 6.6. Явная неопределённость

Непроверенное поведение помечается как задача M0 и не превращается в выдуманный контракт.

### 6.7. Модульность без числового фетиша

Файлы и модули должны иметь одну понятную ответственность. Жёстких лимитов вида «ровно 4–12 файлов в папке» или обязательного CI-лимита строк нет. Большой файл делится, когда у него появляются разные причины для изменения, а не по формальному счётчику.

---

## 7. Workspace и крейты

Граф зависимостей без циклов:

`wikinext-server → wikinext-app → {wikinext-store, wikinext-render, wikinext-search} → wikinext-core`

| Крейт | Ответственность |
| --- | --- |
| `wikinext-core` | доменные типы, идентификаторы, ошибки, валидация, policy-модель, revision hashing, extension contracts |
| `wikinext-store` | PostgreSQL repositories, транзакции, миграции, sessions, audit, outbox, durable jobs, локальное файловое хранилище вложений |
| `wikinext-render` | адаптер ftml, includes, module layer, HTML/Text render, sanitize pipeline, diff, DOM hooks |
| `wikinext-search` | Meilisearch client, schema индекса, документы, обновление индекса, rebuild и проверка состояния |
| `wikinext-app` | use-cases, ACL checks, orchestration, локальные кэши, invalidation, embedded workers, импорт |
| `wikinext-server` | HTTP, маршруты, middleware, Askama shell, HTMX endpoints, CLI и запуск процесса |

`wikinext-core` не зависит от SQLx, Axum, Meilisearch или файловой системы.

`wikinext-app` является единственным слоем, в котором координируются права, запись страницы, аудит, outbox, кэш и внешние эффекты.

---

## 8. Доменная модель

### 8.1. Пользователи и доступ

| Сущность | Суть |
| --- | --- |
| `User` | id, login, normalized_login, email, password_hash, rank, status, created_at |
| `Group` | именованная группа и её системные свойства |
| `GroupMembership` | user_id, group_id, период действия, источник назначения |
| `PermissionAction` | типизированное действие: read, edit, create, delete, restore, rename, manage_files, rate, comment, moderate, manage_acl, admin |
| `AclRule` | scope, principal, action, effect allow/deny |
| `AclVersion` | immutable-снимок набора правил |
| `AclVersionEntry` | конкретное правило внутри версии |
| `Session` | opaque session id hash, user_id, created_at, last_seen_at, expires_at, revoked_at |

Гость существует виртуально с rank 0. Зарегистрированный пользователь по умолчанию получает rank 1.

### 8.2. Страницы и история

| Сущность | Суть |
| --- | --- |
| `Page` | стабильный id, namespace, slug, full_name, current_revision_id, current_acl_version_id, parent_page_id, counters, created_by, updated_by |
| `PageContentBlob` | sha256, source, byte_size; content-addressed дедуп исходника |
| `Revision` | immutable revision с parent, author, action, content_sha256, meta_hash, chain_hash и временем |
| `RevisionMeta` | immutable snapshot title, page_type, deleted, locked, acl_version_id, render settings и совместимых полей |
| `Tag` | нормализованный тег |
| `PageTag` | текущий набор тегов |
| `RevisionTag` | snapshot тегов конкретной ревизии |
| `Redirect` | старый или альтернативный full_name → page_id |
| `PageDependency` | from_page, from_revision, to_page или unresolved_ref, kind=link/include |
| `PageWatch` | пользовательская подписка на страницу |

`page_type`:

- `page`;
- `component`;
- `theme`;
- `system`;
- `redirect`.

### 8.3. Социальные данные

| Сущность | Суть |
| --- | --- |
| `Rating` | page_id, user_id, value; уникальная пара page/user |
| `RatingEvent` | история изменения или модерационной компенсации рейтинга |
| `Comment` | threaded comment, source, rendered_html, parent_id, moderation state, soft delete |
| `CommentRevision` | история правок комментария при включённой политике редактирования |

### 8.4. Вложения

| Сущность | Суть |
| --- | --- |
| `Attachment` | page_id, logical filename, current_version_id, soft delete |
| `AttachmentVersion` | content_type, size, sha256, uploaded_by, created_at, metadata JSONB |
| `FileBlob` | sha256, size, storage state и относительный content-addressed path |

Blurhash и другие вычисленные данные находятся в `AttachmentVersion.metadata`.

### 8.5. Системные данные

| Сущность | Суть |
| --- | --- |
| `AuditEvent` | append-only событие actor/action/target/request/ip/before/after |
| `SearchOutbox` | durable событие синхронизации PostgreSQL → Meilisearch |
| `Job` | durable фоновое задание, attempts, run_at, lock, error, idempotency_key |
| `ImportMapping` | связь внешних id RuFoundation с id WikiNEXT |
| `SchemaState` | версии внутренних схем рендера, поиска и совместимости |

---

## 9. Git-подобная история страниц

### 9.1. Правило ревизии

Любое изменение состояния страницы создаёт новую immutable-ревизию:

- изменение source;
- title;
- tags;
- page type;
- lock;
- deleted state;
- ACL;
- совместимых render options.

Рейтинги, комментарии, watch и файлы имеют собственную историю и не превращаются в page revision, если не меняют непосредственно состояние страницы.

### 9.2. Content blobs

Исходник страницы хранится в `PageContentBlob` по SHA-256. Несколько ревизий могут ссылаться на один blob.

Это сохраняется ради:

- полной совместимости истории;
- дедупликации одинакового контента;
- быстрых revert;
- проверки целостности;
- импорта больших цепочек ревизий.

### 9.3. Metadata snapshots

`RevisionMeta` хранит полный снимок необходимых метаданных, а не только номер изменившегося поля.

Права представлены ссылкой на immutable `AclVersion`, поэтому откат ревизии действительно восстанавливает исторический набор прав.

Теги снапшотятся через `RevisionTag`.

### 9.4. Hash chain

- `content_hash = SHA256(source bytes)`;
- `meta_hash = SHA256(canonical revision metadata bytes)`;
- `chain_hash = SHA256(domain_separator + parent_chain_hash + page_id + revision_id + action + author_id + timestamp + content_hash + meta_hash)`.

Canonical format является версионированным бинарным форматом WikiNEXT, а не неупорядоченным JSON.

Root revision использует нулевой parent hash.

Hash chain предназначена для обнаружения случайной порчи и несанкционированных изменений, которые не сопровождались полным пересчётом цепочки. Она не считается криптографическим доказательством против администратора БД, имеющего возможность переписать всю историю.

В P2 может быть добавлена периодическая подпись checkpoint hash внешним ключом.

### 9.5. Операции

- **Edit:** новая ревизия с parent=current.
- **Revert:** новая ревизия с parent=current и состоянием выбранной исторической ревизии.
- **Delete:** новая ревизия с `is_deleted=true`.
- **Restore:** новая ревизия с `is_deleted=false`.
- **Rename:** page_id остаётся стабильным, full_name меняется, старое имя становится Redirect.
- **ACL change:** новая AclVersion и новая page revision.
- **Tag change:** новая page revision и новый snapshot тегов.
- **Hard purge:** отдельная опасная admin-команда, не часть обычного UI.

### 9.6. Конкурентное редактирование

Каждая запись принимает `base_revision_id`.

Если `base_revision_id != current_revision_id`, запись не применяется автоматически. Пользователь получает conflict view с diff.

Page lock является отдельным флагом и не заменяется ACL.

---

## 10. Транзакция изменения страницы

Успешное изменение страницы выполняется одной PostgreSQL-транзакцией:

1. проверка optimistic lock;
2. проверка прав;
3. создание или reuse content blob;
4. создание AclVersion при изменении прав;
5. создание RevisionMeta;
6. создание Revision;
7. запись RevisionTag;
8. обновление current state в Page;
9. обновление PageTag;
10. обновление Redirect при rename;
11. запись AuditEvent;
12. запись SearchOutbox;
13. commit.

После commit:

1. локальный кэш инвалидируется;
2. embedded worker обрабатывает SearchOutbox;
3. Meilisearch обновляется с eventual consistency.

HTTP-запрос не вызывает Meilisearch внутри PostgreSQL-транзакции.

---

## 11. Права

### 11.1. Модель

- `rank: u32`;
- guest rank = 0;
- обычный зарегистрированный пользователь = 1;
- rank наследует базовые возможности нижних rank;
- группы добавляют нелинейные роли;
- ACL действует на уровнях `global → namespace → page`;
- principal может быть user или group;
- effect — allow или deny;
- lock страницы остаётся отдельным состоянием.

### 11.2. Resolver

Policy resolver:

- находится в `wikinext-core`;
- является чистой функцией без IO;
- получает полностью подготовленный контекст;
- возвращает не только bool, но и объяснение решения;
- полностью покрывается table-driven и property tests;
- не зависит от HTTP/UI;
- fail-closed при неизвестном или повреждённом состоянии.

Точная precedence-модель RuFoundation должна быть прочитана и зафиксирована в M0-Compat до реализации M1. Нельзя сначала реализовать произвольный `deny wins`, а затем переделывать весь движок во время импорта.

До завершения M0-Compat обязательны ответы:

- что является default allow/default deny;
- как взаимодействуют global, namespace и page;
- может ли более конкретный allow перебить общий deny;
- user rule против group rule;
- несколько конфликтующих group rules;
- rank против ACL;
- admin bypass;
- lock против edit permission;
- права на удалённую страницу;
- права на include приватного компонента.

### 11.3. Проверка прав

Все проверки выполняются в `wikinext-app`.

UI лишь отображает уже вычисленный результат.

Render include проверяет read permission каждой включённой страницы.

Private content не помещается в общий render cache и не индексируется в публичный Meilisearch index.

---

## 12. Namespaces и системные страницы

P0 namespaces:

- default;
- `sandbox`;
- `component`;
- `theme`;
- `nav`;
- `system`.

Namespace является частью `full_name` и политик доступа.

Точный набор legacy namespace и alias определяется инвентаризацией RuFoundation в M0-Compat.

Системные страницы не обходят обычную историю ревизий. Изменение темы, компонента или панели создаёт такую же проверяемую ревизию.

---

## 13. Визуал сайта

### 13.1. Каркас

Askama рендерит:

- общий HTML shell;
- header;
- side;
- main;
- footer;
- auth/admin UI;
- history/diff/editor pages;
- HTMX fragments.

Тело wiki-страницы рендерится через ftml.

### 13.2. Навигационные зоны

Зоны заполняются include системных страниц пространства `nav:`.

Предварительные имена:

- `nav:top`;
- `nav:side`;
- `nav:foot`.

Фактический набор и alias берутся из реального seed RuFoundation в M0-Compat.

Правка панели с сайта является обычной правкой `nav:*` страницы с историей, diff и revert.

### 13.3. Themes и components

`theme` — versioned CSS и связанные assets.

`component` — переиспользуемая wiki-страница с аргументами include вида `{$name}`.

Отдельного FTML layout-документа нет. HTML shell остаётся контролируемым сервером, а темы меняют внешний вид через CSS и разрешённые assets.

Рендер по умолчанию использует `ftml::Layout::Wikidot`. Если golden tests покажут, что конкретные темы RuFoundation требуют другого layout или их fork-семантики, решение принимается по результатам M0-Compat.

---

## 14. Render pipeline

Полный pipeline:

1. загрузка Page, Revision, RevisionMeta и security context;
2. проверка read permission;
3. разрешение include через наш `Includer`;
4. `ftml::include`;
5. `preprocess`;
6. `tokenize`;
7. `parse`;
8. выполнение typed ftml modules;
9. выполнение custom ModuleRegistry;
10. `HtmlRender`;
11. trusted pre-sanitize HTML hooks;
12. финальный allowlist sanitizer;
13. safe DOM attribute hooks;
14. Askama shell;
15. параллельный `TextRender` для поискового документа;
16. запись PageDependency;
17. помещение результата в разрешённый локальный кэш.

### 14.1. Include

Наш Includer:

- резолвит страницы, components и themes;
- проверяет ACL каждой цели;
- поддерживает аргументы;
- ограничивает глубину;
- ограничивает общее число include;
- ограничивает раскрытый размер;
- отслеживает cycles;
- имеет общий deadline рендера;
- кэширует повторный fetch в рамках одного request;
- возвращает явную безопасную ошибку при отсутствии страницы.

Off-site PageRef в P0 не загружается по сети. Он становится interwiki link или безопасным placeholder по политике.

### 14.2. Sanitizer и hooks

Произвольный HTML hook никогда не выполняется после sanitizer.

Есть два класса расширений:

- `pre_sanitize_html_hook` — может менять HTML, после него всегда запускается sanitizer;
- `safe_dom_attribute_hook` — работает с уже очищенным DOM и может добавлять только allowlisted class/data/aria атрибуты.

Blurhash реализуется через safe DOM hook: добавляет class и `data-blurhash`, но не вставляет произвольный script/style.

### 14.3. AST cache

`SyntaxTree` кэшируется только после проверки реальной serde-совместимости ftml 1.41.0.

Ключ:

`content_hash + ftml_version + parser_settings_fingerprint`

Кэш локальный, bounded и одноразовый.

---

## 15. Модули

ftml 1.41.0 типизирует ограниченный набор модулей, включая:

- Backlinks;
- Categories;
- Join;
- PageTree;
- Rate.

Остальные Wikidot-модули реализует наш `ModuleRegistry`.

Контракт:

```text
Module {
    name
    parse_args
    authorize
    execute
    cache_policy
}
```

Неизвестный модуль:

- не роняет страницу;
- не исчезает молча;
- рендерит диагностический placeholder;
- пишет structured warning с page_id и module name.

Поведение ftml на неизвестном `[[module X]]` проверяется прототипом в M0, до реализации основного renderer. После теста фиксируется один механизм:

- извлечение custom modules до ftml и безопасные placeholders;
- либо обработка сохранённых AST-узлов, если ftml их предоставляет.

### Приоритет модулей

**P0:**

- ListPages;
- CSS;
- Redirect;
- Backlinks;
- Categories;
- Join.

**P1:**

- Rate;
- Comments;
- PageTree;
- TagCloud;
- Files;
- CountPages;
- SiteChanges;
- Search;
- Watchers.

**P2:**

- ListUsers;
- Members;
- Gallery;
- Feed;
- форумные;
- административные;
- deprecated aliases.

Rate и Comments не считаются P0, пока отсутствуют их полноценные модели и UI.

---

## 16. Search: Meilisearch с первого релиза

Meilisearch обязателен для P0 и является основным полнотекстовым поиском.

PostgreSQL не дублирует полноценный FTS. В нём остаются:

- точный поиск страницы по full_name;
- lookup по id;
- административные выборки;
- источник данных для полного reindex.

### 16.1. Search document

Индексируемый документ содержит:

- page_id;
- full_name;
- namespace;
- slug;
- title;
- plain_text;
- tags;
- page_type;
- rating_score;
- created_at;
- updated_at;
- revision_id;
- search_schema_version.

Searchable fields с приоритетом:

1. title;
2. full_name;
3. tags;
4. plain_text.

Filterable fields:

- namespace;
- tags;
- page_type.

Sortable fields:

- rating_score;
- created_at;
- updated_at;
- title.

### 16.2. Безопасность поиска

В основной индекс попадают только страницы, которые:

- доступны анонимному пользователю;
- не зависят от user/group ACL;
- не включают приватный контент;
- не удалены;
- не скрыты системной политикой.

Это исключает утечку title, tags, excerpts и существования приватной страницы через поиск.

P0 не пытается кодировать произвольный ACL resolver в Meilisearch filters.

Безопасный поиск по непубличным страницам является отдельной P2-задачей и допускается только после отдельной модели visibility и security tests.

### 16.3. Transactional outbox

При изменении страницы событие записывается в `SearchOutbox` в той же PostgreSQL-транзакции.

Embedded worker:

- выбирает события батчами;
- блокирует их через `FOR UPDATE SKIP LOCKED`;
- строит актуальный документ из PostgreSQL;
- выполняет upsert/delete в Meilisearch;
- подтверждает событие после успешного ответа;
- повторяет ошибки с exponential backoff;
- отправляет постоянно падающее событие в dead state;
- работает идемпотентно.

Несколько событий одной страницы могут быть схлопнуты до последнего состояния.

### 16.4. Reindex

Команда `wikinext search reindex`:

1. создаёт индекс новой schema version;
2. экспортирует public documents из PostgreSQL;
3. загружает батчами;
4. проверяет количество и sample queries;
5. переключает alias;
6. удаляет старый индекс после подтверждения.

Meilisearch не входит в канонический backup: индекс всегда можно перестроить из PostgreSQL.

При недоступности Meilisearch:

- чтение и редактирование страниц продолжаются;
- SearchOutbox накапливается;
- search endpoint возвращает явный degraded response;
- `/readyz` не обязан выключать весь сайт из-за временной поломки поиска;
- отдельный search health показывает проблему.

---

## 17. Кэширование

Используется только локальный `moka`.

Кэши:

- parsed AST;
- публичный rendered body;
- короткий lookup page metadata;
- request-local include fetch.

Redis и L2 cache отсутствуют.

### 17.1. Public render cache

В общий process cache попадает только страница, для которой доказано:

- anonymous read разрешён;
- вся include dependency closure также public;
- вывод не зависит от текущего пользователя;
- модуль не имеет user-specific output;
- тема и компоненты публичны.

Ключ включает:

- page_id;
- revision_id;
- ftml_version;
- parser settings fingerprint;
- render policy version;
- module registry version;
- theme revision;
- component dependency fingerprint.

Private, group-only и user-specific страницы рендерятся без shared render cache.

### 17.2. Invalidation

PageDependency сохраняет include/link graph последнего актуального рендера.

После изменения страницы:

- удаляется её cache entry;
- по reverse include graph находятся зависимые страницы;
- invalidation идёт транзитивно;
- ограничение глубины защищает от повреждённого графа;
- при сомнении очищается весь render cache.

Так как процесс один, pub/sub не нужен.

---

## 18. Фоновые задания

Фоновые задания хранятся в PostgreSQL и выполняются embedded workers внутри `wikinext`.

Типовые задания:

- SearchOutbox processing;
- render dependency rebuild;
- attachment metadata;
- blurhash;
- sitemap/RSS;
- notification delivery;
- import batches;
- orphan cleanup;
- integrity verification.

`Job` содержит:

- kind;
- payload version;
- state;
- priority;
- run_at;
- attempts;
- max_attempts;
- locked_at;
- locked_by;
- last_error;
- idempotency_key;
- created_at;
- finished_at.

Правила:

- обработчики идемпотентны;
- lock имеет timeout;
- crash процесса не теряет job;
- retry использует backoff;
- poison job не блокирует очередь;
- graceful shutdown перестаёт брать новые jobs и завершает текущие до deadline.

Отдельный `JobQueue` trait не создаётся, пока существует только PostgreSQL implementation.

---

## 19. Вложения и локальный BlobStore

Вложения хранятся на локальном диске сервера в content-addressed структуре:

```text
data/blobs/ab/cd/<full-sha256>
```

Upload pipeline:

1. ограничение размера до чтения всего body;
2. запись во временный файл;
3. вычисление SHA-256;
4. content sniffing;
5. проверка allowlist MIME;
6. дополнительная проверка опасных форматов;
7. fsync временного файла;
8. atomic rename в blob path;
9. PostgreSQL-транзакция AttachmentVersion;
10. orphan cleanup при сбое между filesystem и DB.

Файлы не лежат в webroot.

Выдача идёт через контроллер, который:

- проверяет read permission страницы;
- выставляет корректный Content-Type;
- задаёт Content-Disposition;
- поддерживает ETag;
- запрещает MIME confusion;
- применяет CSP/sandbox для потенциально активного контента.

Для public assets темы может быть отдельная строгая allowlist-политика.

S3 backend не планируется, так как сервер всегда один.

Backup обязан включать PostgreSQL и `data/blobs`.

---

## 20. Расширяемость

Динамические плагины отвергнуты.

Расширения:

- компилируются вместе с WikiNEXT;
- регистрируются явно;
- включаются compile-time feature или runtime config;
- не получают прямого обхода app-layer;
- используют типизированные contracts.

Точки расширения:

- module registry;
- pre-sanitize render hook;
- safe DOM attribute hook;
- attachment processing hook;
- search document enrichment;
- job handler registry;
- audit enrichment;
- import transformer.

Первое доказательное расширение — blurhash:

- attachment hook вычисляет blurhash;
- metadata сохраняется в AttachmentVersion;
- safe DOM hook на локальных изображениях добавляет class и `data-blurhash`;
- клиентский локальный script отображает placeholder;
- исходная wiki-разметка не меняется.

---

## 21. Безопасность

### 21.1. Auth

- Argon2id;
- параметры хеширования хранятся в hash string и обновляются при входе;
- optional server-side pepper хранится вне БД;
- opaque session token;
- в БД хранится hash токена;
- HttpOnly;
- Secure;
- SameSite=Lax или Strict по конкретному flow;
- rotation после login и privilege change;
- idle timeout;
- absolute timeout;
- revoke all sessions;
- local in-process rate limit на login/register/reset;
- audit auth events.

### 21.2. CSRF

- token для state-changing browser requests;
- проверка Origin;
- fallback Referer policy;
- никакого изменения состояния через GET;
- SameSite cookie не считается единственной защитой.

### 21.3. Markup

- финальный allowlist sanitizer;
- `script` запрещён в пользовательской разметке;
- event handlers запрещены;
- `javascript:` запрещён;
- `data:` ограничен;
- iframe выключен по умолчанию;
- raw HTML выключен по умолчанию;
- внешние ссылки получают `rel="noopener noreferrer"`;
- пользовательский CSS не получает доступ к опасным URL-схемам;
- theme CSS редактируется только отдельным permission;
- CSP остаётся последним ограничением.

### 21.4. Include

- ACL каждой страницы;
- cycle detection;
- max depth;
- max count;
- max expanded bytes;
- render deadline;
- bounded module output;
- отсутствие сетевого include в P0.

### 21.5. SQL

- параметризованные SQLx queries;
- dynamic sort/filter только через enum/whitelist;
- отдельная migration owner role;
- application role без DDL;
- audit table защищена от UPDATE/DELETE application role;
- DB constraints дублируют критичные доменные инварианты.

### 21.6. Headers

- Content-Security-Policy;
- X-Content-Type-Options;
- Referrer-Policy;
- Permissions-Policy;
- HSTS после корректного HTTPS;
- frame-ancestors;
- безопасная cookie policy.

### 21.7. Search security

- Meilisearch доступен только приложению;
- master key обязателен;
- browser не получает административный Meili key;
- public index не содержит private pages;
- search snippets дополнительно escape/sanitize;
- index schema не публикует скрытые поля.

---

## 22. Надёжность и консистентность

- никаких `unwrap`/`expect` в request и worker paths;
- typed errors по слоям;
- request_id и job_id во всех логах;
- graceful shutdown;
- bounded DB pool;
- bounded render blocking pool;
- deadlines для HTTP, render и Meili;
- retry только для идемпотентных операций;
- schema migrations forward-only;
- backup до migration;
- periodic restore test;
- PostgreSQL constraints для current revision, unique full_name и membership;
- outbox гарантирует восстановление поискового индекса;
- local cache можно полностью очистить;
- Meilisearch можно полностью удалить и перестроить;
- filesystem reconciliation обнаруживает missing/orphan blobs;
- integrity checker проверяет revision chains и attachment hashes.

### Health endpoints

- `/healthz` — процесс жив;
- `/readyz` — PostgreSQL доступен, schema version совместима, обязательные директории доступны;
- `/status/search` — Meilisearch и backlog outbox;
- `/status/jobs` — queue lag и failed jobs;
- `/status/storage` — доступность blob directory.

Временная поломка Meilisearch переводит поиск в degraded state, но не делает всю wiki недоступной.

---

## 23. Backup и восстановление

Канонический backup:

- PostgreSQL base backup / dump;
- WAL/PITR по выбранной production-схеме;
- snapshot `data/blobs`;
- конфигурация без секретов;
- отдельно сохранённые secrets;
- manifest версий.

Meilisearch backup необязателен: после восстановления PostgreSQL выполняется `wikinext search reindex`.

Проверка восстановления выполняется периодически на отдельной временной директории или машине.

Single-node означает единую точку отказа, поэтому backup и проверенный restore являются обязательной частью production, а не P2-фичей.

---

## 24. Производительность

Цели являются ориентирами после измерения на целевом сервере:

- p95 public cached page: `< 100–150 ms`;
- p95 uncached render: `< 300–500 ms`;
- p95 search: `< 200–300 ms`;
- cache hit популярных public pages: `> 80%`;
- edit transaction без индексации: `< 250 ms` при обычной странице;
- SearchOutbox lag в норме: `< 5 s`.

Методы:

- AST cache;
- public render cache;
- ETag/304;
- bounded `spawn_blocking` render pool;
- keyset pagination;
- batch loading;
- no N+1;
- индексы PostgreSQL;
- батчи Meilisearch;
- outbox coalescing;
- compression;
- streaming больших файлов;
- preload только измеренно полезных данных.

Не оптимизируем ценой нарушения ACL, истории или корректности.

---

## 25. Индексы и ограничения PostgreSQL

Обязательные ограничения и индексы:

- unique `normalized_login`;
- unique normalized email при политике уникальности;
- unique `full_name`;
- unique `(page_id, user_id)` для Rating;
- unique `(user_id, group_id)` для active membership;
- unique `(page_id, tag_id)` для PageTag;
- unique content sha256;
- unique file sha256;
- index revisions `(page_id, created_at desc, id desc)`;
- index comments `(page_id, created_at, id)`;
- index parent pages;
- index reverse PageDependency по `to_page_id`;
- index SearchOutbox по state/run_at;
- index Job по state/run_at/priority;
- partial indexes для active sessions;
- foreign keys с осознанными delete rules;
- check constraints для rating value, states и rank.

`Page.current_revision_id` должен ссылаться на ревизию той же страницы. Если обычный FK не выражает это напрямую, используется составной unique/FK или deferred constraint trigger.

---

## 26. Тестирование

### Unit

- normalization;
- full_name;
- policy resolver;
- canonical metadata encoding;
- content/meta/chain hashes;
- module args;
- sanitizer policies;
- search document mapping;
- job backoff;
- import normalization.

### Integration

- migrations;
- repositories;
- page transaction;
- optimistic conflict;
- revert;
- delete/restore;
- ACL version rollback;
- append-only audit;
- outbox idempotency;
- worker crash recovery;
- attachment reconciliation;
- Meilisearch upsert/delete/reindex.

Используются testcontainers PostgreSQL и Meilisearch.

### Golden compatibility

- реальные страницы Backrooms Wiki RU;
- includes;
- components;
- themes;
- nav pages;
- modules;
- edge-case markup;
- DOM normalization;
- visual screenshot tests для критичных тем;
- сравнение fork FTML RuFoundation и upstream ftml.

### Security

- XSS payload corpus;
- CSS escapes;
- malformed URLs;
- include cycles/bombs;
- permission bypass;
- private include leakage;
- search metadata leakage;
- CSRF;
- session fixation;
- upload polyglots;
- path traversal;
- decompression bombs;
- oversized module output.

### Property tests

- slug/full_name invariants;
- revision chain integrity;
- ACL resolver determinism;
- PageRef parse;
- import idempotency;
- no duplicate current tags;
- reindex equivalence.

### Load

- popular page reads;
- cache stampede;
- uncached render;
- ListPages;
- recent changes;
- search;
- edits under contention;
- large history;
- import throughput;
- attachment download.

---

## 27. CI

Обязательные проверки:

- `cargo fmt --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- unit/integration tests;
- doctests;
- `cargo deny`;
- `cargo audit`;
- license compatibility;
- migration apply на чистой БД;
- migration apply на previous schema fixture;
- SQLx offline metadata check;
- forbidden dependency directions;
- `cargo tree` review для ftml features;
- security test subset;
- golden render subset;
- reproducible release build.

Toolchain фиксирован, поэтому `-D warnings` не зависит от внезапного обновления compiler в CI.

Жёсткого CI-лимита строк в файле нет.

---

## 28. Функционал по приоритетам

### P0 — первая полноценная wiki

- workspace, toolchain, AGPL license;
- config TOML/env;
- structured logs;
- migrations;
- users;
- register/login/logout;
- PostgreSQL sessions;
- CSRF;
- auth rate limit;
- rank/groups/ACL;
- страницы create/view/edit;
- full revision model;
- history/diff/revert;
- rename/redirect;
- soft delete/restore;
- parent hierarchy;
- tags;
- recent changes;
- ftml render;
- includes;
- sanitizer;
- P0 modules;
- sandbox;
- backlinks;
- Meilisearch;
- SearchOutbox;
- reindex;
- local AST/render cache;
- transitive invalidation;
- audit;
- health/readiness;
- backup/restore procedure;
- CLI doctor/integrity checks.

**Критерий P0:** пользователь может зарегистрироваться, войти, создать и править страницу; каждое изменение создаёт совместимую ревизию; история, diff, revert, права, include, поиск, tags и backlinks работают; XSS и private search leak не проходят; после удаления Meilisearch индекс полностью восстанавливается.

### P1 — полный основной функционал Backrooms Wiki

- `nav:*` shell;
- themes;
- components;
- theme editor с preview;
- ratings;
- comments;
- moderation;
- attachment versions;
- files module;
- page watch;
- page lock UI;
- P1 modules;
- blurhash;
- RSS/Atom;
- sitemap;
- JSON read API;
- admin panel;
- user/group/ACL management;
- failed jobs UI;
- search backlog UI.

### P2 — расширенный функционал

- P2 modules;
- interwiki;
- Git export/import;
- drafts;
- page branches;
- watch notifications;
- Prometheus/OpenTelemetry;
- secure private-page search;
- write API;
- token auth;
- anti-spam;
- captcha adapter;
- external auth hooks;
- signed history checkpoints;
- пользовательские theme overrides, если они реально нужны.

Не появляются в P2:

- Redis;
- S3 primary storage;
- multi-node;
- multi-site;
- dynamic plugins.

---

## 29. Миграция Backrooms Wiki RU

Источник — RuFoundation на Django.

Используются:

- Django `dumpdata`;
- прямой read-only доступ к БД при необходимости;
- export API;
- файловое хранилище RuFoundation;
- код RuFoundation как спецификация поведения.

Родной Wikidot snapshot не является основным источником, если он теряет историю, голоса или авторство.

### 29.1. Переносимые данные

- users;
- logins;
- status;
- ranks;
- groups;
- memberships;
- pages;
- full revision history;
- source;
- titles;
- tags;
- page types;
- ACL;
- deleted/locked states;
- ratings;
- comments;
- attachments и versions;
- parent links;
- redirects;
- themes;
- components;
- nav pages;
- timestamps;
- authorship;
- external ids.

Пароли не переносятся. Для импортированных аккаунтов выполняется принудительная установка нового пароля через безопасный recovery flow.

### 29.2. Import pipeline

1. read source;
2. normalize;
3. map external ids;
4. validate references;
5. import users/groups;
6. import pages;
7. import content blobs;
8. import revisions chronologically;
9. import ACL versions и revision snapshots;
10. import tags;
11. import redirects/parents;
12. import ratings/comments;
13. import attachments;
14. rebuild current counters;
15. rebuild dependencies;
16. enqueue search documents;
17. reindex Meilisearch;
18. verify checksums и sample pages.

Импорт:

- идемпотентен;
- resumable;
- batch-based;
- имеет dry-run;
- пишет подробный отчёт;
- не создаёт дубликаты;
- хранит ImportMapping;
- сохраняет исходные timestamps;
- сохраняет внешний revision id в import metadata;
- рассчитывает WikiNEXT chain hash поверх импортированной последовательности.

### 29.3. Проверка

Минимум:

- случайная выборка не менее 100 страниц;
- все критичные theme/component/nav pages;
- страницы с глубокими include;
- страницы с нестандартными модулями;
- страницы с длинной историей;
- private pages;
- deleted pages;
- attachments;
- comments и ratings.

Проверяется:

- DOM-эквивалентность;
- видимые различия;
- ссылки;
- tags;
- ratings;
- comments;
- files;
- history;
- authors;
- ACL;
- повторный импорт;
- отсутствие search leak.

---

## 30. CLI

Один бинарник `wikinext`.

Основные команды:

```text
wikinext serve
wikinext migrate
wikinext doctor
wikinext integrity verify
wikinext history verify
wikinext search reindex
wikinext search status
wikinext jobs status
wikinext jobs retry <id>
wikinext attachments reconcile
wikinext import rufoundation --config import.toml
wikinext import rufoundation --dry-run
```

`wikinext serve` запускает:

- HTTP server;
- SearchOutbox worker;
- durable job workers;
- periodic maintenance;
- graceful shutdown coordinator.

Отдельный worker process не нужен.

---

## 31. Production deployment

Минимальный production stack на одном сервере:

- reverse proxy с TLS;
- `wikinext`;
- PostgreSQL 18.4;
- Meilisearch 1.45.1;
- локальный `data/blobs`;
- backup storage.

Рекомендуемая изоляция:

- отдельные system users;
- Meilisearch слушает loopback/private interface;
- PostgreSQL не открыт публично;
- секреты находятся в защищённом env file/secret store;
- приложение не запускается от root;
- data directories имеют минимальные права;
- reverse proxy ограничивает body size и timeouts;
- systemd restart policy без бесконечного crash loop;
- disk space alerts;
- backup alerts.

Контейнеры допустимы, но не обязательны. Архитектура не зависит от Docker.

---

## 32. Roadmap

| Milestone | Scope | Exit criteria |
| --- | --- | --- |
| **M0** | workspace, AGPL, toolchain, CI, config, errors, logging, crate graph, PostgreSQL/Meili dev environment | clean build, CI green, migrations skeleton, сервисы проверяются doctor |
| **M0-Compat** | прототип ftml 1.41, unknown modules, real RuFoundation ACL resolver, FTML fork delta, nav inventory, module inventory, sample import | contracts прав и renderer зафиксированы тестами до основной реализации |
| **M1** | users, sessions, Argon2id, login/register/logout, CSRF, rate limit, rank/groups/ACL | auth flow и точный resolver работают, session revoke проверен |
| **M2** | pages, content blobs, revisions, metadata/ACL snapshots, tags snapshots, edit/history/diff/revert/delete/restore/rename | полный immutable revision cycle и integrity verify работают |
| **M3** | ftml pipeline, Includer, P0 modules, sanitizer, dependency graph, AST/public render cache | разметка и includes безопасно рендерятся, transitive invalidation работает |
| **M4** | tags UI, recent changes, backlinks, Meilisearch schema, SearchOutbox, reindex, sandbox, hardening | P0 завершён; поиск перестраивается с нуля и не раскрывает private pages |
| **M5** | nav shell, themes, components, preview, ratings, comments | визуал и основной социальный функционал редактируются с сайта |
| **M6** | attachments, versions, Files, watch, P1 modules, blurhash, RSS/sitemap | основной функционал Backrooms Wiki готов |
| **M7** | admin panel, JSON read API, permissions UI, operations UI, production hardening | сайт управляется без ручного SQL, восстановление проверено |
| **M-Import** | полный RuFoundation reader/importer, dry-run, idempotency, DOM/data verification | данные полны, выборка эквивалентна, повторный импорт не создаёт дубликаты |

M0-Compat является блокирующим для M1 и M3 в соответствующих частях. Семантика прав не откладывается до конца проекта.

---

## 33. Риски

| Риск | Митигация |
| --- | --- |
| fork FTML RuFoundation расходится с upstream 1.41 | M0-Compat, golden tests, собственный compatibility layer |
| ftml ограниченно поддерживает modules | ModuleRegistry и ранний prototype неизвестных modules |
| тяжёлое дерево ftml | feature audit и `cargo tree`; отключение ненужных default features после проверки |
| AGPL ограничивает лицензию | AGPL-3.0-or-later с первого коммита, принято |
| ACL реализован неверно | resolver читается до M1 и фиксируется compatibility tests |
| include DoS | limits, deadline, cycle detection, bounded output |
| private data попадает в поиск | индекс только anonymous-public pages, security tests |
| Meilisearch недоступен | durable outbox, degraded search, полный reindex |
| Meilisearch version migration | индекс производный; новый index + alias swap вместо доверия внутреннему DB format |
| cache отдаёт чужой HTML | shared cache только для доказанно public dependency closure |
| DB и filesystem расходятся | atomic file placement, reconciliation, orphan cleanup, backups |
| revision chain создаёт ложное чувство защиты | явно ограниченная модель угроз; optional signed checkpoints |
| RuFoundation меняется во время разработки | фиксированный migration snapshot и versioned importer |
| single-node является точкой отказа | обязательные backup, PITR, blob snapshots и restore drills |
| имя WikiNEXT пересекается со старым проектом | выбор названия осознанно зафиксирован; перед публичной коммерциализацией отдельная проверка бренда |

---

## 34. Решения, которые больше не пересматриваются без новой причины

- название — WikiNEXT;
- лицензия — AGPL-3.0-or-later;
- язык UI — только русский;
- один deployment = один wiki-сайт;
- один сервер и один app instance;
- Redis отсутствует;
- S3 отсутствует;
- PostgreSQL — source of truth;
- Meilisearch обязателен с P0;
- Meilisearch index перестраиваем;
- фоновые задания — PostgreSQL + embedded workers;
- вложения — локальный content-addressed filesystem;
- полная revision/meta/tag/ACL history сохраняется;
- динамические плагины отсутствуют;
- renderer заканчивается sanitizer;
- private pages не попадают в public search;
- права RuFoundation изучаются до реализации ACL;
- импорт должен быть идемпотентным.

---

## 35. Что обязательно уточняется до соответствующей кодовой фазы

### До M1

- точная precedence прав RuFoundation;
- admin bypass;
- lock semantics;
- импорт групп и rank.

### До M3

- точные сигнатуры ftml 1.41.0;
- поведение неизвестного module;
- serde стабильность SyntaxTree;
- нужный Layout;
- CSS/DOM compatibility profile;
- лимиты include.

### До M4

- schema Meilisearch;
- ranking rules на реальном русском контенте;
- stop words/synonyms;
- public-page eligibility algorithm;
- outbox batch size.

### До M-Import

- Django models и их версии;
- формат файлов;
- полный module inventory;
- nav pages;
- FTML fork delta;
- migration cutover procedure.

Эти пункты являются исследовательскими задачами с конкретными exit criteria, а не поводом оставлять архитектуру неопределённой.

---

## 36. Текущее состояние реализации

Первый operational slice M0 содержит:

- фиксированный crate DAG и автоматическую проверку его направления;
- строгую TOML-конфигурацию с env overrides и редактированием секретов в Debug;
- команды `serve`, `migrate`, `doctor`;
- PostgreSQL pool и forward-only migration skeleton;
- отдельный Meilisearch adapter с проверкой версии;
- локальные data directories вне webroot;
- `/healthz`, `/readyz`, `/status/search`;
- request id, structured tracing, deadlines, security headers и graceful shutdown.

Локальный запуск и полный набор команд описаны в
[`docs/development.md`](docs/development.md).

Ограниченный renderer probe закрепляет FTML 1.41.0, `Layout::Wikidot`,
простой HTML/Text render, fallback неизвестных модулей и serde-поведение AST.
Результат и его границы описаны в
[`docs/compat/ftml-1.41.md`](docs/compat/ftml-1.41.md); sanitizer и полный
production pipeline в этот прототип не входят.

Role/category precedence RuFoundation уже закреплён clean-room compatibility
truth table и исполняемыми тестами в `wikinext-core`. Результат описан в
[`docs/compat/rufoundation-acl.md`](docs/compat/rufoundation-acl.md).
Финальная ACL-модель WikiNEXT `global → namespace → page` остаётся открытой
задачей M1 и не подменяется этим compatibility resolver.
