import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import FindingHierarchy from "@/components/diagnostics/FindingHierarchy.vue";
import type { IniFinding } from "@/services/tauri";

const sample: IniFinding = {
  id: 1, scan_run_id: 1, machine_id: 11, rule_id: "R001", severity: "critical",
  category: "project", file_path: "C:\\F\\Config\\DefaultEngine.ini",
  section: "DDC", key_name: "Path", line_number: 42,
  snippet_before: "Path=X", snippet_after: null,
  recommended_action: "set", recommended_value: "Y",
  symptom: "", rationale: "", fixed_at: null, skipped_at: null,
};

describe("FindingHierarchy", () => {
  it("renders machine groups", () => {
    const w = mount(FindingHierarchy, {
      props: { findings: [sample], selectedId: null, hostnameById: { 11: "RENDER-01" }, groupBy: "machine" },
    });
    expect(w.text()).toContain("RENDER-01");
    expect(w.text()).toContain("R001");
  });
  it("emits select on click", async () => {
    const w = mount(FindingHierarchy, {
      props: { findings: [sample], selectedId: null, hostnameById: { 11: "RENDER-01" }, groupBy: "machine" },
    });
    await w.find("[data-finding-row]").trigger("click");
    expect(w.emitted("select")?.[0]?.[0]).toMatchObject({ id: 1 });
  });
});
