// Nostyra web player, iframe side: game buttons and sticks of every pad (plain
// script, no inline code: CSP). Pads with the Gamepad API "standard" layout feed
// players 1–4 in browser index order; each bound button presses its EmulatorJS
// control ids (several buttons per control allowed, like the desktop). Sticks drive
// the analog controls when the system has them; without a left stick, it acts as
// the D-pad (desktop rule). EmulatorJS's own pad handling is off.
(function () {
  "use strict";
  const PLAYERS = 4;
  const PRESSED_VALUE = 0.5;
  const DEADZONE = 0.15;
  const DPAD = { up: 4, down: 5, left: 6, right: 7 };
  // Analog control ids per stick: [x+, x-, y+, y-] (EmulatorJS 16–23).
  const STICK_IDS = { left: [16, 17, 18, 19], right: [20, 21, 22, 23] };
  const FULL = 0x7fff;

  // EmulatorJS 4.2.3's GamepadHandler indexes its pad list by Gamepad.index, so a
  // pad that is not browser index 0 (common on Windows: DualSense) throws in its
  // "connected" handler and never plays. Its polling loop is switched off when
  // emulator.min.js registers the class; this file reads the pads instead.
  let handlerClass;
  Object.defineProperty(window, "GamepadHandler", {
    configurable: true,
    get: () => handlerClass,
    set: (value) => {
      handlerClass = value;
      if (value && value.prototype) value.prototype.loop = function () {};
    },
  });

  // `input`: PlayerInput (padButtons, sticks); `simulate(player, id, value)`.
  function create(input, simulate) {
    // Per player: control id → last value sent (only changes are sent).
    const sent = Array.from({ length: PLAYERS }, () => new Map());
    const send = (player, id, value) => {
      if (sent[player].get(id) === value) return;
      sent[player].set(id, value);
      simulate(player, id, value);
    };
    const axis = (value) => (Math.abs(value) < DEADZONE ? 0 : value);

    function updatePad(player, pad) {
      const values = new Map();
      const add = (id, value) => values.set(id, Math.max(values.get(id) || 0, value));
      pad.buttons.forEach((button, index) => {
        const down = button.pressed || button.value > PRESSED_VALUE;
        for (const id of input.padButtons[index] || []) add(id, down ? 1 : 0);
      });
      const [lx = 0, ly = 0, rx = 0, ry = 0] = pad.axes.map(axis);
      if (input.sticks.left) stick(STICK_IDS.left, lx, ly, add);
      else {
        // Left stick as the D-pad: full deflection past half way.
        add(DPAD.right, lx > PRESSED_VALUE ? 1 : 0);
        add(DPAD.left, lx < -PRESSED_VALUE ? 1 : 0);
        add(DPAD.down, ly > PRESSED_VALUE ? 1 : 0);
        add(DPAD.up, ly < -PRESSED_VALUE ? 1 : 0);
      }
      if (input.sticks.right) stick(STICK_IDS.right, rx, ry, add);
      for (const [id, value] of values) send(player, id, value);
    }

    function stick([xPlus, xMinus, yPlus, yMinus], x, y, add) {
      add(xPlus, x > 0 ? Math.round(FULL * x) : 0);
      add(xMinus, x < 0 ? Math.round(-FULL * x) : 0);
      add(yPlus, y > 0 ? Math.round(FULL * y) : 0);
      add(yMinus, y < 0 ? Math.round(-FULL * y) : 0);
    }

    // Called every frame with the connected standard-layout pads, index order.
    return function update(pads) {
      pads.slice(0, PLAYERS).forEach((pad, player) => updatePad(player, pad));
      // A player whose pad went away releases everything it held.
      for (let player = pads.length; player < PLAYERS; player++) {
        for (const [id, value] of sent[player]) if (value !== 0) send(player, id, 0);
      }
    };
  }

  window.NostyraPlayerPads = { create };
})();
