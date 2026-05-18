import { describe, it, expect } from "vitest";
import { PROBE_LAYER_MAP } from "@/services/tauri";
import en from "@/locales/en";
import zh from "@/locales/zh";

describe("health probe locale coverage", () => {
  const probeKeys = Object.keys(PROBE_LAYER_MAP);
  const layers = ["l1_port", "l2_bootstrap", "l3_business"];

  function pick(obj: any, path: string[]): any {
    return path.reduce((acc, key) => (acc && acc[key] !== undefined ? acc[key] : undefined), obj);
  }

  for (const locale of [{ name: "en", t: en }, { name: "zh", t: zh }]) {
    it(`${locale.name}: every probe key in PROBE_LAYER_MAP has a label`, () => {
      for (const probeKey of probeKeys) {
        const label = pick(locale.t, ["healthCheck", "probe", probeKey]);
        expect(label, `missing healthCheck.probe.${probeKey} in ${locale.name}`).toBeTruthy();
      }
    });
    it(`${locale.name}: every layer has a label`, () => {
      for (const layer of layers) {
        const label = pick(locale.t, ["healthCheck", "layer", layer]);
        expect(label, `missing healthCheck.layer.${layer} in ${locale.name}`).toBeTruthy();
      }
    });
  }
});
