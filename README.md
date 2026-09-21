# YETI³ Cleaner

> **Deep macOS Cleanup**
> **CODE. CLEAN. OPTIMIZE. REPEAT.**

YETI³ Cleaner — нативная утилита глубокой очистки macOS, написанная на **Rust**.

Она очищает системные и пользовательские кэши, временные файлы, логи, артефакты разработки, контейнерные и AI/ML-кэши, сохраняя историю каждой очистки локально.

**Версия:** 0.3.0
**Платформа:** macOS 14+
**Архитектура:** Apple Silicon / arm64

---

## Download

### YETI³ Cleaner 0.3.0

**Рекомендуемый вариант:**

`downloads/Yeti3-Cleaner-0.3.0-arm64.dmg`

Альтернативный архив приложения:

`downloads/Yeti3-Cleaner-0.3.0-arm64.zip`

Контрольные суммы:

`downloads/SHA256SUMS.txt`

---

## Установка

1. Скачайте `Yeti3-Cleaner-0.3.0-arm64.dmg`.
2. Откройте DMG.
3. Перетащите **YETI³ Cleaner** в **Applications**.
4. Запустите приложение.

YETI³ Cleaner работает как приложение строки меню macOS.

---

## Возможности

- глубокая очистка macOS;
- очистка временных файлов и логов;
- очистка пользовательских кэшей;
- Xcode и Swift development caches;
- Gradle / Android caches;
- Node.js / npm / pnpm / Yarn caches;
- Python caches;
- Rust / Cargo caches;
- Go caches;
- Homebrew caches;
- Docker и Podman cleanup;
- AI / ML caches;
- локальная история очисток;
- статистика по запускам и категориям;
- объём свободного места **до / после**;
- количество удалённых файлов и каталогов;
- продолжительность очистки;
- отчёт об ошибках и пропущенных объектах.

---

## Glass + Ice UI

Интерфейс YETI³ Cleaner выполнен в фирменном стиле **Glass + Ice**.

В приложение входят:

- анимированный YETI³ launcher;
- меню macOS menu bar;
- стилизованные настройки;
- окно «О программе»;
- premium result dialog;
- графическая статистика очисток;
- история результатов.

В строке меню используется компактный индикатор:

- `Y³` — ожидание;
- `Y³ · / • / ●` — очистка;
- `Y³ Ⅱ` — пауза;
- `Y³ ✓` — завершено;
- `Y³ !` — ошибка.

**ПКМ по `Y³` открывает статистику.**

---

## История и статистика

YETI³ Cleaner хранит историю локально в SQLite:

```text
~/Library/Application Support/Yeti3-Cleaner/history.sqlite3
```


Эта версия для свободного распространения.

Разработчик: @bonumursi
