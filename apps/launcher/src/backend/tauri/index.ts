// Desktop backend: the Tauri APIs the launcher always used, unchanged behaviour.
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { Backend } from "../backend-contract";

const backend: Backend = {
  edition: "desktop",
  boot: async () => "ready",
  capabilities: {
    coreDownload: true,
    achievements: true,
    shaders: true,
    videoScaling: true,
    runAhead: true,
    rewind: true,
    logs: true,
    revealInFolder: true,
    memoryCards: true,
    gamepadCaptureNative: true,
    dataFolderMove: true,
    updateCheck: true,
    cloudSaves: true,
    netplay: true,
  },
  invoke: (command, args) => invoke(command, args),
  listen: (event, handler) => listen(event, handler),
  open: (options) => open(options),
  // Cached files are backend-provided absolute paths (never user input), served by
  // Tauri's asset protocol; `version` busts the WebView cache on rewrite.
  assetUrl: (ref, version) => {
    const url = convertFileSrc(ref);
    return version ? `${url}?v=${version}` : url;
  },
  resolveAsset: async (src) => src,
};

export default backend;
