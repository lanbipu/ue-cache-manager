import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UecmScoreTile from "@/components/primitives/UecmScoreTile.vue";

describe("UecmScoreTile", () => {
  it("renders score + verdict", () => {
    const w = mount(UecmScoreTile, { props: { label: "Cluster", score: 70, tone: "warning", verdict: "DEGRADED" } });
    expect(w.text()).toContain("70");
    expect(w.text()).toContain("DEGRADED");
  });
});
