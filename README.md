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

Ядро использует проверяемую копию в `packages/` с точным commit и SHA-256.
Это переходный способ поставки host-side discovery, а не новый процессный
runtime или resolver пакетов. Формат существующего adoption receipt сохраняется.

Полноценного service runtime пока нет: его реализация остаётся в issue #1.
