import { beforeEach, describe, expect, it } from "vitest";
import { nextTick } from "vue";
import { useColorMode } from "@/composables/useColorMode";

const storage = (() => {
  let data: Record<string, string> = {};
  return {
    getItem: (key: string) => data[key] ?? null,
    setItem: (key: string, value: string) => {
      data[key] = value;
    },
    removeItem: (key: string) => {
      delete data[key];
    },
    clear: () => {
      data = {};
    },
  };
})();

describe("useColorMode", () => {
  beforeEach(() => {
    Object.defineProperty(window, "localStorage", {
      value: storage,
      configurable: true,
    });
    storage.clear();
    document.documentElement.className = "";
  });

  it("defaults to dark when nothing stored", () => {
    const { mode, resolved } = useColorMode();
    expect(mode.value).toBe("dark");
    expect(resolved.value).toBe("dark");
  });

  it("applies dark class on html when set to dark", async () => {
    const { mode } = useColorMode();
    mode.value = "dark";
    await nextTick();
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("removes dark class when set to light", async () => {
    const { mode } = useColorMode();
    mode.value = "light";
    await nextTick();
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });
});
