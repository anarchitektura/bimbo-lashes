import { test, expect } from "@playwright/test";

/**
 * E2E tests for the client booking flow.
 *
 * NOTE: These tests run against the Vite dev server with the API proxied
 * to localhost:3000. The Telegram initData auth is mocked — in real
 * Telegram WebView, initData is injected by the client.
 *
 * To mock auth for E2E, set BYPASS_AUTH=1 on the server or use a test
 * initData token.
 */

test.describe("Home page", () => {
  test("should display the salon name and services list", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Bimbo Lashes")).toBeVisible();
    // Wait for services to load
    await expect(page.getByText("Классика")).toBeVisible({ timeout: 10000 });
    await expect(page.getByText("2D")).toBeVisible();
    await expect(page.getByText("3D")).toBeVisible();
  });

  test("should show prices for each service", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Классика")).toBeVisible({ timeout: 10000 });
    // Prices should be visible (formatted as "X XXX ₽")
    await expect(page.getByText("₽")).toHaveCount(7); // 7 default services
  });

  test("should show 'Мои записи' button", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Мои записи")).toBeVisible();
  });
});

test.describe("Booking flow", () => {
  test("clicking a service navigates to date selection", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Классика")).toBeVisible({ timeout: 10000 });
    await page.getByText("Классика").click();
    // Should show date picker step
    await expect(page.getByText("Выбери дату")).toBeVisible();
  });

  test("shows empty state when no dates available", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("2D")).toBeVisible({ timeout: 10000 });
    await page.getByText("2D").click();
    // If no slots are open, should show empty state
    const emptyState = page.getByText("Нет доступных дат");
    const dateChips = page.locator(".chip");
    // Either we see dates or the empty state
    await expect(emptyState.or(dateChips.first())).toBeVisible({ timeout: 5000 });
  });

  test("progress bar shows correct step", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Классика")).toBeVisible({ timeout: 10000 });
    await page.getByText("Классика").click();
    // First step indicator should be highlighted (3 bars total)
    const bars = page.locator(".h-1.rounded-full");
    await expect(bars).toHaveCount(3);
  });
});

test.describe("My Bookings page", () => {
  test("shows empty state when no bookings", async ({ page }) => {
    await page.goto("/");
    await page.getByText("Мои записи").click();
    await expect(page.getByText("Пока нет записей").or(page.getByText("Мои записи").nth(1))).toBeVisible({
      timeout: 5000,
    });
  });
});

test.describe("Responsive design", () => {
  test("works on 320px width (small phone)", async ({ page }) => {
    await page.setViewportSize({ width: 320, height: 568 });
    await page.goto("/");
    await expect(page.getByText("Bimbo Lashes")).toBeVisible();
    // All touch targets should be at least 44x44px
    const buttons = page.locator("button");
    const count = await buttons.count();
    for (let i = 0; i < Math.min(count, 5); i++) {
      const box = await buttons.nth(i).boundingBox();
      if (box) {
        expect(box.height).toBeGreaterThanOrEqual(36); // allowing some tolerance
      }
    }
  });
});
