# Тестирование и готовность изменения

Проверки разделены на быстрый цикл одного крейта и полный набор, совпадающий с
CI. Не называйте изменение проверенным, если запускали только компиляцию.

## Быстрый цикл

Во время разработки запускайте форматирование, clippy и тесты затронутого
крейта:

```bash
cargo fmt --all --check
cargo test --locked -p wikinext-core
cargo clippy --locked -p wikinext-core --all-targets --all-features -- \
  -D warnings
```

Замените `wikinext-core` на нужный package:

```text
wikinext-core
wikinext-store
wikinext-render
wikinext-search
wikinext-app
wikinext-server
```

После изменения `Cargo.toml` дополнительно:

```bash
cargo check --workspace --all-targets --all-features --locked
python3 scripts/check_architecture.py
```

`--locked` гарантирует, что команда не переписывает и не обходит
зафиксированный `Cargo.lock`.

## Полная локальная проверка

Перед commit или PR из корня репозитория:

```bash
cargo fmt --all --check
python3 scripts/check_architecture.py
python3 scripts/check_ftml_contract.py
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo test --workspace --doc --locked
cargo test -p wikinext-core policy --locked
cargo test -p wikinext-render --locked
cargo build --workspace --release --locked
cargo deny check
cargo audit
```

Две команды после doc-tests намеренно повторно запускают
security/compatibility contracts отдельной видимой группой. Это облегчает
диагностику CI: поломка ACL или FTML не теряется внутри общего test log.

`cargo build --workspace --release --locked` обязателен: debug test build не
доказывает, что весь workspace собирается с release profile.

Установить два дополнительных инструмента:

```bash
cargo install --locked cargo-deny cargo-audit
```

`cargo audit` использует обновляемую advisory database. Результат относится к
моменту запуска, поэтому для security-sensitive изменения важна свежая
проверка, а не старый лог.

## Проверка с внешними сервисами

Unit-тесты не доказывают, что версии PostgreSQL, migrations и Meilisearch
согласованы. Для operational slice:

```bash
cp config.example.toml config.toml
docker compose -f compose.dev.yml up -d --wait
WIKINEXT_DATABASE_MIGRATION_URL='postgres://wikinext_migrator:wikinext-migrator-dev-password@127.0.0.1:5432/wikinext' \
  cargo run --locked -- migrate
cargo run --locked -- doctor
```

После запуска сервера:

```bash
cargo run --locked -- serve
```

В другом терминале:

```bash
curl --fail http://127.0.0.1:3000/healthz
curl --fail http://127.0.0.1:3000/readyz
curl --fail http://127.0.0.1:3000/status/search
```

Для автоматизации используйте JSON и exit code:

```bash
cargo run --locked -- doctor --json
```

CI поднимает оба сервиса, создаёт раздельные DDL-migrator и DML-app роли,
причём обе роли остаются `NOSUPERUSER`. После migrations CI отрицательно
проверяет, что app-роль не может создавать таблицы, менять schema state или
читать внутреннюю таблицу SQLx, и только затем запускает `doctor` от неё.
Локальный прогон без Docker проверяет Rust-код, но не заменяет этот integration
evidence.

## Какие тесты нужны

### `core`

- обычные unit-тесты чистых правил;
- truth tables для ACL и другого precedence;
- boundary cases;
- fail-closed invalid input;
- property tests для сложных invariants, когда появится соответствующий домен.

### `store`

- unit-тесты validation/redaction;
- migration test на реальном PostgreSQL;
- repository integration tests;
- rollback/constraint/concurrency cases;
- проверка совместимой версии БД.

### `render`

- unit-тесты limits и error mapping;
- golden tests на реальном Wikidot/RuFoundation corpus;
- sanitizer security tests;
- include cycles/depth/budget;
- неизвестные модули и стабильные diagnostics.

Текущие FTML tests — только ограниченный probe, не production security proof.

### `search`

- URL/header/size-limit unit tests;
- adapter tests на реальном Meilisearch;
- outbox retry/idempotency;
- ACL filtering;
- полный reindex и восстановление после недоступности.

Последние три группы относятся к roadmap до появления индексатора.

### `app`

- use-case tests с контролируемыми adapter implementations;
- порядок ACL и mutation;
- transaction boundary и compensation;
- overload/deadline behavior;
- password hashing/verification и hostile PHC input.

### `server`

- router tests без открытия TCP-порта;
- status mapping и безопасные response bodies;
- request size/timeout;
- обязательные security headers и request ID;
- auth/CSRF/session tests после появления M1.

## Правила тестового кода

- `expect()` допустим для создания fixture и должен объяснять, что именно
  обязан гарантировать тест.
- Не используйте случайную задержку как единственную синхронизацию
  concurrency-теста.
- Тест не должен зависеть от порядка запуска других тестов.
- Секреты в fixtures должны быть явно фиктивными.
- Golden fixture хранит происхождение и причину совместимости.
- Тест внешнего сервиса обязан либо поднять известную версию, либо явно
  проверить prerequisites; молчаливый pass без проверки недопустим.

## Definition of Done

Изменение готово, когда применимые пункты выполнены:

### Контракт и архитектура

- поведение и границы функции сформулированы;
- current API отделён от roadmap;
- код находится в самом нижнем подходящем крейте;
- crate DAG не нарушен;
- совместимое поведение подтверждено clean-room evidence/truth table;
- публичный API минимален и не протекает типами чужого слоя.

### Корректность и безопасность

- внешний ввод валидирован и имеет предел размера/сложности;
- ошибка типизирована, сохраняет безопасную причину и не вызывает panic;
- невалидное security-state обрабатывается fail-closed;
- секреты отсутствуют в `Debug`, `Display`, tracing и response;
- I/O имеет timeout;
- CPU-heavy работа не блокирует async worker и имеет bounded admission;
- нет `unsafe`, runtime `unwrap()` и runtime `expect()`;
- SQL parameterized, а migration forward-only;
- пользовательский HTML проходит обязательный sanitizer, если функция его
  производит.

### Тесты

- есть позитивный сценарий;
- есть негативный сценарий;
- есть важные границы и regression test исправляемого бага;
- затронутый adapter проверен integration-тестом, если это возможно;
- compatibility/security изменение имеет специализированные tests;
- полный локальный набор проверок, включая doc-tests, compatibility subsets и
  release build, прошёл без warnings.

### Эксплуатация и документация

- конфигурация имеет безопасный default или fail-fast validation;
- новая env variable добавлена в `config.example.toml` и документацию;
- migration/doctor/readiness обновлены, если затронут startup contract;
- tracing помогает диагностировать операцию и не раскрывает данные;
- документация объясняет, как использовать новый API;
- явно записано, что было проверено только компиляцией, а что — живым runtime.

Если пункт неприменим, это должно быть понятно из границы изменения. Например,
чистое новое правило `core` не требует Meilisearch integration-теста, но требует
unit-тестов и проверки DAG.

## Что считать доказательством

Формулируйте результат точно:

- «`cargo check` прошёл» — код компилируется;
- «unit-тесты workspace прошли» — проверены локальные test cases;
- «`doctor` прошёл на Compose» — доступны заданные версии внешних сервисов,
  migrations и storage;
- «security audit прошёл» — `cargo deny` и свежий `cargo audit` не нашли
  блокирующую проблему на момент запуска;
- «совместимость подтверждена» — есть source evidence, fixture/truth table и
  исполняемый regression test.

Один вид evidence не подменяет другой.
