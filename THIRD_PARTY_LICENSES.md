# Лицензии сторонних компонентов

WikiNEXT распространяется по лицензии `AGPL-3.0-or-later`.

Прямые Rust-зависимости на этапе M0:

| Компонент | Лицензия |
| --- | --- |
| `argon2` | MIT OR Apache-2.0 |
| `axum` | MIT |
| `clap` | MIT OR Apache-2.0 |
| `ftml` | AGPL-3.0-or-later |
| `http` | MIT OR Apache-2.0 |
| `reqwest` | MIT OR Apache-2.0 |
| `serde`, `serde_json` | MIT OR Apache-2.0 |
| `sqlx` | MIT OR Apache-2.0 |
| `thiserror` | MIT OR Apache-2.0 |
| `tokio` | MIT |
| `toml` | MIT OR Apache-2.0 |
| `tower`, `tower-http` | MIT |
| `tracing`, `tracing-subscriber` | MIT |
| `url`, `uuid` | MIT OR Apache-2.0 |
| `wikidot-normalize` | MIT |

Полный транзитивный состав фиксируется в `Cargo.lock`. Его лицензии и источники
проверяются командой:

```bash
cargo deny check
```

Тексты лицензий конкретных версий находятся в исходных пакетах Cargo registry.
Этот файл является сводкой и не заменяет условия соответствующих лицензий.

Среди транзитивных зависимостей также присутствуют компоненты под
`BSD-3-Clause`, `ISC`, `MIT-0`, `MPL-2.0`, `Unicode-3.0`, `Zlib` и
`CDLA-Permissive-2.0`. Все они явно перечислены в allowlist `deny.toml`;
неизвестные registry, Git-источники и wildcard-версии запрещены.
