import { test, expect } from "@playwright/test";

/**
 * E2E tests for the client booking flow.
 *
 * NOTE: These tests run against the Vite dev server with the API proxied
 * to localhost:3000. The Telegram initData auth is mocked — in real
 * Telegram WebView, initData is injected by the client.
 *
 * Prerequisites:
 *   1. Server running on localhost:3000
 *   2. Vite dev server on localhost:5173 (auto-started by Playwright)
 *   3. Database with seeded services
 *
 * Run:
 *   cd tests/e2e && npx playwright test
 */

test.describe("Home page", () => {
  test("should display the salon name and services list", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Bimbo Lashes")).toBeVisible();
    // Wait for services to load — 2 main services seeded
    await expect(page.getByText("Наращивание ресниц")).toBeVisible({ timeout: 10000 });
    await expect(page.getByText("Коррекция")).toBeVisible();
  });

  test("should show prices for each main service", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Наращивание ресниц")).toBeVisible({ timeout: 10000 });
    // 2 main services → 2 price elements with ₽
    const prices = page.getByText("₽");
    await expect(prices).toHaveCount(2);
  });

  test("should show 'Мои записи' button", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Мои записи")).toBeVisible();
  });

  test("should show service descriptions and durations", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Наращивание ресниц")).toBeVisible({ timeout: 10000 });
    // Service descriptions
    await expect(page.getByText("Любой объём")).toBeVisible();
    await expect(page.getByText("Коррекция наращивания")).toBeVisible();
    // Duration badges
    await expect(page.getByText("2 ч")).toBeVisible();
    await expect(page.getByText("1 ч").first()).toBeVisible();
  });
});

test.describe("Booking flow — Наращивание ресниц", () => {
  test("clicking наращивание shows addon selector", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Наращивание ресниц")).toBeVisible({ timeout: 10000 });
    await page.getByText("Наращивание ресниц").click();
    // Addon selector should appear (for services ≥ 120 min)
    await expect(page.getByText("Выбрать время →")).toBeVisible();
    // Should show addon checkbox for lower lashes
    await expect(page.getByText("нижних")).toBeVisible();
  });

  test("addon selector shows correct total price", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Наращивание ресниц")).toBeVisible({ timeout: 10000 });
    await page.getByText("Наращивание ресниц").click();
    // Total without addon
    await expect(page.getByText("Итого:")).toBeVisible();
    await expect(page.getByText("2 500 ₽")).toBeVisible();
  });

  test("can go back from addon selector", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Наращивание ресниц")).toBeVisible({ timeout: 10000 });
    await page.getByText("Наращивание ресниц").click();
    await expect(page.getByText("← Назад")).toBeVisible();
    await page.getByText("← Назад").click();
    // Services list should be visible again
    await expect(page.getByText("Наращивание ресниц")).toBeVisible();
    await expect(page.getByText("Коррекция")).toBeVisible();
  });
});

test.describe("Booking flow — Коррекция", () => {
  test("clicking коррекция navigates directly to date selection", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Коррекция")).toBeVisible({ timeout: 10000 });
    await page.getByText("Коррекция").click();
    // Should go straight to date picker (no addon selector for short services)
    await expect(page.getByText("Выбери дату")).toBeVisible();
  });

  test("progress bar shows 4 steps", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Коррекция")).toBeVisible({ timeout: 10000 });
    await page.getByText("Коррекция").click();
    // 4 progress bars
    const bars = page.locator(".h-1.rounded-full");
    await expect(bars).toHaveCount(4);
  });

  test("shows empty state or date chips", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Коррекция")).toBeVisible({ timeout: 10000 });
    await page.getByText("Коррекция").click();
    // Either available dates or empty state
    const emptyState = page.getByText("Нет доступных дат");
    const dateChips = page.locator("[class*='chip']");
    await expect(emptyState.or(dateChips.first())).toBeVisible({ timeout: 5000 });
  });
});

test.describe("My Bookings page", () => {
  test("shows empty state when no bookings", async ({ page }) => {
    await page.goto("/");
    await page.getByText("Мои записи").click();
    await expect(
      page.getByText("Пока нет записей").or(page.getByText("Мои записи").nth(1))
    ).toBeVisible({ timeout: 5000 });
  });
});

test.describe("Responsive design", () => {
  test("works on 320px width (small phone)", async ({ page }) => {
    await page.setViewportSize({ width: 320, height: 568 });
    await page.goto("/");
    await expect(page.getByText("Bimbo Lashes")).toBeVisible();
    // All touch targets should be at least 36px height
    const buttons = page.locator("button");
    const count = await buttons.count();
    for (let i = 0; i < Math.min(count, 5); i++) {
      const box = await buttons.nth(i).boundingBox();
      if (box) {
        expect(box.height).toBeGreaterThanOrEqual(36);
      }
    }
  });

  test("services load on 375px width (iPhone SE)", async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });
    await page.goto("/");
    await expect(page.getByText("Наращивание ресниц")).toBeVisible({ timeout: 10000 });
    await expect(page.getByText("Коррекция")).toBeVisible();
  });
});
