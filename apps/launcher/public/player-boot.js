// Nostyra web player, iframe side (player.html; plain script, no inline code: CSP).
// The launcher page (src/backend/web/player/player-frame.ts) owns every file: it
// posts `boot` with the content File, BIOS and SRAM bytes; this page sets the
// EmulatorJS 4.2.3 globals, injects the saves into EmulatorJS's file system before
// the game starts, and relays SRAM, save-state and exit events back. Messages:
// parent → player: boot, flush, exit {autoState}, load-state {state}, message {text},
//                  pause, resume, key {code, down, repeat}
// player → parent: ready, started {saveFilePath}, sram {sram, rtc}, save-state-request
//                  {state, screenshot}, load-state-request, auto-state {state,
//                  screenshot}, error {message, fatal}, exited, exit-request
// Bindings: keyboard, pads and hotkeys read by player-input.js / player-pads.js.
(function () {
  "use strict";
  // EmulatorJS's own Save / Load buttons (saveState / loadState events) open
  // Nostyra's slot picker in the launcher; its export/import and quick-slot
  // buttons are hidden, as are netplay, cheats, core settings (phase 00 D6) and the
  // control remap menu (bindings come from Nostyra's shared config).
  const SAVES_DIR = "/data/saves";
  const SCREENSHOT_TIMEOUT_MS = 3000;
  let emu = null;
  let started = false;
  let exiting = false;
  let keyboard = null; // player-input.js, once started

  const post = (type, data) => parent.postMessage({ from: "player", type, data: data || {} }, location.origin);
  // `fatal`: the game cannot start (EmulatorJS's own start error, or the browser ran
  // out of memory for a big disc); other script errors are only reported.
  const report = (message, fatal) => post("error", { message: String(message), fatal });
  const fail = (message) => report(message, true);
  const scriptError = (message) => report(message, /memory|allocation/i.test(String(message)));

  window.addEventListener("error", (event) => scriptError(event.message || event.error));
  window.addEventListener("unhandledrejection", (event) => scriptError(event.reason && (event.reason.message || event.reason)));

  // EmulatorJS's class: report start failures, skip its update check (phase 00 D11).
  let emulatorClass;
  Object.defineProperty(window, "EmulatorJS", {
    configurable: true,
    get: () => emulatorClass,
    set: (value) => {
      emulatorClass = value;
      const startGameError = value.prototype.startGameError;
      value.prototype.startGameError = function (message) {
        fail(message);
        return startGameError.call(this, message);
      };
      value.prototype.checkForUpdates = function () {};
    },
  });

  function wipe(FS, dir) {
    for (const name of FS.readdir(dir)) {
      if (name === "." || name === "..") continue;
      const path = dir + "/" + name;
      if (FS.isDir(FS.stat(path).mode)) {
        wipe(FS, path);
        FS.rmdir(path);
      } else {
        FS.unlink(path);
      }
    }
  }

  function writeAll(FS, root, files) {
    for (const [name, bytes] of Object.entries(files || {})) {
      let dir = root;
      for (const part of name.split("/").slice(0, -1)) {
        dir += "/" + part;
        if (!FS.analyzePath(dir).exists) FS.mkdir(dir);
      }
      FS.writeFile(root + "/" + name, bytes);
    }
  }

  function readRtc() {
    const FS = emu.gameManager.FS;
    const path = emu.gameManager.getSaveFilePath().replace(/\.srm$/, ".rtc");
    return FS.analyzePath(path).exists ? FS.readFile(path) : null;
  }

  function screenshot() {
    const timeout = new Promise((resolve) => setTimeout(() => resolve(null), SCREENSHOT_TIMEOUT_MS));
    return Promise.race([emu.gameManager.screenshot(), timeout]).catch(() => null);
  }

  const requestSave = async (state) => post("save-state-request", { state, screenshot: await screenshot() });

  function togglePause() {
    if (emu.paused) return emu.play();
    emu.pause();
    emu.displayMessage("Paused: press again to resume, hold Esc to exit", 4000);
  }

  function attach(boot, instance) {
    emu = instance;
    // IndexedDB keeps /data/saves across games (phase 00 D5): wipe it, then write
    // Nostyra's files, so the Data folder is the only source of truth.
    instance.on("saveDatabaseLoaded", (FS) => {
      wipe(FS, SAVES_DIR);
      writeAll(FS, SAVES_DIR, boot.saves);
      writeAll(FS, "", boot.bios); // content dir = RetroArch's system dir here
    });
    instance.on("start", () => {
      started = true;
      post("started", { saveFilePath: instance.gameManager.getSaveFilePath() });
      if (boot.loadState) setTimeout(() => instance.gameManager.loadState(boot.loadState), 10);
      if (boot.notice) instance.displayMessage(boot.notice, 5000);
      keyboard = window.NostyraPlayerInput.install(boot.input, {
        simulate: (player, id, value) => instance.gameManager.simulateInput(player, id, value),
        togglePause,
        save: () => requestSave(instance.gameManager.getState()),
        load: () => post("load-state-request"),
        exit: () => post("exit-request"),
      });
    });
    instance.on("saveSaveFiles", (bytes) => {
      if (started) post("sram", { sram: bytes || null, rtc: readRtc() });
    });
    instance.on("exit", () => {
      if (!exiting) setTimeout(() => post("exited"), 0); // EmulatorJS ended it (start failure)
    });
  }

  const handlers = {
    boot({ boot }) {
      window.EJS_player = "#game";
      window.EJS_core = boot.core;
      window.EJS_gameUrl = boot.rom;
      window.EJS_pathtodata = boot.pathToData;
      window.EJS_startOnLoaded = true;
      window.EJS_disableDatabases = true;
      window.EJS_disableLocalStorage = true;
      window.EJS_language = "en-US";
      window.EJS_disableAutoLang = false;
      window.EJS_defaultControls = { 0: {}, 1: {}, 2: {}, 3: {} }; // pads + keys: player-*.js
      window.EJS_Buttons = {
        netplay: false,
        cheat: false,
        saveSavFiles: false,
        loadSavFiles: false,
        quickSave: false,
        quickLoad: false,
        cacheManager: false,
        exitEmulation: false,
        settings: false,
        gamepad: false,
      };
      window.EJS_onSaveState = (event) => requestSave(event.state);
      window.EJS_onLoadState = () => post("load-state-request");
      let instance;
      Object.defineProperty(window, "EJS_emulator", {
        configurable: true,
        get: () => instance,
        set: (value) => {
          instance = value;
          attach(boot, value);
        },
      });
      const loader = document.createElement("script");
      loader.src = boot.pathToData + "loader.js";
      document.body.appendChild(loader);
    },
    flush() {
      if (started) emu.gameManager.saveSaveFiles();
    },
    async exit({ autoState }) {
      if (exiting) return;
      exiting = true;
      if (!started) return post("exited");
      if (autoState) {
        try {
          const state = emu.gameManager.getState();
          post("auto-state", { state, screenshot: await screenshot() });
        } catch (error) {
          console.warn("resume point not saved", error);
        }
      }
      emu.callEvent("exit"); // EmulatorJS flushes SRAM here (saveSaveFiles → "sram")
      setTimeout(() => post("exited"), 0);
    },
    "load-state"({ state }) {
      if (started) emu.gameManager.loadState(state);
    },
    message({ text }) {
      if (started) emu.displayMessage(text, 3000);
    },
    pause() {
      if (started) emu.pause();
    },
    resume() {
      if (started) emu.play();
    },
    key({ code, down, repeat }) {
      if (keyboard && !exiting) keyboard.forwardedKey(code, down, repeat);
    },
  };

  window.addEventListener("message", (event) => {
    if (event.origin !== location.origin || event.source !== parent) return;
    const handler = handlers[event.data && event.data.type];
    if (handler) Promise.resolve(handler(event.data)).catch(scriptError);
  });
  post("ready");
})();
