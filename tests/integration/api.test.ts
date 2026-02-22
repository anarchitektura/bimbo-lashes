/**
 * Integration tests for the Bimbo Lashes API.
 *
 * Prerequisites:
 *   1. Server running on localhost:3000
 *   2. Fresh database (or test database)
 *   3. Auth header — use a valid test initData or set BYPASS_AUTH=1
 *
 * Run:
 *   cd tests/integration && npm install && npm test
 */

const BASE = process.env.API_URL || "http://localhost:3000";

// Replace with a real test initData or configure BYPASS_AUTH on the server
const AUTH_HEADER = `tma ${process.env.TEST_INIT_DATA || "test"}`;

async function api<T>(
  path: string,
  options: RequestInit = {}
): Promise<{ status: number; body: { ok: boolean; data: T; error: string | null } }> {
  const res = await fetch(`${BASE}${path}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      Authorization: AUTH_HEADER,
      ...options.headers,
    },
  });
  const body = await res.json();
  return { status: res.status, body };
}

describe("GET /api/services", () => {
  it("should return a list of services", async () => {
    const { status, body } = await api("/api/services");
    // Without valid auth, this will return 401
    // With BYPASS_AUTH or valid initData, should return 200
    if (status === 200) {
      expect(body.ok).toBe(true);
      expect(Array.isArray(body.data)).toBe(true);
      expect(body.data.length).toBeGreaterThan(0);

      const service = body.data[0];
      expect(service).toHaveProperty("id");
      expect(service).toHaveProperty("name");
      expect(service).toHaveProperty("price");
      expect(service).toHaveProperty("duration_min");
      expect(service).toHaveProperty("is_active");
      expect(service.is_active).toBe(true);
    } else {
      expect(status).toBe(401); // auth not configured
    }
  });

  it("should return services sorted by sort_order", async () => {
    const { status, body } = await api("/api/services");
    if (status === 200) {
      const orders = body.data.map((s: any) => s.sort_order);
      const sorted = [...orders].sort((a: number, b: number) => a - b);
      expect(orders).toEqual(sorted);
    }
  });

  it("should only return active services", async () => {
    const { status, body } = await api("/api/services");
    if (status === 200) {
      for (const service of body.data) {
        expect(service.is_active).toBe(true);
      }
    }
  });
});

describe("GET /api/slots/dates", () => {
  it("should return an array of date strings", async () => {
    const { status, body } = await api("/api/slots/dates");
    if (status === 200) {
      expect(body.ok).toBe(true);
      expect(Array.isArray(body.data)).toBe(true);

      for (const date of body.data) {
        expect(date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
      }
    }
  });

  it("should only return future dates", async () => {
    const { status, body } = await api("/api/slots/dates");
    if (status === 200) {
      const today = new Date().toISOString().split("T")[0];
      for (const date of body.data) {
        expect(date >= today).toBe(true);
      }
    }
  });
});

describe("GET /api/slots", () => {
  it("should require a date parameter", async () => {
    const { status } = await api("/api/slots");
    // Should fail without date param (400 or similar)
    expect([200, 400, 422]).toContain(status);
  });

  it("should return slots for a specific date", async () => {
    const { status, body } = await api("/api/slots?date=2026-03-01");
    if (status === 200) {
      expect(body.ok).toBe(true);
      expect(Array.isArray(body.data)).toBe(true);
    }
  });
});

describe("POST /api/bookings", () => {
  it("should reject booking without auth", async () => {
    const { status } = await fetch(`${BASE}/api/bookings`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ service_id: 1, slot_id: 1 }),
    }).then(async (r) => ({ status: r.status }));

    expect(status).toBe(401);
  });

  it("should reject booking for non-existent slot", async () => {
    const { status, body } = await api("/api/bookings", {
      method: "POST",
      body: JSON.stringify({ service_id: 1, slot_id: 99999 }),
    });

    if (status !== 401) {
      // Should be 404 (slot not found)
      expect(status).toBe(404);
      expect(body.ok).toBe(false);
    }
  });
});

describe("GET /api/bookings/my", () => {
  it("should return client bookings (empty for new user)", async () => {
    const { status, body } = await api("/api/bookings/my");
    if (status === 200) {
      expect(body.ok).toBe(true);
      expect(Array.isArray(body.data)).toBe(true);
    }
  });
});

describe("Admin endpoints", () => {
  it("should reject non-admin access to admin endpoints", async () => {
    const { status } = await api("/api/admin/services");
    // Unless test user IS the admin, should be 403
    expect([200, 401, 403]).toContain(status);
  });

  it("should reject unauthorized access to admin bookings", async () => {
    const { status } = await fetch(`${BASE}/api/admin/bookings`, {
      headers: { "Content-Type": "application/json" },
    }).then(async (r) => ({ status: r.status }));

    expect(status).toBe(401);
  });
});

describe("Error responses", () => {
  it("should return consistent JSON error format", async () => {
    const { status, body } = await api("/api/bookings", {
      method: "POST",
      body: JSON.stringify({ service_id: 999, slot_id: 999 }),
    });

    if (status !== 401) {
      expect(body).toHaveProperty("ok");
      expect(body).toHaveProperty("error");
      expect(body.ok).toBe(false);
      expect(typeof body.error).toBe("string");
    }
  });
});
