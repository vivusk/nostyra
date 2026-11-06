// Nostyra web player, iframe side: the keyboard and the hotkeys (plain script, no
// inline code: CSP). The keyboard is read here by physical key (`KeyboardEvent.code`,
// the names the shared config stores) so every bound key works as on the desktop;
// pad game buttons and sticks go through player-pads.js from the same poll; the
// hotkeys run here for keyboard and pads (src/lib/web-input-map.ts builds the tables).
// Fixed inputs: Esc / Guide tap = pause or resume, held 1 s = exit the game;
// Select + Start held 1 s = pause or resume.
(function () {
  "use strict";
  const HOLD_MS = 1000;
  const FAST_FORWARD_ID = 27;
  const GUIDE = 16;
  const CHORD = [8, 9]; // back + start
  const PRESSED_VALUE = 0.5;

  // `input`: PlayerInput; `actions`: {simulate(player, id, value), togglePause(), save(), load(), exit()}.
  function install(input, actions) {
    let exiting = false;
    const exit = () => {
      if (exiting) return;
      exiting = true;
      actions.exit();
    };
    // Analog stick directions take a full deflection.
    const pressValue = (id) => (id >= 16 && id <= 23 ? 0x7fff : 1);
    const hotkey = (name, down) => {
      if (exiting) return;
      if (name === "fast_forward_hold") return actions.simulate(0, FAST_FORWARD_ID, down ? 1 : 0);
      if (!down) return;
      if (name === "pause_menu") actions.togglePause();
      else if (name === "save_state") actions.save();
      else if (name === "load_state") actions.load();
    };
    // Tap = pause / resume, hold = exit (Esc, Guide).
    const holdToExit = () => {
      let timer = null;
      return (down) => {
        if (down && timer === null) {
          timer = setTimeout(() => {
            timer = null;
            exit();
          }, HOLD_MS);
        } else if (!down && timer !== null) {
          clearTimeout(timer);
          timer = null;
          if (!exiting) actions.togglePause();
        }
      };
    };

    const keyHotkeys = {};
    for (const [name, codes] of Object.entries(input.hotkeys.keyboard)) for (const code of codes) keyHotkeys[code] = name;
    const escape = holdToExit();
    const onKey = (event) => {
      const down = event.type === "keydown";
      const ids = input.keys[event.code];
      const name = keyHotkeys[event.code];
      if (event.code !== "Escape" && !ids && !name) return;
      event.preventDefault();
      event.stopPropagation(); // EmulatorJS's own key handling stays out (no keyboard binds there)
      if (event.repeat) return;
      if (event.code === "Escape") return escape(down);
      for (const id of ids || []) actions.simulate(0, id, down ? pressValue(id) : 0);
      if (name) hotkey(name, down);
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("keyup", onKey, true);

    const padHotkeys = {};
    for (const [name, indices] of Object.entries(input.hotkeys.gamepad)) for (const index of indices) padHotkeys[index] = name;
    const guide = holdToExit();
    const previous = new Map();
    let chordTimer = null;
    const onButton = (index, down, now) => {
      if (index === GUIDE) guide(down);
      if (padHotkeys[index]) hotkey(padHotkeys[index], down);
      if (!CHORD.includes(index)) return;
      const chordHeld = CHORD.every((button) => now[button]);
      if (chordHeld && chordTimer === null) {
        chordTimer = setTimeout(() => {
          chordTimer = null;
          if (!exiting) actions.togglePause();
        }, HOLD_MS);
      } else if (!chordHeld && chordTimer !== null) {
        clearTimeout(chordTimer);
        chordTimer = null;
      }
    };
    const updatePads = window.NostyraPlayerPads.create(input, actions.simulate);
    const poll = () => {
      const pads = Array.from(navigator.getGamepads()).filter((pad) => pad && pad.mapping === "standard");
      if (!exiting) updatePads(pads);
      for (const pad of pads) {
        const before = previous.get(pad.index) || [];
        const now = pad.buttons.map((button) => button.pressed || button.value > PRESSED_VALUE);
        now.forEach((down, index) => down !== Boolean(before[index]) && onButton(index, down, now));
        previous.set(pad.index, now);
      }
      requestAnimationFrame(poll);
    };
    requestAnimationFrame(poll);
  }

  window.NostyraPlayerInput = { install };
})();
