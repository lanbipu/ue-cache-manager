import { config } from "@vue/test-utils";
import { i18n } from "@/locales";

// Stub <Teleport> so its content renders inline within the wrapper.
// This lets wrapper.find() locate modal elements that would otherwise
// be teleported to document.body. Production behavior is unaffected.
// DialogPortal is reka-ui's portal wrapper — stub it the same way.
config.global.stubs = {
  ...(config.global.stubs ?? {}),
  teleport: true,
  DialogPortal: { template: "<div data-stub-portal><slot /></div>" },
};

// Inject i18n into all mount() calls so `t()` resolves correctly.
// Tests assert against English strings, so force locale=en regardless of env.
(i18n.global.locale as unknown as { value: string }).value = "en";
config.global.plugins = [...(config.global.plugins ?? []), i18n];
