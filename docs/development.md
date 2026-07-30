# Локальная разработка

Это руководство даёт два варианта старта:

- без Docker — для работы с чистым Rust-кодом и unit-тестами;
- с Docker — для запуска PostgreSQL, Meilisearch, миграций и HTTP-сервера.

## Требования

Обязательно:

- Rust 1.97.1; нужная версия закреплена в `rust-toolchain.toml`;
- Cargo;
- Python 3 для `scripts/check_architecture.py`.

Для полного запуска:

- Docker с Compose plugin или совместимая команда `docker compose`.

Для полного набора проверок:

- `cargo-deny`;
- `cargo-audit`.

Если используется `rustup`, переход в корень репозитория автоматически выберет
закреплённый toolchain. Проверить окружение можно так:

```bash
rustc --version
cargo --version
```

## Старт без Docker

Без внешних сервисов можно собрать все крейты, изучать `core`, разрабатывать
чистые правила и запускать unit-тесты:

```bash
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-targets --locked
python3 scripts/check_architecture.py
python3 scripts/check_ftml_contract.py
```

Отдельно проверить независимый от БД endpoint `/healthz` можно его router-тестом:

```bash
cargo test --locked -p wikinext-server \
  liveness_is_dependency_independent_and_hardened
```

Команды `migrate`, успешный `doctor`, `/readyz` и полноценный `serve` требуют
доступных PostgreSQL 18.4 и Meilisearch 1.45.1. Docker не обязателен сам по себе:
можно установить эти сервисы вручную или указать адреса уже работающих
экземпляров в `config.toml`. Но один только Rust-процесс не заменяет их.

Для быстрой работы над конкретным слоем запускайте тесты одного крейта:

```bash
cargo test --locked -p wikinext-core
cargo test --locked -p wikinext-render
cargo test --locked -p wikinext-app
```

## Полный старт с Docker

### 1. Создайте локальную конфигурацию

```bash
cp config.example.toml config.toml
```

`config.toml` игнорируется Git. Значения в примере предназначены только для
локальной разработки и не подходят для production.

### 2. Запустите зависимости

```bash
docker compose -f compose.dev.yml up -d --wait
docker compose -f compose.dev.yml ps
```

Compose поднимает:

- PostgreSQL 18.4 на `127.0.0.1:5432`;
- Meilisearch 1.45.1 на `127.0.0.1:7700`.

Теги дополнительно закреплены OCI digest в `compose.dev.yml` и CI, чтобы
повторный запуск не получил другой образ под тем же именем версии.
При первой инициализации [`ops/postgres/init.sql`](../ops/postgres/init.sql)
создаёт две `NOSUPERUSER`-роли: владелец БД `wikinext_migrator` применяет DDL,
а `wikinext_app` используется только runtime. Контейнерный `postgres`
нужен лишь bootstrap-скрипту и отсутствует в конфигурации WikiNEXT.

### 3. Примените миграции и проверьте окружение

```bash
WIKINEXT_DATABASE_MIGRATION_URL='postgres://wikinext_migrator:wikinext-migrator-dev-password@127.0.0.1:5432/wikinext' \
  cargo run --locked -- migrate
cargo run --locked -- doctor
```

Успешный `doctor` печатает три проверки: `postgresql`, `meilisearch` и
`storage`. Машиночитаемый вариант:

```bash
cargo run --locked -- doctor --json
```

Если хотя бы одна проверка не прошла, команда завершится ненулевым exit code.

### 4. Запустите сервер

```bash
cargo run --locked -- serve
```

По умолчанию сервер слушает `127.0.0.1:3000`. В другом терминале:

```bash
curl --fail http://127.0.0.1:3000/healthz
curl --fail http://127.0.0.1:3000/readyz
curl --fail http://127.0.0.1:3000/status/search
```

Назначение endpoints:

| Endpoint | Что проверяет | Зависит от |
| --- | --- | --- |
| `/healthz` | HTTP-процесс жив | ни от чего внешнего |
| `/readyz` | приложение готово обслуживать основной контур | PostgreSQL, версия схемы, локальное хранилище |
| `/status/search` | производный поисковый сервис доступен | Meilisearch |

Недоступность Meilisearch не выключает `/readyz`: PostgreSQL является источником
истины, а поиск должен восстанавливаться через будущие outbox/reindex механизмы.
Сами outbox и reindex пока относятся к roadmap.

### 5. Остановите окружение

Остановить контейнеры, сохранив данные:

```bash
docker compose -f compose.dev.yml down
```

Не добавляйте `--volumes`, если не хотите намеренно удалить локальные данные
PostgreSQL и Meilisearch.

## Конфигурация

Полный шаблон находится в
[`config.example.toml`](../config.example.toml). Парсер использует
`deny_unknown_fields`: опечатка или неизвестное поле приводят к ошибке запуска,
а не игнорируются.

Путь к конфигурации можно передать двумя способами:

```bash
cargo run --locked -- --config ./config.toml doctor
WIKINEXT_CONFIG=./config.toml cargo run --locked -- doctor
```

Значения окружения переопределяют соответствующие значения TOML:

| Переменная | Назначение |
| --- | --- |
| `WIKINEXT_APP_NAME` | имя приложения |
| `WIKINEXT_BIND` | адрес HTTP-сервера, например `127.0.0.1:3000` |
| `WIKINEXT_REQUEST_TIMEOUT_MS` | общий HTTP timeout |
| `WIKINEXT_SHUTDOWN_TIMEOUT_SECONDS` | deadline graceful shutdown |
| `WIKINEXT_DATABASE_URL` | PostgreSQL DSN |
| `WIKINEXT_DATABASE_MIGRATION_URL` | process-scoped DSN DDL-роли, читается только `migrate` |
| `WIKINEXT_DATABASE_MAX_CONNECTIONS` | предел пула соединений |
| `WIKINEXT_DATABASE_ACQUIRE_TIMEOUT_MS` | ожидание соединения из пула |
| `WIKINEXT_MEILISEARCH_URL` | базовый HTTP(S) URL Meilisearch |
| `WIKINEXT_MEILISEARCH_API_KEY` | API key Meilisearch |
| `WIKINEXT_MEILISEARCH_TIMEOUT_MS` | timeout запросов к Meilisearch |
| `WIKINEXT_DATA_DIR` | корень локального хранилища |
| `WIKINEXT_LOG_FILTER` | фильтр `tracing` |
| `WIKINEXT_LOG_FORMAT` | `pretty` или `json` |

Стандартные libpq-переменные `PGHOST`, `PGPORT`, `PGUSER`, `PGPASSWORD`,
`PGDATABASE`, `PGSSLMODE`, `PGOPTIONS`, сертификатные `PGSSL*` и `PGPASSFILE`
намеренно отвергаются. Они не должны неявно менять проверенный DSN; храните всю
строку подключения только в `WIKINEXT_DATABASE_URL` (и одноразовом migration
override).

Пример разового override:

```bash
WIKINEXT_BIND=127.0.0.1:8080 \
WIKINEXT_LOG_FORMAT=json \
cargo run --locked -- serve
```

Никогда не коммитьте `config.toml`, `.env`, каталог `data/`, реальные DSN,
пароли или master key Meilisearch.

`storage.data_dir` должен указывать на отдельный каталог: корень файловой
системы, `.` и компоненты `..` отвергаются. Подготовка storage проверяет все
существующие компоненты пути на symlink; на Unix каталоги `data`, `blobs` и
`tmp`, созданные WikiNEXT, получают права `0700`. Существующий каталог сервис
никогда не делает приватным автоматически: заранее выполните `chmod 700
<data-dir>`. Если `blobs` и `tmp` уже существуют после обновления старого
checkout, примените `chmod 700 <data-dir>/{blobs,tmp}` и к ним. Иначе запуск
завершится fail-fast и оставит права без изменений.

Для PostgreSQL на loopback допускается локальное соединение без TLS. Любой
не-loopback TCP URL обязан содержать `sslmode=verify-full`; SQLx собран с
Rustls и системными корневыми сертификатами. В query-части DSN разрешён только
`sslmode`; неизвестные ключи отвергаются до SQLx, поэтому опечатка с секретом
не попадёт в warning-лог. В production задайте обычному
`database.url` только DML-права приложения, а DDL-права передавайте команде
`migrate` только через process-scoped `WIKINEXT_DATABASE_MIGRATION_URL`. Если
переменная не задана, `migrate` использует основной DSN.

Команда `migrate` загружает отдельный узкий `MigrationConfig`: ему достаточно
секции `[database]`, поэтому migration job не нужен master key Meilisearch,
storage path или HTTP-настройки. Например, `config.migrate.toml`:

```toml
[database]
url = "postgresql://wikinext_app:app-secret@db.example.test/wikinext?sslmode=verify-full"
```

Одноразовый запуск с DDL-ролью:

```bash
WIKINEXT_DATABASE_MIGRATION_URL='postgresql://wikinext_migrator:ddl-secret@db.example.test/wikinext?sslmode=verify-full' \
cargo run --locked -- --config config.migrate.toml migrate
```

Не добавляйте migration DSN в постоянное окружение `serve`: после завершения
миграции процесс должен закончиться вместе с DDL-секретом. В реальном
production передавайте значение из secret store, а не сохраняйте literal из
примера в shell history. `serve` и `doctor` fail-fast отвергают процесс, если
эта переменная всё же осталась в их окружении.

В production обе роли заранее создаёт DBA; локально ту же схему воспроизводит
одноразовый init-скрипт. Сам WikiNEXT не выдаёт себе `CREATEROLE`. База и
schema должны принадлежать migration-роли. Первая миграция выдаёт имени роли
из обычного `database.url` только `USAGE` на schema, `SELECT` на таблицу версии
схемы. Каждая следующая миграция обязана выдавать точные права только своим
runtime-таблицам; широких default grants нет, чтобы immutable revisions и audit
не получили лишние `UPDATE`/`DELETE`. Поэтому `serve` работает без DDL-прав, а
ошибка в имени или отсутствие app-роли прерывает миграцию. CI запускает
миграцию под отдельным `NOSUPERUSER` владельцем БД, затем отрицательно
проверяет запрет DDL, записи в schema state и чтения SQLx metadata для app-роли.

## CLI

Текущий бинарник предоставляет три команды:

```text
wikinext serve
wikinext migrate
wikinext doctor [--json]
```

Актуальную справку всегда можно получить из самого бинарника:

```bash
cargo run --locked -- --help
cargo run --locked -- doctor --help
```

- `serve` собирает сервисы и запускает HTTP-сервер;
- `migrate` проверяет PostgreSQL до DDL, применяет только forward-only SQL
  migrations, выдаёт app-роли минимальные runtime grants и повторно проверяет
  версию схемы;
- `doctor` параллельно проверяет PostgreSQL, Meilisearch и storage.

## Репозиторные контракты

Два Python-скрипта проверяют свойства, которые недостаточно выразить обычной
компиляцией:

```bash
python3 scripts/check_architecture.py
python3 scripts/check_ftml_contract.py
```

- `check_architecture.py` сверяет реальный crate DAG с шестислойной схемой;
- `check_ftml_contract.py` проверяет, что разрешилась ровно одна зависимость
  FTML версии 1.41.0 с ожидаемым feature surface.

Оба скрипта используют `cargo metadata --locked` и входят в CI.

## Частые проблемы

### `config.toml` не найден

Создайте его из примера или передайте `--config`:

```bash
cp config.example.toml config.toml
```

### `doctor` сообщает об отсутствующей таблице схемы

Сначала примените миграции:

```bash
WIKINEXT_DATABASE_MIGRATION_URL='postgres://wikinext_migrator:wikinext-migrator-dev-password@127.0.0.1:5432/wikinext' \
  cargo run --locked -- migrate
```

### После обновления отсутствуют роли PostgreSQL

Init-скрипты официального PostgreSQL image выполняются только при создании
пустого volume. Если disposable M0-volume был создан старой версией
`compose.dev.yml`, сначала сохраните всё нужное, затем намеренно пересоздайте
его:

```bash
docker compose -f compose.dev.yml down --volumes
docker compose -f compose.dev.yml up -d --wait
```

`--volumes` безвозвратно удаляет локальные данные PostgreSQL и Meilisearch;
не выполняйте эту команду для нужного вам окружения. Такой volume следует
мигрировать вручную, создав роли из `ops/postgres/init.sql`.

### Получена несовместимая версия сервиса

M0 намеренно проверяет точные совместимые ветки PostgreSQL 18.4 и Meilisearch
1.45.1. Сверьте запущенные images с `compose.dev.yml`.

### Порт уже занят

Остановите конфликтующий сервис либо согласованно измените port mapping в
Compose и URL/адрес в локальном `config.toml`.

### Сервер отвечает на `/healthz`, но не на `/readyz`

Это ожидаемо при проблеме PostgreSQL, неприменённой миграции или недоступном
storage. Запустите `doctor`; его отчёт разбивает проблему по компонентам.

## Следующий шаг

- Для изменения кода: [`architecture.md`](architecture.md).
- Для доменных типов и прав: [`core-development.md`](core-development.md).
- Перед отправкой изменения: [`testing.md`](testing.md).
