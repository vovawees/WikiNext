# Работа с `wikinext-core`

`wikinext-core` — самый нижний и устойчивый слой WikiNEXT. Он содержит типы и
правила wiki, которые не должны зависеть от PostgreSQL, HTTP, Tokio,
Meilisearch или конкретного renderer.

Сейчас публичная полезная поверхность `core` состоит из:

- `id::{UserId, PageId, GroupId}`;
- `policy::{Action, PolicyInput, PolicySubject, PagePolicyState, resolve, ...}`.

Остальная доменная модель страниц и ревизий относится к roadmap.

Сгенерировать локальную справку по точным Rust signatures:

```bash
cargo doc --locked -p wikinext-core --no-deps --open
```

Этот файл объясняет семантику API, а Rustdoc остаётся источником точных имён,
типов и trait implementations.

## Типизированные ID

`UserId`, `PageId` и `GroupId` — разные newtype-обёртки над UUID. Они не дают
случайно передать ID страницы в функцию, которая ожидает ID пользователя.

```rust
use uuid::Uuid;
use wikinext_core::id::{PageId, UserId};

let raw_user_id = Uuid::new_v4();
let user_id = UserId::new(raw_user_id);

assert_eq!(user_id.into_uuid(), raw_user_id);

let page_id = PageId::from(Uuid::new_v4());
assert_ne!(user_id.into_uuid(), page_id.into_uuid());
```

Для такого кода потребляющий крейт должен объявить зависимости явно:

```toml
[dependencies]
wikinext-core.workspace = true
uuid.workspace = true
```

ID реализуют `Copy`, сравнение, сортировку, hashing и Serde. Serde-представление
прозрачно относительно UUID, но в Rust типы остаются несовместимыми:

```rust,compile_fail
use wikinext_core::id::{PageId, UserId};

fn load_user(_id: UserId) {}

fn wrong(page_id: PageId) {
    load_user(page_id); // ошибка компиляции: PageId не является UserId
}
```

Не размывайте эту защиту:

- не храните разные сущности как общий `Uuid` внутри доменной модели;
- преобразуйте UUID в нужный ID на границе adapter/transport;
- не добавляйте невалидное «нулевое» значение;
- если появляется новая самостоятельная сущность, добавьте новый ID-тип.

## Compatibility ACL

Текущий resolver воспроизводит независимо исследованную role/category
семантику RuFoundation. Это не финальная модель WikiNEXT
`global → namespace → page`.

Источник контракта и truth table:
[`compat/rufoundation-acl.md`](compat/rufoundation-acl.md).

### Основные типы

| Тип | Значение |
| --- | --- |
| `Action` | конкретное проверяемое действие |
| `RolePolicy` | базовые allow/restriction одной роли |
| `RoleOverride` | category override той же роли |
| `PolicyInput` | полный набор ролей и уже выбранных category overrides |
| `PolicySubject` | пользователь и все его эффективные role IDs |
| `PagePolicyState` | lock страницы и признак авторства |
| `Decision` | `allowed` плюс объяснимый `DecisionReason` |

Доступные `Action`:

```text
read, edit, create, delete, restore, rename,
manage_files, manage_authors, tag, rate, comment,
moderate, manage_acl, lock, admin
```

### Рабочий пример

Следующий пример использует существующий публичный API:

```rust
use std::collections::BTreeSet;

use uuid::Uuid;
use wikinext_core::id::{GroupId, UserId};
use wikinext_core::policy::{
    Action, DecisionReason, PagePolicyState, PolicyInput, PolicySubject,
    RolePolicy, resolve,
};

fn actions(values: impl IntoIterator<Item = Action>) -> BTreeSet<Action> {
    values.into_iter().collect()
}

let editors = GroupId::new(Uuid::new_v4());
let user_id = UserId::new(Uuid::new_v4());

let input = PolicyInput {
    roles: vec![RolePolicy {
        role_id: editors,
        allows: actions([Action::Read, Action::Edit]),
        restrictions: BTreeSet::new(),
    }],
    category_overrides: Vec::new(),
};

let subject = PolicySubject {
    user_id: Some(user_id),
    is_active: true,
    is_superuser: false,
    role_ids: [editors].into_iter().collect(),
};

let page = PagePolicyState {
    locked: false,
    subject_is_author: false,
};

let decision = resolve(&input, &subject, page, Action::Edit);

assert!(decision.allowed);
assert!(matches!(
    decision.reason,
    DecisionReason::RoleGrant { role_id } if role_id == editors
));
```

Приложение, собирающее `PolicySubject`, обязано добавить виртуальную роль
`everyone`, а для аутентифицированного пользователя ещё и `registered`.
`core` не назначает UUID этим ролям и не загружает memberships из БД.

### Порядок решения

Resolver выполняет правила в таком порядке:

1. неактивный аккаунт получает `InactiveAccount` и отказ;
2. активный superuser получает `Superuser`;
3. роли и overrides проверяются на дубликаты и неизвестные ссылки;
4. для каждой роли вычисляется `allows − restrictions`;
5. category override добавляет и снимает grants только внутри своей роли;
6. результаты ролей объединяются;
7. lock снимает изменяющие страницу действия, если нет grant `Action::Lock`;
8. автор незаблокированной страницы получает `ManageAuthors`;
9. искомый grant даёт `RoleGrant`, его отсутствие — `NoGrant`.

Важная совместимая особенность: restriction в одной роли не отменяет grant
другой роли. Это union результатов отдельных ролей, а не глобальный deny-wins.

### Fail-closed поведение

Невалидный input не игнорируется. Resolver возвращает отказ с
`DecisionReason::InvalidPolicy`:

- `DuplicateRole`;
- `DuplicateOverride`;
- `UnknownSubjectRole`;
- `UnknownOverrideRole`.

Потребитель должен логировать безопасную техническую причину для оператора, но
не превращать такой результат в allow.

### Lock страницы

На locked-странице без `Action::Lock` снимаются:

- `Edit`;
- `Delete`;
- `Rename`;
- `ManageFiles`;
- `ManageAuthors`;
- `Tag`.

Другие действия продолжают определяться grants. Если меняется этот список,
нужны compatibility evidence, обновление документа и отдельные тесты.

## Как добавить новое доменное правило

1. Сформулируйте входы, результат и invariants без упоминания SQL/HTTP.
2. Для совместимого поведения добавьте evidence и truth table в `docs/compat/`.
3. Используйте отдельные типы вместо набора слабо связанных `bool`/`String`.
4. Возвращайте объяснимый результат или типизированную ошибку.
5. Сделайте функцию детерминированной и независимой от clock/random/I/O;
   значения времени и ID передавайте снаружи.
6. Добавьте позитивные, негативные и граничные unit-тесты.
7. Только после этого подключайте правило через use case в `app`.

### Добавление `Action`

`Action` сериализуется в `snake_case`, поэтому новый variant становится частью
хранимого/API-контракта. Перед изменением:

- убедитесь, что это самостоятельное действие, а не transport detail;
- определите влияние lock и авторских исключений;
- добавьте truth-table tests;
- проверьте backward compatibility сериализации;
- обновите этот документ и compatibility note.

### Что не должно попасть в `core`

- SQL query, названия таблиц и `sqlx::PgPool`;
- Axum extractors, cookies, JSON response и HTTP status;
- Reqwest/Meilisearch DTO;
- чтение файлов или env;
- spawning задач и runtime lifecycle;
- HTML sanitizer и конкретные FTML structures.

Если чистому правилу нужны данные, `core` описывает входной тип. Загрузка этих
данных происходит в adapter, а `app` передаёт их правилу.

## Ошибки `core`

Общего `wikinext_core::Error` или `Result` намеренно нет: одна универсальная
ошибка скрыла бы разные доменные исходы. Не добавляйте такой placeholder
механически.

Предпочтительно:

- ожидаемый исход правила моделировать отдельным enum/decision;
- ошибку конкретной операции делать локальным типизированным enum;
- не включать чувствительные значения в `Display`;
- не терять вариант ошибки при передаче в `app`.

`Decision` ACL — пример результата, который не является исключительной
ошибкой: отказ в праве ожидаем и объясним.

## Тесты `core`

Быстрый цикл:

```bash
cargo test --locked -p wikinext-core
cargo clippy --locked -p wikinext-core --all-targets --all-features -- \
  -D warnings
```

Текущие тесты находятся рядом с реализацией в:

- `crates/core/src/id.rs`;
- `crates/core/src/policy.rs`.

Общие требования и Definition of Done:
[`testing.md`](testing.md).

## Следующие доменные этапы

Roadmap, а не текущий API:

- финальная иерархическая ACL-модель;
- валидированные slug/category/title;
- page/revision/content hash types;
- команды create/edit/revert/rename/delete;
- optimistic concurrency;
- author, tags, rating и comment invariants.

Конкретные решения этих этапов принимаются по манифесту и compatibility
evidence, а не путём преждевременного добавления абстракций.
