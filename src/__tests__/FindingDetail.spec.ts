import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import FindingDetail from "@/components/diagnostics/FindingDetail.vue";
import type { IniFinding } from "@/services/tauri";

const sample: IniFinding = {
  id: 1, scan_run_id: 1, machine_id: 11, rule_id: "R001", severity: "critical",
  category: "project", file_path: "C:\\f.ini",
  section: "DDC", key_name: "Path", line_number: 1,
  snippet_before: "Path=X", snippet_after: "EnvPathOverride=Y",
  recommended_action: "set", recommended_value: "Y",
  symptom: "DDC silent fallback", rationale: "Hardcoded path.", fixed_at: null, skipped_at: null,
};

describe("FindingDetail", () => {
  it("renders empty state when null", () => {
    const w = mount(FindingDetail, { props: { finding: null, busy: false } });
    expect(w.find("[data-finding-empty]").exists()).toBe(true);
  });
  it("emits apply on Apply click", async () => {
    const w = mount(FindingDetail, { props: { finding: sample, busy: false } });
    await w.find("[data-apply-btn]").trigger("click");
    expect(w.emitted("apply")?.[0]?.[0]).toMatchObject({ id: 1 });
  });
});
