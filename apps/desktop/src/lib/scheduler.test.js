import { describe, it, expect, beforeEach } from "vitest";
import {
  STORAGE_KEY,
  DEFAULT_SCHEDULE,
  loadSchedule,
  saveSchedule,
  normalizeSchedule,
  serializeTime,
  parseTime,
} from "./scheduler.js";

// Mock localStorage for node environment
class MockStorage {
  constructor() {
    this.store = {};
  }
  getItem(key) {
    return this.store[key] ?? null;
  }
  setItem(key, val) {
    this.store[key] = String(val);
  }
  removeItem(key) {
    delete this.store[key];
  }
  clear() {
    this.store = {};
  }
}

describe("Scheduler logic & persistence (scheduler.js)", () => {
  let mockStorage;

  beforeEach(() => {
    mockStorage = new MockStorage();
  });

  describe("Defaults and normalization", () => {
    it("returns DEFAULT_SCHEDULE when storage is empty", () => {
      const schedule = loadSchedule(mockStorage);
      expect(schedule).toEqual(DEFAULT_SCHEDULE);
      expect(schedule.days).toEqual([1, 2, 3, 4, 5, 6, 7]);
      expect(schedule.queue_size).toBe(2);
      expect(schedule.shutdown_when_done).toBe(false);
    });

    it("normalizes invalid or partial objects safely", () => {
      expect(normalizeSchedule(null)).toEqual(DEFAULT_SCHEDULE);
      expect(normalizeSchedule(undefined)).toEqual(DEFAULT_SCHEDULE);
      expect(normalizeSchedule("invalid")).toEqual(DEFAULT_SCHEDULE);

      const normalized = normalizeSchedule({
        start_hour: 25, // clamped to 23
        start_minute: -5, // clamped to 0
        queue_size: 99, // clamped to 10
        days: [1, 2, 8, 9, 2], // 8 and 9 filtered, duplicate 2 removed
        shutdown_when_done: 1,
      });

      expect(normalized.start_hour).toBe(23);
      expect(normalized.start_minute).toBe(0);
      expect(normalized.queue_size).toBe(10);
      expect(normalized.days).toEqual([1, 2]);
      expect(normalized.shutdown_when_done).toBe(true);
    });
  });

  describe("localStorage save/load round-trip", () => {
    it("saves and loads schedule configuration round-trip", () => {
      const customConfig = {
        enabled: true,
        start_hour: 3,
        start_minute: 30,
        stop_hour: 7,
        stop_minute: 45,
        days: [2, 4, 6],
        queue_size: 4,
        shutdown_when_done: true,
      };

      const saved = saveSchedule(customConfig, mockStorage);
      expect(saved.start_hour).toBe(3);
      expect(saved.start_minute).toBe(30);

      // Verify raw JSON in storage
      const rawStored = mockStorage.getItem(STORAGE_KEY);
      expect(rawStored).toBeTruthy();
      const parsed = JSON.parse(rawStored);
      expect(parsed.days).toEqual([2, 4, 6]);
      expect(parsed.shutdown_when_done).toBe(true);

      // Load back
      const loaded = loadSchedule(mockStorage);
      expect(loaded).toEqual(customConfig);
    });

    it("falls back to default on corrupted JSON in localStorage", () => {
      mockStorage.setItem(STORAGE_KEY, "{corrupted-json");
      const loaded = loadSchedule(mockStorage);
      expect(loaded).toEqual(DEFAULT_SCHEDULE);
    });
  });

  describe("Days and time serialization", () => {
    it("serializes hour and minute into HH:MM string", () => {
      expect(serializeTime(2, 5)).toBe("02:05");
      expect(serializeTime(14, 30)).toBe("14:30");
      expect(serializeTime(0, 0)).toBe("00:00");
      expect(serializeTime(23, 59)).toBe("23:59");
    });

    it("parses HH:MM string into { hour, minute }", () => {
      expect(parseTime("02:05")).toEqual({ hour: 2, minute: 5 });
      expect(parseTime("14:30")).toEqual({ hour: 14, minute: 30 });
      expect(parseTime("23:59")).toEqual({ hour: 23, minute: 59 });
      expect(parseTime("invalid")).toEqual({ hour: 0, minute: 0 });
      expect(parseTime(null)).toEqual({ hour: 0, minute: 0 });
    });

    it("round-trips serializeTime and parseTime", () => {
      const times = [
        { h: 0, m: 0 },
        { h: 8, m: 15 },
        { h: 12, m: 0 },
        { h: 23, m: 45 },
      ];
      for (const t of times) {
        const serialized = serializeTime(t.h, t.m);
        const parsed = parseTime(serialized);
        expect(parsed.hour).toBe(t.h);
        expect(parsed.minute).toBe(t.m);
      }
    });

    it("serializes and normalizes days correctly", () => {
      const schedule = normalizeSchedule({
        days: [7, 1, 5, 3],
      });
      // Should be sorted and filtered to Mon(1)..Sun(7)
      expect(schedule.days).toEqual([1, 3, 5, 7]);
    });
  });
});
