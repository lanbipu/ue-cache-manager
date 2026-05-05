import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UecmCodeBlock from "@/components/primitives/UecmCodeBlock.vue";

describe("UecmCodeBlock", () => {
  it("renders line numbers starting from startLine", () => {
    const w = mount(UecmCodeBlock, { props: { code: "a\nb", startLine: 10 } });
    const html = w.html();
    expect(html).toContain("10");
    expect(html).toContain("11");
  });
  it("highlights specified line", () => {
    const w = mount(UecmCodeBlock, { props: { code: "a\nb\nc", startLine: 5, highlightLine: 6 } });
    expect(w.html()).toMatch(/bg-yellow-500\/15/);
  });
});
