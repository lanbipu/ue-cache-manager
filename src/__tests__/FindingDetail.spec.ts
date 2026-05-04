import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import FindingDetail from "@/components/diagnostics/FindingDetail.vue";

const finding = {
  id: 1,
  scan_run_id: 1,
  machine_id: 1,
  rule_id: "R001",
  severity: "critical",
  category: "project",
  file_path: "C:\\P\\Config\\DefaultEngine.ini",
  section: "/Script/UnrealEd.DerivedDataCacheSettings",
  key_name: "Path",
  line_number: 2,
  snippet_before: "Path=D:\\Old",
  snippet_after: "EnvPathOverride=UE-SharedDataCachePath",
  recommended_action: "set_env_override_remove_path",
  recommended_value: "UE-SharedDataCachePath",
  symptom: "bad path",
  rationale: "env wins",
  fixed_at: null,
  skipped_at: null,
} as const;

describe("FindingDetail", () => {
  it("renders diagnostic detail and emits apply", async () => {
    const wrapper = mount(FindingDetail, { props: { finding } });
    expect(wrapper.find("[data-code-block]").text()).toContain("Path=");
    await wrapper.find("[data-apply-finding-btn]").trigger("click");
    expect(wrapper.emitted("apply")?.[0]?.[0]).toMatchObject({ rule_id: "R001" });
  });
});
