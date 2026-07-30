# Документация разработчика WikiNEXT

Здесь описана **текущая реализация**, а корневой
[`README.md`](../README.md) фиксирует полную целевую архитектуру движка.
Если между ними кажется, что есть противоречие, сначала проверьте пометку
«roadmap»: манифест описывает и уже готовые части, и будущие этапы.

## С чего начать

Для первого знакомства достаточно такого порядка:

1. [`development.md`](development.md) — собрать проект и запустить его локально;
2. [`architecture.md`](architecture.md) — понять, в какой крейт должен попасть код;
3. [`core-development.md`](core-development.md) — использовать типизированные ID
   и compatibility ACL;
4. [`testing.md`](testing.md) — проверить изменение перед commit или PR.

## Быстрая карта

| Задача | Куда идти |
| --- | --- |
| Собрать проект без Docker | [Старт без Docker](development.md#старт-без-docker) |
| Запустить PostgreSQL, Meilisearch и сервер | [Полный старт с Docker](development.md#полный-старт-с-docker) |
| Настроить приложение | [Конфигурация](development.md#конфигурация) |
| Понять зависимости между крейтами | [Crate DAG](architecture.md#crate-dag) |
| Выбрать место для новой функции | [Вертикальный путь функции](architecture.md#вертикальный-путь-новой-функции) |
| Использовать `UserId`, `PageId`, `GroupId` | [Типизированные ID](core-development.md#типизированные-id) |
| Проверить право пользователя | [Compatibility ACL](core-development.md#compatibility-acl) |
| Разобраться с ошибками и секретами | [Общие инженерные правила](architecture.md#общие-инженерные-правила) |
| Прогнать все проверки CI | [Полная локальная проверка](testing.md#полная-локальная-проверка) |
| Понять критерии готовности | [Definition of Done](testing.md#definition-of-done) |

## Что уже работает

Текущая реализация включает operational slice M0:

- workspace из шести крейтов и автоматическую проверку направления зависимостей;
- строгую TOML-конфигурацию и env overrides;
- CLI-команды `serve`, `migrate`, `doctor`;
- подключение к PostgreSQL 18.4, forward-only migrations и проверку версии схемы;
- подготовку локального хранилища и проверку его доступности для записи;
- безопасно сконфигурированный диагностический клиент Meilisearch 1.45.1;
- HTTP endpoints `/healthz`, `/readyz`, `/status/search`;
- request ID, structured tracing, timeout, security headers и graceful shutdown;
- типизированные `UserId`, `PageId`, `GroupId`.

Поверх M0 отдельно готовы ранние проверяемые части следующих этапов:

- clean-room compatibility resolver прав RuFoundation и ограниченный FTML
  1.41.0 probe относятся к M0-Compat;
- изолированный Argon2id-сервис — primitive для M1, но регистрации, login и
  сессий пока нет.

## Чего пока нет

Следующее описано в манифесте, но **ещё не является готовым публичным API**:

- регистрация, login, сессии и HTTP auth flow;
- CRUD страниц и полная история ревизий;
- финальная схема `global → namespace → page` для ACL;
- production render pipeline с include, модулями и обязательным sanitizer;
- поисковые документы, outbox, индексирование и reindex;
- загрузка вложений и content-addressed blob API;
- comments, ratings, tags, themes и импорт RuFoundation.

Не добавляйте временный «почти production» API в обход этих этапов. Если новая
работа относится к roadmap, сначала закрепите контракт и критерии совместимости
тестом или отдельной заметкой в `docs/compat/`.

## Где лежит код

```text
crates/core/    чистые доменные типы и правила
crates/store/   PostgreSQL и локальное файловое хранилище
crates/render/  FTML и будущий безопасный render pipeline
crates/search/  адаптер Meilisearch
crates/app/     use cases, конфигурация и сборка сервисов
crates/server/  CLI и HTTP transport
docs/compat/    проверенные clean-room контракты совместимости
scripts/        проверки репозитория
```

## Термины

- **Крейт** — отдельный Rust package в workspace с собственной границей
  зависимостей.
- **Домен** — правила wiki, не зависящие от HTTP, SQL или конкретного поисковика.
- **Адаптер** — реализация доступа к внешней системе: PostgreSQL, файловой
  системе, FTML или Meilisearch.
- **Use case** — законченная прикладная операция, которая связывает домен и
  адаптеры.
- **Transport** — CLI или HTTP: парсит внешний ввод и переводит результат use
  case в ответ.
- **Compatibility contract** — независимо зафиксированное наблюдаемое поведение
  RuFoundation/Wikidot, защищённое тестами.

## Куда смотреть дальше

- Полная целевая модель: [`../README.md`](../README.md).
- ACL RuFoundation:
  [`compat/rufoundation-acl.md`](compat/rufoundation-acl.md).
- FTML probe: [`compat/ftml-1.41.md`](compat/ftml-1.41.md).
- Пример конфигурации: [`../config.example.toml`](../config.example.toml).
- Локальные сервисы: [`../compose.dev.yml`](../compose.dev.yml).
- Те же проверки, что запускает CI:
  [`../.github/workflows/ci.yml`](../.github/workflows/ci.yml).
- Автоматические архитектурные контракты:
  [`../scripts/check_architecture.py`](../scripts/check_architecture.py) и
  [`../scripts/check_ftml_contract.py`](../scripts/check_ftml_contract.py).
