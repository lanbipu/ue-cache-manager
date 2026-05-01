import { config } from "@vue/test-utils";

// Stub <Teleport> so its content renders inline within the wrapper.
// This lets wrapper.find() locate modal elements that would otherwise
// be teleported to document.body. Production behavior is unaffected.
config.global.stubs = {
  ...(config.global.stubs ?? {}),
  teleport: true,
};
