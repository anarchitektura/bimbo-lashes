/**
 * Integration tests for the Bimbo Lashes API.
 *
 * Prerequisites:
 *   1. Server running on localhost:3000 with:
 *      BOT_TOKEN=7777777777:AAFake_Test_Token_For_Unit_Tests
 *      ADMIN_TG_ID=12345
 *      DATABASE_URL=sqlite:test.db?mode=rwc
 *      WEBAPP_URL=https://example.com
 *      YOOKASSA_SHOP_ID=test YOOKASSA_SECRET_KEY=test
 *   2. Fresh database (auto-created by the server)
 *
 * Run:
 *   cd tests/integration && npx vitest run api
 */

import { createHmac } from "node:crypto";

const BASE = process.env.API_URL || "http://localhost:3000";
const TEST_BOT_TOKEN = "7777777777:AAFake_Test_Token_For_Unit_Tests";
const TEST_ADMIN_TG_ID = 12345;
const TEST_USER_TG_ID = 99999;

// ---------------------------------------------------------------------------
// Telegram initData generation (HMAC-SHA256, same algo as Rust server)
// ---------------------------------------------------------------------------

function buildInitData(
  botToken: string,
  userId: number,
  firstName: string,
  extra: Record<string, string> = {}
): string {
  const userJson = JSON.stringify({
    id: userId,
    first_name: firstName,
    language_code: "ru",
  });

  const authDate = Math.floor(Date.now() / 1000);

  const params: Record<string, string> = {
    auth_date: authDate.toString(),
    user: userJson,
    ...extra,
  };

  // data_check_string: sorted key=value pairs joined by \n (excluding hash)
  const dataCheckString = Object.keys(params)
    .sort()
    .map((k) => `${k}=${params[k]}`)
    .join("\n");

  // secret_key = HMAC-SHA256("WebAppData", bot_token)
  const secretKey = createHmac("sha256", "WebAppData")
    .update(botToken)
    .digest();

  // hash = HMAC-SHA256(secret_key, data_check_string)
  const hash = createHmac("sha256", secretKey)
    .update(dataCheckString)
    .digest("hex");

  // URL-encode all params + hash
  const urlParams = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    urlParams.set(k, v);
  }
  urlParams.set("hash", hash);
  return urlParams.toString();
}

function userAuth(): string {
  return `tma ${buildInitData(TEST_BOT_TOKEN, TEST_USER_TG_ID, "TestUser")}`;
}

function adminAuth(): string {
  return `tma ${buildInitData(TEST_BOT_TOKEN, TEST_ADMIN_TG_ID, "Admin")}`;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function api<T = any>(
  path: string,
  options: RequestInit = {},
  auth?: string
): Promise<{ status: number; body: { ok: boolean; data: T; error: string | null } }> {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(options.headers as Record<string, string>),
  };
  if (auth) {
    headers["Authorization"] = auth;
  }
  const res = await fetch(`${BASE}${path}`, {
    ...options,
    headers,
  });
  const body = await res.json();
  return { status: res.status, body };
}

async function rawFetch(
  path: string,
  options: RequestInit = {}
): Promise<{ status: number; body: any }> {
  const res = await fetch(`${BASE}${path}`, options);
  const body = await res.json();
  return { status: res.status, body };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("GET /api/health", () => {
  it("should return healthy status", async () => {
    const { status, body } = await rawFetch("/api/health");
    expect(status).toBe(200);
    expect(body.status).toBe("ok");
    expect(body).toHaveProperty("version");
    expect(body).toHaveProperty("uptime_secs");
    expect(body.db_ok).toBe(true);
  });
});

describe("GET /api/services (no auth)", () => {
  it("should return a list of active main services", async () => {
    const { status, body } = await api("/api/services");
    expect(status).toBe(200);
    expect(body.ok).toBe(true);
    expect(Array.isArray(body.data)).toBe(true);

    // Fresh DB has seeded services
    if (body.data.length > 0) {
      const svc = body.data[0];
      expect(svc).toHaveProperty("id");
      expect(svc).toHaveProperty("name");
      expect(svc).toHaveProperty("price");
      expect(svc).toHaveProperty("duration_min");
      expect(svc).toHaveProperty("is_active");
      expect(svc.is_active).toBe(true);
    }
  });

  it("should return services sorted by sort_order", async () => {
    const { status, body } = await api("/api/services");
    if (status === 200 && body.data.length > 1) {
      const orders = body.data.map((s: any) => s.sort_order);
      const sorted = [...orders].sort((a: number, b: number) => a - b);
      expect(orders).toEqual(sorted);
    }
  });

  it("should only return active services", async () => {
    const { status, body } = await api("/api/services");
    expect(status).toBe(200);
    for (const svc of body.data) {
      expect(svc.is_active).toBe(true);
    }
  });
});

describe("GET /api/available-dates", () => {
  it("should return an array of date strings", async () => {
    const { status, body } = await api("/api/available-dates?service_id=1");
    expect(status).toBe(200);
    expect(body.ok).toBe(true);
    expect(Array.isArray(body.data)).toBe(true);

    for (const date of body.data) {
      expect(date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    }
  });

  it("should work without service_id", async () => {
    const { status, body } = await api("/api/available-dates");
    expect(status).toBe(200);
    expect(body.ok).toBe(true);
    expect(Array.isArray(body.data)).toBe(true);
  });

  it("should only return future dates", async () => {
    const { status, body } = await api("/api/available-dates");
    if (status === 200) {
      const today = new Date().toISOString().split("T")[0];
      for (const date of body.data) {
        expect(date >= today).toBe(true);
      }
    }
  });

  it("alias /api/slots/dates should also work", async () => {
    const { status, body } = await api("/api/slots/dates");
    expect(status).toBe(200);
    expect(body.ok).toBe(true);
    expect(Array.isArray(body.data)).toBe(true);
  });
});

describe("GET /api/available-times", () => {
  it("should reject missing params with 400", async () => {
    // Missing params — Axum returns 400 plain text (not JSON)
    const res = await fetch(`${BASE}/api/available-times`);
    expect(res.status).toBe(400);
  });

  it("should return times structure for a valid date", async () => {
    const { status, body } = await api(
      "/api/available-times?date=2026-04-01&service_id=1"
    );
    expect(status).toBe(200);
    expect(body.ok).toBe(true);
    expect(body.data).toHaveProperty("mode");
    expect(["free", "tight"]).toContain(body.data.mode);
    expect(Array.isArray(body.data.times)).toBe(true);

    for (const block of body.data.times) {
      expect(block).toHaveProperty("start_time");
      expect(block).toHaveProperty("end_time");
      expect(block.start_time).toMatch(/^\d{2}:\d{2}$/);
      expect(block.end_time).toMatch(/^\d{2}:\d{2}$/);
    }
  });
});

describe("GET /api/calendar", () => {
  it("should return calendar data for a month", async () => {
    const { status, body } = await api(
      "/api/calendar?year=2026&month=4&service_id=1"
    );
    expect(status).toBe(200);
    expect(body.ok).toBe(true);
    expect(Array.isArray(body.data)).toBe(true);

    for (const day of body.data) {
      expect(day).toHaveProperty("date");
      expect(day).toHaveProperty("total");
      expect(day).toHaveProperty("free");
      expect(day).toHaveProperty("bookable");
      expect(day.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    }
  });

  it("should work without service_id", async () => {
    const { status, body } = await api("/api/calendar?year=2026&month=4");
    expect(status).toBe(200);
    expect(body.ok).toBe(true);
  });
});

describe("GET /api/addon-info (no auth)", () => {
  it("should return addon info", async () => {
    const { status, body } = await api("/api/addon-info");
    expect(status).toBe(200);
    expect(body.ok).toBe(true);
  });
});

describe("POST /api/bookings — auth required", () => {
  it("should reject booking without auth header", async () => {
    const { status } = await rawFetch("/api/bookings", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        service_id: 1,
        date: "2026-04-01",
        start_time: "12:00",
        with_lower_lashes: false,
      }),
    });
    expect(status).toBe(401);
  });

  it("should reject booking with invalid auth", async () => {
    const { status } = await api(
      "/api/bookings",
      {
        method: "POST",
        body: JSON.stringify({
          service_id: 1,
          date: "2026-04-01",
          start_time: "12:00",
          with_lower_lashes: false,
        }),
      },
      "tma invalid_init_data_here"
    );
    expect(status).toBe(401);
  });

  it("should reject booking with expired auth", async () => {
    // auth_date 48 hours ago — beyond 24h limit
    const userJson = JSON.stringify({
      id: TEST_USER_TG_ID,
      first_name: "Expired",
      language_code: "ru",
    });
    const expiredAuthDate = Math.floor(Date.now() / 1000) - 48 * 3600;
    const params: Record<string, string> = {
      auth_date: expiredAuthDate.toString(),
      user: userJson,
    };
    const dataCheckString = Object.keys(params)
      .sort()
      .map((k) => `${k}=${params[k]}`)
      .join("\n");
    const secretKey = createHmac("sha256", "WebAppData")
      .update(TEST_BOT_TOKEN)
      .digest();
    const hash = createHmac("sha256", secretKey)
      .update(dataCheckString)
      .digest("hex");
    const urlParams = new URLSearchParams();
    for (const [k, v] of Object.entries(params)) urlParams.set(k, v);
    urlParams.set("hash", hash);

    const { status } = await api(
      "/api/bookings",
      {
        method: "POST",
        body: JSON.stringify({
          service_id: 1,
          date: "2026-04-01",
          start_time: "12:00",
          with_lower_lashes: false,
        }),
      },
      `tma ${urlParams.toString()}`
    );
    expect(status).toBe(401);
  });
});

describe("GET /api/bookings/my — auth required", () => {
  it("should return empty bookings for a new user", async () => {
    const { status, body } = await api("/api/bookings/my", {}, userAuth());
    expect(status).toBe(200);
    expect(body.ok).toBe(true);
    expect(Array.isArray(body.data)).toBe(true);
    expect(body.data.length).toBe(0);
  });

  it("should reject without auth", async () => {
    const { status } = await rawFetch("/api/bookings/my");
    expect(status).toBe(401);
  });
});

describe("Admin endpoints — access control", () => {
  it("GET /api/admin/bookings should reject without auth", async () => {
    const { status } = await rawFetch("/api/admin/bookings");
    expect(status).toBe(401);
  });

  it("GET /api/admin/bookings should reject non-admin user", async () => {
    const { status } = await api("/api/admin/bookings", {}, userAuth());
    expect(status).toBe(403);
  });

  it("GET /api/admin/bookings should allow admin", async () => {
    const { status, body } = await api("/api/admin/bookings", {}, adminAuth());
    expect(status).toBe(200);
    expect(body.ok).toBe(true);
    expect(Array.isArray(body.data)).toBe(true);
  });

  it("GET /api/admin/services should allow admin", async () => {
    const { status, body } = await api("/api/admin/services", {}, adminAuth());
    expect(status).toBe(200);
    expect(body.ok).toBe(true);
    expect(Array.isArray(body.data)).toBe(true);
  });

  it("POST /api/admin/services should reject non-admin", async () => {
    const { status } = await api(
      "/api/admin/services",
      {
        method: "POST",
        body: JSON.stringify({
          name: "Hack",
          price: 100,
          duration_min: 60,
        }),
      },
      userAuth()
    );
    expect(status).toBe(403);
  });
});

describe("Error response format", () => {
  it("should return consistent JSON error for missing auth", async () => {
    const { status, body } = await rawFetch("/api/bookings/my");
    expect(status).toBe(401);
    expect(body).toHaveProperty("ok");
    expect(body.ok).toBe(false);
    expect(body).toHaveProperty("error");
    expect(typeof body.error).toBe("string");
  });

  it("should return consistent JSON error for forbidden", async () => {
    const { status, body } = await api("/api/admin/bookings", {}, userAuth());
    expect(status).toBe(403);
    expect(body.ok).toBe(false);
    expect(typeof body.error).toBe("string");
  });
});
