import { test, expect } from "@playwright/test";

/**
 * E2E tests for the admin panel.
 *
 * NOTE: Admin access requires the user's Telegram ID to match ADMIN_TG_ID.
 * For E2E tests, mock the initData or set BYPASS_AUTH=1 with a test admin ID.
 */

test.describe("Admin panel", () => {
  test.skip(true, "Requires auth mock — enable when test auth is configured");

  test("admin button is visible for admin user", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Админ")).toBeVisible();
  });

  test("admin page shows today/tomorrow/week tabs", async ({ page }) => {
    await page.goto("/");
    await page.getByText("Админ").click();
    await expect(page.getByText("Сегодня")).toBeVisible();
    await expect(page.getByText("Завтра")).toBeVisible();
    await expect(page.getByText("Неделя")).toBeVisible();
  });

  test("schedule page shows date chips for 14 days", async ({ page }) => {
    await page.goto("/");
    await page.getByText("Админ").click();
    await page.getByText("Расписание").click();
    const chips = page.locator(".chip");
    // 14 date chips + 3 template chips + 1 custom
    await expect(chips.first()).toBeVisible();
    expect(await chips.count()).toBeGreaterThanOrEqual(14);
  });

  test("can add slots via morning template", async ({ page }) => {
    await page.goto("/");
    await page.getByText("Админ").click();
    await page.getByText("Расписание").click();
    await page.getByText("Утро 9–13").click();
    // Should see slots appear
    await expect(page.getByText("09:00")).toBeVisible({ timeout: 5000 });
    await expect(page.getByText("11:00")).toBeVisible();
  });

  test("can open custom slot form", async ({ page }) => {
    await page.goto("/");
    await page.getByText("Админ").click();
    await page.getByText("Расписание").click();
    await page.getByText("Своё время").click();
    // Time inputs should appear
    await expect(page.getByText("Начало")).toBeVisible();
    await expect(page.getByText("Конец")).toBeVisible();
    await expect(page.getByText("Добавить слот")).toBeVisible();
  });

  test("services page shows list of services", async ({ page }) => {
    await page.goto("/");
    await page.getByText("Админ").click();
    await page.getByText("Услуги").click();
    await expect(page.getByText("Классика")).toBeVisible({ timeout: 5000 });
    await expect(page.getByText("Новая")).toBeVisible();
  });
});
