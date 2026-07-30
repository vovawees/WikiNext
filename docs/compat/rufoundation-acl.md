# RuFoundation ACL: clean-room compatibility note

Статус: частичный результат M0-Compat. Исследован commit
[`7ce46de`](https://github.com/SCPru/RuFoundation/commit/7ce46de2339a1be437065e9230c5b15fe4fc730d).
Код RuFoundation в WikiNEXT не копируется; ниже зафиксировано наблюдаемое
поведение и независимая truth table.

## Проверенные источники поведения

- [`web/permissions/backends.py`](https://github.com/SCPru/RuFoundation/blob/7ce46de2339a1be437065e9230c5b15fe4fc730d/web/permissions/backends.py)
  собирает эффективный набор прав;
- [`web/models/roles.py`](https://github.com/SCPru/RuFoundation/blob/7ce46de2339a1be437065e9230c5b15fe4fc730d/web/models/roles.py)
  задаёт роли и category overrides;
- [`web/models/articles.py`](https://github.com/SCPru/RuFoundation/blob/7ce46de2339a1be437065e9230c5b15fe4fc730d/web/models/articles.py)
  задаёт article lock и авторское исключение;
- [`scpdev/settings.py`](https://github.com/SCPru/RuFoundation/blob/7ce46de2339a1be437065e9230c5b15fe4fc730d/scpdev/settings.py)
  включает стандартный Django backend перед role backend.

## Зафиксированная семантика

1. Гость получает виртуальную роль `everyone`.
2. Активный зарегистрированный пользователь получает `everyone`,
   `registered` и назначенные роли.
3. Внутри каждой роли сначала вычисляется `allow − restriction`.
4. Category override добавляет и удаляет права только у соответствующей роли.
5. Результаты всех ролей объединяются. Поэтому restriction одной роли не
   отменяет grant другой роли.
6. Неактивный аккаунт не получает role permissions.
7. Активный Django superuser проходит через стандартный backend.
8. Lock страницы снимает edit/delete/move/tag/file/author-management, если у
   пользователя нет права управления lock.
9. На незаблокированной странице автор получает управление списком авторов.

| Роль A | Роль B | Category override | Итог |
| --- | --- | --- | --- |
| allow read | — | — | allow |
| allow+restrict read | — | — | deny |
| restrict read | allow read | — | allow |
| allow read | — | A restrict read | deny |
| restrict read | — | A allow read | allow |
| restrict read | allow read | A restrict read | allow через B |

Исполняемая версия таблицы находится в
[`crates/core/src/policy.rs`](../../crates/core/src/policy.rs).

## Что ещё не зафиксировано

RuFoundation использует category-level overrides, а манифест WikiNEXT требует
новую версионируемую модель `global → namespace → page`. Правила precedence
между этими тремя слоями, импорт legacy role IDs и composite permissions ещё
не считаются закрытыми. До этого compatibility resolver нельзя автоматически
превращать в финальную M1 ACL schema.
