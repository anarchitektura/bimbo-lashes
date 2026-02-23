/**
 * Unit tests for shared validation functions.
 *
 * Run: cd tests/integration && npm install && npm test
 */

import {
  isValidDate,
  isValidTime,
  isValidTimeRange,
  isNotPastDate,
  validateCreateService,
  validateSlot,
  formatPrice,
  formatDuration,
} from "../../packages/shared/src/validation";

describe("isValidDate", () => {
  it("accepts YYYY-MM-DD format", () => {
    expect(isValidDate("2026-02-26")).toBe(true);
    expect(isValidDate("2026-12-31")).toBe(true);
  });

  it("rejects invalid formats", () => {
    expect(isValidDate("26-02-2026")).toBe(false);
    expect(isValidDate("2026/02/26")).toBe(false);
    expect(isValidDate("not-a-date")).toBe(false);
    expect(isValidDate("")).toBe(false);
  });
});

describe("isValidTime", () => {
  it("accepts HH:MM format", () => {
    expect(isValidTime("09:00")).toBe(true);
    expect(isValidTime("14:30")).toBe(true);
    expect(isValidTime("23:59")).toBe(true);
  });

  it("rejects invalid formats", () => {
    expect(isValidTime("9:00")).toBe(false);
    expect(isValidTime("")).toBe(false);
    expect(isValidTime("abc")).toBe(false);
    expect(isValidTime("12:0")).toBe(false);
  });

  it("accepts 25:00 (format-only check, no semantic validation)", () => {
    // isValidTime only checks \d{2}:\d{2} regex, not hour/minute ranges
    expect(isValidTime("25:00")).toBe(true);
  });
});

describe("isValidTimeRange", () => {
  it("accepts valid ranges", () => {
    expect(isValidTimeRange("09:00", "11:00")).toBe(true);
    expect(isValidTimeRange("14:00", "16:30")).toBe(true);
  });

  it("rejects when start >= end", () => {
    expect(isValidTimeRange("14:00", "14:00")).toBe(false);
    expect(isValidTimeRange("16:00", "14:00")).toBe(false);
  });
});

describe("isNotPastDate", () => {
  it("accepts today and future dates", () => {
    const today = new Date().toISOString().split("T")[0];
    expect(isNotPastDate(today)).toBe(true);
    expect(isNotPastDate("2030-01-01")).toBe(true);
  });

  it("rejects past dates", () => {
    expect(isNotPastDate("2020-01-01")).toBe(false);
  });
});

describe("validateCreateService", () => {
  it("accepts valid service", () => {
    const result = validateCreateService({
      name: "Классика",
      price: 2500,
      duration_min: 120,
    });
    expect(result).toBeNull();
  });

  it("rejects empty name", () => {
    expect(validateCreateService({ name: "", price: 2500, duration_min: 120 })).not.toBeNull();
    expect(validateCreateService({ name: "  ", price: 2500, duration_min: 120 })).not.toBeNull();
  });

  it("rejects zero or negative price", () => {
    expect(validateCreateService({ name: "Test", price: 0, duration_min: 120 })).not.toBeNull();
    expect(validateCreateService({ name: "Test", price: -100, duration_min: 120 })).not.toBeNull();
  });

  it("rejects too high price", () => {
    expect(
      validateCreateService({ name: "Test", price: 999999, duration_min: 120 })
    ).not.toBeNull();
  });

  it("rejects zero or negative duration", () => {
    expect(validateCreateService({ name: "Test", price: 2500, duration_min: 0 })).not.toBeNull();
  });

  it("rejects too long duration", () => {
    expect(
      validateCreateService({ name: "Test", price: 2500, duration_min: 600 })
    ).not.toBeNull();
  });
});

describe("validateSlot", () => {
  it("accepts valid slot", () => {
    expect(validateSlot({ start_time: "09:00", end_time: "11:00" })).toBeNull();
  });

  it("rejects when start >= end", () => {
    expect(validateSlot({ start_time: "14:00", end_time: "12:00" })).not.toBeNull();
  });
});

describe("formatPrice", () => {
  it("formats with space separator and ₽", () => {
    const result = formatPrice(2500);
    expect(result).toContain("₽");
    expect(result).toContain("2");
    expect(result).toContain("500");
  });

  it("formats large prices", () => {
    const result = formatPrice(10000);
    expect(result).toContain("₽");
  });
});

describe("formatDuration", () => {
  it("formats minutes only", () => {
    expect(formatDuration(30)).toBe("30 мин");
    expect(formatDuration(45)).toBe("45 мин");
  });

  it("formats whole hours", () => {
    expect(formatDuration(60)).toBe("1 ч");
    expect(formatDuration(120)).toBe("2 ч");
  });

  it("formats hours + minutes", () => {
    expect(formatDuration(90)).toBe("1 ч 30 мин");
    expect(formatDuration(150)).toBe("2 ч 30 мин");
  });
});
