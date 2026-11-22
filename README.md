# lekalo-rust

Языковой адаптер [Lekalo](https://github.com/ichinya/lekalo) для Rust-сервисов и компонентов (рабочее имя адаптера — lekalo-target-rust-service).

- Протокол к ядру — lekalo.target process/JSON protocol.
- Задача создания адаптера — [#1](https://github.com/ichinya/lekalo-rust/issues/1).
- Общий план отделения — [ichinya/lekalo#241](https://github.com/ichinya/lekalo/issues/241).

## Распознавание проектов для `init --adopt`

`crates/discovery` владеет языковыми правилами обнаружения, перенесёнными из
`lekalo-core/src/init/detect.rs`. Эта небольшая Rust-библиотека принимает
байты/значения; обход каталогов, no-follow чтение, проверка путей и ограничения
остаются в хосте. Команды build/test являются предложениями и не исполняются.

```sh
cargo test --locked --manifest-path crates/discovery/Cargo.toml
```

Ядро вызывает самостоятельный процесс `lekalo-rust-discovery` по `lekalo/runtime/v1`; копий и Cargo-зависимости на этот пакет в ядре нет. Формат существующего adoption receipt сохраняется.

Полноценного service runtime пока нет: его реализация остаётся в issue #1.

## Установка discovery-сервиса

```sh
cargo install --locked --path crates/discovery --root /your/tools
```

Добавьте `/your/tools/bin` в PATH, укажите `services.discovery/rust: {command: [lekalo-rust-discovery]}` в `lekalo.yaml` и выполните `lekalo runtime lock`. Операции: `describe`, `discovery.describe`, `discovery.inspect`; входные файлы передаются как данные, процесс не читает проект. [Контракт сервиса](https://github.com/ichinya/lekalo/blob/feat/external-services-241/docs/external-services.md). Сборка сервиса использует `debug=0`, `incremental=false`.

## Нативная проверка контрактов

`lekalo-rust-check` получает Rust-исходники и правила через stdin по
`lekalo/runtime/v1`. Использует нативную грамматику Tree-sitter Rust 0.24.0.
Поддерживает `describe` и `check`, факты `source.declarations/v1` и
`source.matches/v1`, а также bindings `lekalo-rust.queries/v1` из существующих
пакетов правил.

```sh
cargo install --locked --path crates/runtime
```

Укажите команду у компонента в `lekalo.yaml`:

```yaml
schema: lekalo/runtime-config/v1
components:
  app:
    path: src
    runtime:
      command:
        - lekalo-rust-check
    rules:
      - path:rules
```

Добавьте бинарник в PATH, выполните `lekalo runtime lock`, затем
`lekalo runtime check`.

## Portable runtime

Select `gh/ichinya/lekalo-rust` as a component runtime and run `lekalo runtime lock`. Node runs the pinned WebAssembly grammar. The `source.declarations/v1` result lists syntactic declarations with path, kind and name. Rule-specific `source.matches/v1` bindings under `lekalo-rust.queries/v1` map each check ID to a Tree-sitter query. Syntax errors produce incomplete evidence. Grammar from tree-sitter-rust 0.24.0 (see `runtime/LICENSE.grammar`).
