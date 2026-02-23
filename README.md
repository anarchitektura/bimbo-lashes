# Bimbo Lashes — Telegram Mini App

Приложение для онлайн-записи к мастеру по ресницам с предоплатой через ЮКассу.

## Возможности

**Для клиентов:**
- Каталог услуг с ценами и длительностью
- Запись в 4 шага: услуга → дата → время → оплата
- Предоплата 500 ₽ (карты + СБП через ЮКассу)
- Умный подбор времени: при записи за ≤3 дня показывает только слоты рядом с существующими (минимизация фрагментации)
- Автоматический возврат при отмене за 24+ часов
- Напоминания за день до визита

**Для мастера:**
- Админ-панель в Mini App (расписание, услуги, слоты)
- Плавающий график — сама выставляет 1-часовые слоты через «Открыть день»
- Уведомления в бота о новых записях, отменах и оплатах
- Команды `/today`, `/tomorrow`, `/schedule YYYY-MM-DD` для просмотра расписания
- Отмена записей через inline-кнопки в боте (всегда с возвратом)

## Стек

| Компонент | Технологии |
|-----------|-----------|
| Frontend  | Solid.js, Vite, Tailwind CSS v4, @twa-dev/sdk |
| Backend   | Rust, Axum 0.8, SQLite (sqlx 0.8) |
| Bot       | Rust, teloxide 0.13 |
| Payments  | ЮКасса (REST API, вебхуки) |
| Infra     | Docker, nginx, Caddy (HTTPS), GitHub Actions |

## Быстрый старт

### 1. Подготовка

```bash
# Создай бота через @BotFather в Telegram
# Получи свой Telegram ID через @userinfobot
# Зарегистрируй магазин на yookassa.ru

cp .env.example .env
# Заполни BOT_TOKEN, ADMIN_TG_ID, WEBAPP_URL, YOOKASSA_*
```

### 2. Запуск через Docker

```bash
docker compose up --build
```

Три контейнера: `server` (:3000), `bot`, `web` (:8080 → nginx).
Health check на `/api/health`, auto-restart при падении.

### 3. Локальная разработка

```bash
# Backend
cargo run --package bimbo-lashes-server

# Bot
cargo run --package bimbo-lashes-bot

# Frontend
cd apps/web && npm install && npm run dev
```

Vite dev-сервер проксирует `/api/*` на `localhost:3000`.

### 4. Настройка Mini App в Telegram

1. @BotFather → `/mybots` → выбери бота
2. `Bot Settings` → `Menu Button` → укажи HTTPS URL фронтенда
3. Или: `Bot Settings` → `Configure Mini App`

> Telegram требует HTTPS. Для разработки — [ngrok](https://ngrok.com/) или Cloudflare Tunnel.

## Тесты

```bash
# Все unit-тесты (69 тестов, ~0.01s)
cargo test --workspace

# Только сервер (61 тест)
cargo test --package bimbo-lashes-server

# Только бот (8 тестов)
cargo test --package bimbo-lashes-bot
```

**Покрытие:**

| Модуль | Что тестируется | Тестов |
|--------|----------------|--------|
| `auth.rs` | HMAC-SHA256 валидация initData, проверка срока auth_date (24ч), is_admin | 17 |
| `client.rs` | Расчёт слотов, дни между датами, сложение времени, поиск последовательных слотов, bookable blocks (free/tight mode), adjacency | 44 |
| `bot/main.rs` | Форматирование даты (DD.MM) | 8 |

Тесты автоматически запускаются:
- **CI** — `cargo test --workspace` на каждый PR (`ci.yml`)
- **Docker build** — `cargo test --release` гейтит сборку образа (сломанный тест = сборка не пройдёт)

Тесты НЕ попадают в production Docker-образ (multi-stage build).

## CI/CD

| Workflow | Триггер | Что делает |
|----------|---------|-----------|
| `ci.yml` | PR → main | `cargo check` + `cargo test` + `cargo clippy` + `npm run build` + `docker compose build` |
| `deploy.yml` | Push → main | Backup → tag previous images → `docker compose build` (с тестами) → deploy → health check → rollback on fail |
| `backup.yml` | Cron 03:00 | Бэкап SQLite на VPS |

Деплой автоматический: push в main → GitHub Actions → SSH на VPS → docker compose up.
При провале health check — автоматический rollback на предыдущие образы.

## Переменные окружения

| Переменная | Описание | Обязательна |
|-----------|---------|-------------|
| `BOT_TOKEN` | Токен бота из @BotFather | ✅ |
| `ADMIN_TG_ID` | Telegram ID мастера | ✅ |
| `DATABASE_URL` | Путь к SQLite (`sqlite:bimbo.db?mode=rwc`) | ✅ |
| `YOOKASSA_SHOP_ID` | Shop ID из ЮКассы | ✅ |
| `YOOKASSA_SECRET_KEY` | Секретный ключ ЮКассы | ✅ |
| `WEBAPP_URL` | Публичный URL Mini App (HTTPS) | ✅ |
| `HOST` | Хост сервера | `0.0.0.0` |
| `PORT` | Порт сервера | `3000` |
| `VITE_API_URL` | URL API для фронтенда | пустой при dev |
| `VITE_ADMIN_TG_ID` | ID мастера (фронт показывает админку) | ✅ |

## Структура проекта

```
bimbo-lashes/
├── apps/
│   ├── web/                  # Solid.js frontend
│   │   ├── src/
│   │   │   ├── pages/        # HomePage, BookingPage, MyBookingsPage, Admin*
│   │   │   ├── components/   # Calendar, Loader
│   │   │   └── lib/          # api.ts, router.ts, utils.ts
│   │   └── index.html
│   ├── server/               # Rust API (Axum)
│   │   ├── src/
│   │   │   ├── handlers/     # client.rs, admin.rs, payment.rs, health.rs
│   │   │   ├── auth.rs       # Telegram initData HMAC-SHA256 validation
│   │   │   ├── db.rs         # Migrations (001–007)
│   │   │   ├── models.rs     # Types & DTOs
│   │   │   └── main.rs       # Router, CORS, background tasks
│   │   └── Dockerfile
│   └── bot/                  # Telegram bot (teloxide)
│       ├── src/main.rs       # Commands, callbacks, reminders
│       └── Dockerfile
├── tests/
│   ├── integration/          # Vitest API tests
│   └── e2e/                  # Playwright E2E tests
├── scripts/                  # backup.sh, health-check.sh, rollback.sh, notify.sh
├── .github/workflows/        # ci.yml, deploy.yml, backup.yml
├── docker-compose.yml
└── .env.example
```

## API

### Клиентские эндпоинты

| Method | Path | Описание |
|--------|------|---------|
| GET | `/api/services` | Активные услуги (main, без аддонов) |
| GET | `/api/addon-info` | Информация об аддоне (нижние ресницы) |
| GET | `/api/calendar?year=&month=&service_id=` | Календарь с доступностью |
| GET | `/api/available-dates?service_id=` | Даты с достаточным числом свободных слотов |
| GET | `/api/available-times?date=&service_id=` | Доступное время (free/tight mode) |
| POST | `/api/bookings` | Создать запись + платёж ЮКассы |
| GET | `/api/bookings/my` | Мои записи (confirmed + pending_payment) |
| GET | `/api/bookings/:id/status` | Статус записи (polling оплаты) |
| DELETE | `/api/bookings/:id` | Отменить запись (с логикой возврата) |

### Админские эндпоинты

| Method | Path | Описание |
|--------|------|---------|
| GET | `/api/admin/services` | Все услуги (вкл. неактивные) |
| POST | `/api/admin/services` | Создать услугу |
| PUT | `/api/admin/services/:id` | Обновить услугу (COALESCE) |
| GET | `/api/admin/slots?date=` | Все слоты на дату |
| POST | `/api/admin/slots` | Создать слоты |
| DELETE | `/api/admin/slots/:id` | Удалить слот |
| POST | `/api/admin/openday` | Открыть день (слоты 12–20) |
| GET | `/api/admin/bookings` | Список записей (фильтры date/from/to) |
| POST | `/api/admin/bookings/:id/cancel` | Отменить запись (всегда с возвратом) |

### Служебные

| Method | Path | Описание |
|--------|------|---------|
| GET | `/api/health` | Health check (статус, uptime, DB) |
| POST | `/api/payments/webhook` | Вебхук ЮКассы (IP whitelist) |

Все эндпоинты (кроме health и webhook) требуют `Authorization: tma <initData>`.

## Платёжный поток

```
Клиент выбирает время → POST /api/bookings
  → Букинг создаётся (status=pending_payment, слоты заняты)
  → ЮКасса создаёт платёж → клиент перенаправлен на оплату
  → Фронтенд поллит GET /bookings/:id/status каждые 3с

ЮКасса вебхук → POST /api/payments/webhook
  → payment.succeeded → status=confirmed, payment_status=paid → уведомление мастеру
  → payment.canceled → status=expired → слоты освобождены

Фоновая задача (каждые 5 мин):
  → Букинги pending_payment старше 15 мин → expired → слоты освобождены
```

**Возвраты:**
- Клиент отменяет за >24ч → автоматический возврат 500 ₽
- Клиент отменяет за ≤24ч → предоплата не возвращается
- Мастер отменяет → всегда возврат
