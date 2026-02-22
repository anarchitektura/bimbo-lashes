-- Services offered by the master
CREATE TABLE IF NOT EXISTS services (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    price INTEGER NOT NULL,           -- price in smallest currency unit (e.g. rubles)
    duration_min INTEGER NOT NULL,     -- duration in minutes
    is_active BOOLEAN NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Available time slots set by the master (floating schedule)
CREATE TABLE IF NOT EXISTS available_slots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL,                -- YYYY-MM-DD
    start_time TEXT NOT NULL,          -- HH:MM
    end_time TEXT NOT NULL,            -- HH:MM
    is_booked BOOLEAN NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_slots_date ON available_slots(date);
CREATE INDEX idx_slots_available ON available_slots(date, is_booked);

-- Client bookings
CREATE TABLE IF NOT EXISTS bookings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id INTEGER NOT NULL REFERENCES services(id),
    slot_id INTEGER NOT NULL REFERENCES available_slots(id),
    client_tg_id INTEGER NOT NULL,
    client_username TEXT,
    client_first_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'confirmed',  -- confirmed | cancelled
    reminder_sent BOOLEAN NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    cancelled_at TEXT
);

CREATE INDEX idx_bookings_client ON bookings(client_tg_id);
CREATE INDEX idx_bookings_date ON bookings(slot_id);
CREATE INDEX idx_bookings_status ON bookings(status);

-- App settings (admin TG ID, etc.)
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Seed default services
INSERT INTO services (name, description, price, duration_min, sort_order) VALUES
    ('Классика', 'Классическое наращивание 1:1', 2500, 120, 1),
    ('2D', 'Объёмное наращивание 2D', 3000, 150, 2),
    ('3D', 'Объёмное наращивание 3D', 3500, 150, 3),
    ('Мега-объём', 'Голливудское наращивание 4D-6D', 4500, 180, 4),
    ('Коррекция', 'Коррекция наращивания', 2000, 90, 5),
    ('Снятие', 'Снятие наращенных ресниц', 500, 30, 6),
    ('Ламинирование', 'Ламинирование и ботокс ресниц', 2500, 60, 7);
