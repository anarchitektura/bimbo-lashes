# Architecture Design Doc: Bimbo Lashes

## Overview

Система состоит из трёх компонентов, упакованных в Rust workspace + отдельный
Node.js проект для фронтенда.

```mermaid
graph LR
    subgraph Telegram
        U[Клиент] -->|открывает| BOT[Bot]
        BOT -->|WebApp button| TMA[Mini App]
        BOT -->|уведомления| M[Мастер]
    end

    subgraph Backend
        TMA -->|HTTPS /api/*| SRV[Axum Server :3000]
        BOT2[Bot Process] -->|SQLite| DB[(bimbo.db)]
        SRV -->|SQLite| DB
        SRV -->|Bot API| TG[Telegram API]
    end

    TMA -->|Authorization: tma initData| SRV
```

## Компоненты

### 1. Frontend (apps/web)

```mermaid
graph TD
    index.tsx --> App.tsx
    App.tsx --> Router[lib/router.ts]
    Router --> HP[HomePage]
    Router --> BP[BookingPage]
    Router --> MBP[MyBookingsPage]
    Router --> AP[AdminPage]
    Router --> ASP[AdminSchedulePage]
    Router --> ASRV[AdminServicesPage]

    HP --> API[lib/api.ts]
    BP --> API
    MBP --> API
    AP --> API
    ASP --> API
    ASRV --> API
    API -->|fetch + tma auth| Server
```

**Решения:**
- **Solid.js** вместо React — меньше бандл, быстрее рендер, реактивность без Virtual DOM
- **Tailwind v4** — утилитарные стили, тёмная тема через CSS-переменные Telegram
- **Нет роутера-библиотеки** — простой сигнал `route` достаточен для 6 экранов
- **@twa-dev/sdk** — типизированный доступ к WebApp API

### 2. Backend (apps/server)

```mermaid
graph TD
    REQ[HTTP Request] --> AUTH{Auth Middleware}
    AUTH -->|valid initData| HANDLER[Handler]
    AUTH -->|invalid| R401[401 Unauthorized]

    HANDLER --> CLIENT[Client Handlers]
    HANDLER --> ADMIN[Admin Handlers]

    CLIENT --> DB[(SQLite)]
    ADMIN --> DB
    CLIENT -->|notify| TGAPI[Telegram Bot API]
    ADMIN -->|notify client| TGAPI

    subgraph Admin Guard
        ADMIN --> ACHECK{user.id == admin_tg_id?}
        ACHECK -->|no| R403[403 Forbidden]
    end
```

**Решения:**
- **Axum 0.8** — самый производительный Rust web-framework, тайп-сейф
- **SQLite** через sqlx — один мастер, < 100 записей/день, не нужен PostgreSQL
- **WAL mode** — для конкурентного доступа из server + bot
- **HMAC-SHA256 валидация** initData на каждом запросе
- **reqwest** для отправки уведомлений через Bot API (а не через teloxide в server)

### 3. Bot (apps/bot)

```mermaid
graph TD
    TG[Telegram Updates] --> DISP[Dispatcher]
    DISP --> CMD[Command Handler]
    DISP --> CB[Callback Handler]

    CMD --> START[/start — WebApp button]
    CMD --> MYBK[/mybookings — список записей]
    CMD --> TODAY[/today — записи на сегодня]
    CMD --> TOMORROW[/tomorrow — записи на завтра]
    CMD --> HELP[/help]

    CB --> CANCEL[cancel:ID — клиент отменяет]
    CB --> ACANCEL[admin_cancel:ID — мастер отменяет]

    REMIND[Reminder Task] -->|каждый час| DB[(SQLite)]
    REMIND -->|sendMessage| TG
```

**Решения:**
- **teloxide 0.13** — стабильный, хорошо документированный Telegram bot framework
- **dptree** — функциональный диспатчер, разделение command/callback веток
- **Фоновый таск** с `tokio::time::interval` для напоминаний (не cron)

## Data Model

```mermaid
erDiagram
    services ||--o{ bookings : "has"
    available_slots ||--o{ bookings : "has"

    services {
        int id PK
        text name
        text description
        int price
        int duration_min
        bool is_active
        int sort_order
    }

    available_slots {
        int id PK
        text date
        text start_time
        text end_time
        bool is_booked
    }

    bookings {
        int id PK
        int service_id FK
        int slot_id FK
        int client_tg_id
        text client_username
        text client_first_name
        text status
        bool reminder_sent
        text created_at
        text cancelled_at
    }
```

## Auth Flow

```mermaid
sequenceDiagram
    participant C as Client (TMA)
    participant S as Server
    participant T as Telegram

    C->>C: WebApp.initData (from Telegram WebView)
    C->>S: GET /api/services<br/>Authorization: tma {initData}
    S->>S: Parse initData params
    S->>S: Extract hash
    S->>S: Build data-check-string (sorted, excl. hash)
    S->>S: secret = HMAC-SHA256("WebAppData", BOT_TOKEN)
    S->>S: computed = HMAC-SHA256(secret, data-check-string)
    alt computed == hash
        S->>S: Parse user JSON from initData
        S-->>C: 200 OK + data
    else
        S-->>C: 401 Unauthorized
    end
```

## Notification Flow

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server
    participant T as Telegram API
    participant M as Master (chat)

    C->>S: POST /api/bookings
    S->>S: Create booking, mark slot booked
    S->>T: sendMessage(admin_tg_id, "Новая запись!")
    T->>M: 📋 Новая запись!<br/>👤 @username<br/>💅 2D<br/>📅 26 фев в 14:00
    S-->>C: 200 OK (booking details)
```

## Deployment

```mermaid
graph LR
    subgraph Docker Compose
        WEB[nginx :8080] -->|proxy /api| SRV[server :3000]
        SRV --> DB[(SQLite volume)]
        BOTC[bot] --> DB
    end

    INET[Internet] -->|HTTPS| WEB
    BOTC -->|polling| TGAPI[Telegram API]
    SRV -->|sendMessage| TGAPI
```

## ADR: Почему SQLite, а не PostgreSQL

**Контекст:** один мастер, < 100 записей в день, < 1000 записей в месяц.

**Решение:** SQLite в WAL mode.

**Аргументы за:**
- Zero-config: не нужен отдельный сервер БД
- Один файл — простой бэкап (cp bimbo.db bimbo.db.bak)
- Latency < 1ms для всех запросов
- Docker volume вместо отдельного контейнера

**Риски:**
- Конкурентная запись из server + bot → WAL mode решает
- Масштабирование на несколько мастеров → миграция на PostgreSQL (v2.0)
