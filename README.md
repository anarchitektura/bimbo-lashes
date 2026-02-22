# Bimbo Lashes — Telegram Mini App

Приложение для онлайн-записи к мастеру по ресницам.

## Возможности

**Для клиентов:**
- Просмотр услуг с ценами и длительностью
- Запись в 3 шага: услуга → дата → время
- Управление своими записями (просмотр, отмена)
- Напоминания за 24 часа до визита

**Для мастера:**
- Админ-панель в Mini App (расписание, услуги, записи)
- Плавающий график — сама выставляет слоты
- Уведомления в бота о новых записях и отменах
- Команды `/today` и `/tomorrow` для быстрого просмотра
- Отмена записей через inline-кнопки в боте

## Стек

| Компонент | Технологии |
|-----------|-----------|
| Frontend  | Solid.js, Vite, Tailwind CSS v4, @twa-dev/sdk |
| Backend   | Rust, Axum, SQLite (sqlx) |
| Bot       | Rust, teloxide |
| Infra     | Docker, nginx |

## Быстрый старт

### 1. Подготовка

```bash
# Создай бота через @BotFather в Telegram
# Получи свой Telegram ID через @userinfobot

cp .env.example .env
# Заполни BOT_TOKEN, ADMIN_TG_ID, WEBAPP_URL
```

### 2. Запуск через Docker

```bash
docker compose up --build
```

### 3. Локальная разработка

```bash
# Backend
cargo run --package bimbo-lashes-server

# Bot
cargo run --package bimbo-lashes-bot

# Frontend
cd apps/web && npm install && npm run dev
```

Frontend dev-сервер (Vite) проксирует `/api/*` на `localhost:3000`.

### 4. Настройка Mini App в Telegram

1. Открой @BotFather → `/mybots` → выбери бота
2. `Bot Settings` → `Menu Button` → укажи HTTPS URL фронтенда
3. Или: `Bot Settings` → `Menu Button` → `Configure Mini App`

> Telegram требует HTTPS. Для разработки используй [ngrok](https://ngrok.com/) или Cloudflare Tunnel.

## Переменные окружения

| Переменная | Описание |
|-----------|---------|
| `BOT_TOKEN` | Токен бота из @BotFather |
| `ADMIN_TG_ID` | Telegram ID мастера (число) |
| `DATABASE_URL` | Путь к SQLite (`sqlite:bimbo.db?mode=rwc`) |
| `HOST` | Хост сервера (default: `0.0.0.0`) |
| `PORT` | Порт сервера (default: `3000`) |
| `WEBAPP_URL` | Публичный URL фронтенда (HTTPS) |
| `VITE_API_URL` | URL API для фронтенда (при разработке пустой — Vite проксирует) |
| `VITE_ADMIN_TG_ID` | ID мастера для показа админки в TMA |

## Структура

```
bimbo-lashes/
├── apps/
│   ├── web/                # Solid.js frontend
│   │   ├── src/
│   │   │   ├── pages/      # HomePage, BookingPage, MyBookingsPage, Admin*
│   │   │   ├── components/ # Loader
│   │   │   └── lib/        # api.ts, router.ts, utils.ts
│   │   └── index.html
│   ├── server/             # Rust API
│   │   ├── src/
│   │   │   ├── handlers/   # client.rs, admin.rs
│   │   │   ├── auth.rs     # Telegram initData validation
│   │   │   ├── db.rs       # Migrations
│   │   │   ├── models.rs   # Types
│   │   │   └── main.rs
│   │   └── migrations/     # SQL schema
│   └── bot/                # Telegram bot
│       └── src/main.rs     # Commands, callbacks, reminders
├── docker-compose.yml
├── .env.example
└── CLAUDE.md
```

## API

### Client
| Method | Path | Описание |
|--------|------|---------|
| GET | `/api/services` | Список активных услуг |
| GET | `/api/slots/dates` | Даты с доступными слотами |
| GET | `/api/slots?date=YYYY-MM-DD` | Свободные слоты на дату |
| POST | `/api/bookings` | Создать запись |
| GET | `/api/bookings/my` | Мои записи |
| DELETE | `/api/bookings/:id` | Отменить запись |

### Admin
| Method | Path | Описание |
|--------|------|---------|
| GET | `/api/admin/services` | Все услуги (вкл. неактивные) |
| POST | `/api/admin/services` | Создать услугу |
| PUT | `/api/admin/services/:id` | Обновить услугу |
| GET | `/api/admin/slots?date=` | Все слоты на дату |
| POST | `/api/admin/slots` | Создать слоты |
| DELETE | `/api/admin/slots/:id` | Удалить слот |
| GET | `/api/admin/bookings` | Список записей |
| POST | `/api/admin/bookings/:id/cancel` | Отменить запись (админ) |

Все эндпоинты требуют `Authorization: tma <initData>` в заголовке.
