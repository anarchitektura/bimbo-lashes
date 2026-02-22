use sqlx::sqlite::SqlitePoolOptions;
use teloxide::{
    prelude::*,
    types::{
        ChatId, InlineKeyboardButton, InlineKeyboardMarkup, ParseMode, WebAppInfo,
    },
    utils::command::BotCommands,
};
use tokio::time::{interval, Duration};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    #[command(description = "Открыть запись")]
    Start,
    #[command(description = "Мои записи")]
    MyBookings,
    #[command(description = "Записи на сегодня (для мастера)")]
    Today,
    #[command(description = "Записи на завтра (для мастера)")]
    Tomorrow,
    #[command(description = "Помощь")]
    Help,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct BookingInfo {
    id: i64,
    service_name: String,
    service_price: i64,
    date: String,
    start_time: String,
    end_time: String,
    client_tg_id: i64,
    client_username: Option<String>,
    client_first_name: String,
}

#[derive(Clone)]
struct BotState {
    pool: sqlx::SqlitePool,
    webapp_url: String,
    admin_tg_id: i64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse()?),
        )
        .init();

    let bot_token = std::env::var("BOT_TOKEN").expect("BOT_TOKEN must be set");
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:bimbo.db?mode=rwc".into());
    let webapp_url = std::env::var("WEBAPP_URL").expect("WEBAPP_URL must be set");
    let admin_tg_id: i64 = std::env::var("ADMIN_TG_ID")
        .expect("ADMIN_TG_ID must be set")
        .parse()
        .expect("ADMIN_TG_ID must be a number");

    let pool = SqlitePoolOptions::new()
        .max_connections(3)
        .connect(&database_url)
        .await?;

    let bot = Bot::new(&bot_token);

    tracing::info!("💅 Bimbo Lashes bot starting...");

    // Spawn reminder task
    let reminder_bot = bot.clone();
    let reminder_pool = pool.clone();
    tokio::spawn(async move {
        send_reminders(reminder_bot, reminder_pool).await;
    });

    let state = BotState {
        pool,
        webapp_url,
        admin_tg_id,
    };

    // Handle commands + callback queries (inline buttons)
    let cmd_handler = Update::filter_message()
        .filter_command::<Command>()
        .endpoint({
            let state = state.clone();
            move |bot: Bot, msg: Message, cmd: Command| {
                let state = state.clone();
                async move {
                    handle_command(bot, msg, cmd, &state).await?;
                    Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
                }
            }
        });

    let callback_handler = Update::filter_callback_query().endpoint({
        let state = state.clone();
        move |bot: Bot, q: CallbackQuery| {
            let state = state.clone();
            async move {
                handle_callback(bot, q, &state).await?;
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            }
        }
    });

    let handler = dptree::entry()
        .branch(cmd_handler)
        .branch(callback_handler);

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

// ── Command handlers ──

async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    state: &BotState,
) -> anyhow::Result<()> {
    match cmd {
        Command::Start => {
            let keyboard = InlineKeyboardMarkup::new(vec![vec![
                InlineKeyboardButton::web_app(
                    "💅 Записаться",
                    WebAppInfo {
                        url: state.webapp_url.parse().expect("Invalid WEBAPP_URL"),
                    },
                ),
            ]]);

            bot.send_message(
                msg.chat.id,
                "✨ <b>Bimbo Lashes</b> ✨\n\n\
                 Привет! 👋\n\
                 Я помогу тебе записаться на реснички.\n\n\
                 Нажми кнопку ниже, чтобы выбрать услугу и удобное время 💕",
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(keyboard)
            .await?;
        }

        Command::MyBookings => {
            let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);

            let bookings = sqlx::query_as::<_, BookingInfo>(
                "SELECT b.id, s.name as service_name, s.price as service_price,
                        sl.date, sl.start_time, sl.end_time,
                        b.client_tg_id, b.client_username, b.client_first_name
                 FROM bookings b
                 JOIN services s ON s.id = b.service_id
                 JOIN available_slots sl ON sl.id = b.slot_id
                 WHERE b.client_tg_id = ? AND b.status = 'confirmed'
                 AND sl.date >= date('now')
                 ORDER BY sl.date ASC, sl.start_time ASC",
            )
            .bind(user_id)
            .fetch_all(&state.pool)
            .await?;

            if bookings.is_empty() {
                let keyboard = InlineKeyboardMarkup::new(vec![vec![
                    InlineKeyboardButton::web_app(
                        "💅 Записаться",
                        WebAppInfo {
                            url: state.webapp_url.parse().expect("Invalid WEBAPP_URL"),
                        },
                    ),
                ]]);

                bot.send_message(msg.chat.id, "У тебя пока нет активных записей 🤷‍♀️")
                    .reply_markup(keyboard)
                    .await?;
            } else {
                let mut text = "📋 <b>Твои записи:</b>\n\n".to_string();
                for b in &bookings {
                    text.push_str(&format!(
                        "💅 <b>{}</b>\n📅 {} · {} — {}\n💰 {} ₽\n\n",
                        b.service_name,
                        format_date_ru(&b.date),
                        &b.start_time[..5],
                        &b.end_time[..5],
                        b.service_price,
                    ));
                }

                // Add cancel buttons for each booking
                let buttons: Vec<Vec<InlineKeyboardButton>> = bookings
                    .iter()
                    .map(|b| {
                        vec![InlineKeyboardButton::callback(
                            format!("❌ Отменить {} ({})", b.service_name, &b.date),
                            format!("cancel:{}", b.id),
                        )]
                    })
                    .collect();

                let keyboard = InlineKeyboardMarkup::new(buttons);
                bot.send_message(msg.chat.id, text)
                    .parse_mode(ParseMode::Html)
                    .reply_markup(keyboard)
                    .await?;
            }
        }

        Command::Today => {
            let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
            if user_id != state.admin_tg_id {
                bot.send_message(msg.chat.id, "⛔ Только для мастера").await?;
                return Ok(());
            }

            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            send_day_bookings(&bot, msg.chat.id, &state.pool, &today, "Сегодня").await?;
        }

        Command::Tomorrow => {
            let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
            if user_id != state.admin_tg_id {
                bot.send_message(msg.chat.id, "⛔ Только для мастера").await?;
                return Ok(());
            }

            let tomorrow = (chrono::Local::now() + chrono::TimeDelta::days(1))
                .format("%Y-%m-%d")
                .to_string();
            send_day_bookings(&bot, msg.chat.id, &state.pool, &tomorrow, "Завтра").await?;
        }

        Command::Help => {
            let is_admin = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0)
                == state.admin_tg_id;

            let mut text = "💕 <b>Bimbo Lashes — бот для записи</b>\n\n\
                 /start — открыть приложение для записи\n\
                 /mybookings — посмотреть мои записи\n\
                 /help — помощь"
                .to_string();

            if is_admin {
                text.push_str(
                    "\n\n<b>🔧 Команды мастера:</b>\n\
                     /today — записи на сегодня\n\
                     /tomorrow — записи на завтра",
                );
            }

            bot.send_message(msg.chat.id, text)
                .parse_mode(ParseMode::Html)
                .await?;
        }
    }

    Ok(())
}

// ── Callback query handler (inline button clicks) ──

async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    state: &BotState,
) -> anyhow::Result<()> {
    let data = q.data.as_deref().unwrap_or("");
    let chat_id = q.message.as_ref().map(|m| m.chat().id);
    let user_id = q.from.id.0 as i64;

    if let Some(booking_id_str) = data.strip_prefix("cancel:") {
        let booking_id: i64 = booking_id_str.parse().unwrap_or(0);

        // Verify booking belongs to this user
        let booking = sqlx::query_as::<_, BookingInfo>(
            "SELECT b.id, s.name as service_name, s.price as service_price,
                    sl.date, sl.start_time, sl.end_time,
                    b.client_tg_id, b.client_username, b.client_first_name
             FROM bookings b
             JOIN services s ON s.id = b.service_id
             JOIN available_slots sl ON sl.id = b.slot_id
             WHERE b.id = ? AND b.client_tg_id = ? AND b.status = 'confirmed'",
        )
        .bind(booking_id)
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?;

        if let Some(b) = booking {
            // Cancel it
            sqlx::query(
                "UPDATE bookings SET status = 'cancelled', cancelled_at = datetime('now') WHERE id = ?",
            )
            .bind(booking_id)
            .execute(&state.pool)
            .await?;

            // Free the slot
            let slot_id: Option<i64> = sqlx::query_scalar(
                "SELECT slot_id FROM bookings WHERE id = ?",
            )
            .bind(booking_id)
            .fetch_optional(&state.pool)
            .await?;

            if let Some(sid) = slot_id {
                sqlx::query("UPDATE available_slots SET is_booked = 0 WHERE id = ?")
                    .bind(sid)
                    .execute(&state.pool)
                    .await?;
            }

            bot.answer_callback_query(&q.id).text("✅ Запись отменена").await?;

            if let Some(cid) = chat_id {
                bot.send_message(
                    cid,
                    format!(
                        "✅ Запись отменена:\n💅 {}\n📅 {} · {}",
                        b.service_name,
                        format_date_ru(&b.date),
                        &b.start_time[..5],
                    ),
                )
                .await?;
            }

            // Notify admin
            let mention = b
                .client_username
                .as_ref()
                .map(|u| format!("@{}", u))
                .unwrap_or_else(|| b.client_first_name.clone());

            let admin_msg = format!(
                "❌ Отмена записи\n\n👤 {}\n💅 {}\n📅 {} в {}",
                mention,
                b.service_name,
                format_date_ru(&b.date),
                &b.start_time[..5],
            );

            bot.send_message(ChatId(state.admin_tg_id), admin_msg).await?;
        } else {
            bot.answer_callback_query(&q.id)
                .text("Запись не найдена или уже отменена")
                .await?;
        }
    } else if let Some(booking_id_str) = data.strip_prefix("admin_cancel:") {
        // Admin cancels a booking
        if user_id != state.admin_tg_id {
            bot.answer_callback_query(&q.id).text("⛔").await?;
            return Ok(());
        }

        let booking_id: i64 = booking_id_str.parse().unwrap_or(0);

        let booking = sqlx::query_as::<_, BookingInfo>(
            "SELECT b.id, s.name as service_name, s.price as service_price,
                    sl.date, sl.start_time, sl.end_time,
                    b.client_tg_id, b.client_username, b.client_first_name
             FROM bookings b
             JOIN services s ON s.id = b.service_id
             JOIN available_slots sl ON sl.id = b.slot_id
             WHERE b.id = ? AND b.status = 'confirmed'",
        )
        .bind(booking_id)
        .fetch_optional(&state.pool)
        .await?;

        if let Some(b) = booking {
            sqlx::query(
                "UPDATE bookings SET status = 'cancelled', cancelled_at = datetime('now') WHERE id = ?",
            )
            .bind(booking_id)
            .execute(&state.pool)
            .await?;

            let slot_id: Option<i64> = sqlx::query_scalar(
                "SELECT slot_id FROM bookings WHERE id = ?",
            )
            .bind(booking_id)
            .fetch_optional(&state.pool)
            .await?;

            if let Some(sid) = slot_id {
                sqlx::query("UPDATE available_slots SET is_booked = 0 WHERE id = ?")
                    .bind(sid)
                    .execute(&state.pool)
                    .await?;
            }

            bot.answer_callback_query(&q.id)
                .text("✅ Запись отменена")
                .await?;

            // Notify the client
            bot.send_message(
                ChatId(b.client_tg_id),
                format!(
                    "😔 Твоя запись на {} в {} была отменена мастером.\n\n\
                     Выбери другое время 💕",
                    format_date_ru(&b.date),
                    &b.start_time[..5],
                ),
            )
            .await
            .ok(); // client may have blocked the bot

            if let Some(cid) = chat_id {
                bot.send_message(
                    cid,
                    format!("✅ Запись {} отменена", b.client_first_name),
                )
                .await?;
            }
        } else {
            bot.answer_callback_query(&q.id)
                .text("Запись не найдена")
                .await?;
        }
    }

    Ok(())
}

// ── Admin helpers ──

async fn send_day_bookings(
    bot: &Bot,
    chat_id: ChatId,
    pool: &sqlx::SqlitePool,
    date: &str,
    label: &str,
) -> anyhow::Result<()> {
    let bookings = sqlx::query_as::<_, BookingInfo>(
        "SELECT b.id, s.name as service_name, s.price as service_price,
                sl.date, sl.start_time, sl.end_time,
                b.client_tg_id, b.client_username, b.client_first_name
         FROM bookings b
         JOIN services s ON s.id = b.service_id
         JOIN available_slots sl ON sl.id = b.slot_id
         WHERE sl.date = ? AND b.status = 'confirmed'
         ORDER BY sl.start_time ASC",
    )
    .bind(date)
    .fetch_all(pool)
    .await?;

    if bookings.is_empty() {
        bot.send_message(
            chat_id,
            format!("☀️ {} ({}) — записей нет, свободный день!", label, format_date_ru(date)),
        )
        .await?;
        return Ok(());
    }

    let mut text = format!(
        "📋 <b>{}</b> ({})\n\n",
        label,
        format_date_ru(date)
    );

    let total: i64 = bookings.iter().map(|b| b.service_price).sum();

    for (i, b) in bookings.iter().enumerate() {
        let mention = b
            .client_username
            .as_ref()
            .map(|u| format!("@{}", u))
            .unwrap_or_else(|| b.client_first_name.clone());

        text.push_str(&format!(
            "{}. <b>{} — {}</b>\n   👤 {} · 💅 {}\n   💰 {} ₽\n\n",
            i + 1,
            &b.start_time[..5],
            &b.end_time[..5],
            mention,
            b.service_name,
            b.service_price,
        ));
    }

    text.push_str(&format!(
        "━━━━━━━━━━━━━\n📊 Всего записей: <b>{}</b>\n💰 Итого: <b>{} ₽</b>",
        bookings.len(),
        total,
    ));

    // Add cancel buttons
    let buttons: Vec<Vec<InlineKeyboardButton>> = bookings
        .iter()
        .map(|b| {
            vec![InlineKeyboardButton::callback(
                format!(
                    "❌ {} ({} {})",
                    b.client_first_name,
                    &b.start_time[..5],
                    b.service_name,
                ),
                format!("admin_cancel:{}", b.id),
            )]
        })
        .collect();

    let keyboard = InlineKeyboardMarkup::new(buttons);
    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

// ── Reminders ──

async fn send_reminders(bot: Bot, pool: sqlx::SqlitePool) {
    // Initial delay: wait 10 seconds before first check
    tokio::time::sleep(Duration::from_secs(10)).await;

    let mut ticker = interval(Duration::from_secs(3600)); // check every hour

    loop {
        ticker.tick().await;

        let tomorrow = (chrono::Local::now() + chrono::TimeDelta::days(1))
            .format("%Y-%m-%d")
            .to_string();

        let bookings = sqlx::query_as::<_, BookingInfo>(
            "SELECT b.id, s.name as service_name, s.price as service_price,
                    sl.date, sl.start_time, sl.end_time,
                    b.client_tg_id, b.client_username, b.client_first_name
             FROM bookings b
             JOIN services s ON s.id = b.service_id
             JOIN available_slots sl ON sl.id = b.slot_id
             WHERE sl.date = ? AND b.status = 'confirmed' AND b.reminder_sent = 0",
        )
        .bind(&tomorrow)
        .fetch_all(&pool)
        .await;

        if let Ok(bookings) = bookings {
            for booking in bookings {
                let message = format!(
                    "💕 Напоминание!\n\n\
                     Завтра у тебя запись в <b>Bimbo Lashes</b>:\n\n\
                     💅 {}\n\
                     🕐 {} в {}\n\n\
                     Ждём тебя! ✨",
                    booking.service_name,
                    format_date_ru(&booking.date),
                    &booking.start_time[..5],
                );

                let sent = bot
                    .send_message(ChatId(booking.client_tg_id), &message)
                    .parse_mode(ParseMode::Html)
                    .await;

                if sent.is_ok() {
                    let _ =
                        sqlx::query("UPDATE bookings SET reminder_sent = 1 WHERE id = ?")
                            .bind(booking.id)
                            .execute(&pool)
                            .await;
                    tracing::info!("📬 Reminder sent to {}", booking.client_first_name);
                }
            }
        }
    }
}

// ── Date formatting helper ──

fn format_date_ru(date_str: &str) -> String {
    let months = [
        "января", "февраля", "марта", "апреля", "мая", "июня",
        "июля", "августа", "сентября", "октября", "ноября", "декабря",
    ];
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() != 3 {
        return date_str.to_string();
    }
    let day: u32 = parts[2].parse().unwrap_or(0);
    let month: usize = parts[1].parse::<usize>().unwrap_or(1) - 1;
    format!("{} {}", day, months.get(month).unwrap_or(&"???"))
}
