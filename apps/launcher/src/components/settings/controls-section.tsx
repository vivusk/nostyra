import { ControlsEditor } from "../controls/controls-editor";

/** Settings → Controls: the global keyboard / gamepad bindings and shortcuts
 *  (config/global.toml), used by every game without its own controls. */
export function ControlsSection() {
  return (
    <>
      <p className="m-0 text-xs text-muted">
        Used by every game without its own controls (Game → Settings). Keyboard controls player 1; the gamepad layout applies
        to every pad.
      </p>
      <ControlsEditor scope="global" />
    </>
  );
}
