// Building blocks of the gameplay settings form: a labelled row, a select and a
// segmented button group. Choices are matched by value; `null` = "Use default".
import type { ReactNode } from "react";
import type { Choice } from "./gameplay-config-choices";

interface SettingRowProps {
  /** id of the control the label names (select / checkbox / button group). */
  controlId: string;
  label: string;
  help?: string;
  /** Game scope: this game sets its own value. */
  overridden?: boolean;
  children: ReactNode;
}

/** Label (+ "Overridden" badge, one-line help) on the left, control on the right. */
export function SettingRow({ controlId, label, help, overridden, children }: SettingRowProps) {
  return (
    <div className="flex flex-wrap items-center gap-x-6 gap-y-2 border-t border-border py-3.5 first:border-t-0 first:pt-0">
      <div className="flex min-w-48 flex-1 flex-col gap-0.5">
        <span className="flex items-center gap-2">
          <label id={`${controlId}-label`} htmlFor={controlId} className="text-sm font-semibold text-text">
            {label}
          </label>
          {overridden && <span className="rounded-md bg-[#2A2A35] px-2 py-0.5 text-[11px] font-bold text-[#DAD5FF]">Overridden</span>}
        </span>
        {help && <span className="text-xs text-muted">{help}</span>}
      </div>
      {children}
    </div>
  );
}

interface ChoiceControlProps<V> {
  controlId: string;
  choices: Choice<V>[];
  value: V | null;
  onChange: (value: V | null) => void;
}

/** Native select (keyboard + pad friendly); options are indexed so any value type works. */
export function SelectControl<V>({ controlId, choices, value, onChange }: ChoiceControlProps<V>) {
  const selected = choices.findIndex((choice) => choice.value === value);
  return (
    <select
      id={controlId}
      value={selected}
      onChange={(e) => onChange(choices[Number(e.target.value)].value)}
      className="h-10 max-w-80 cursor-pointer truncate rounded-control border border-border-strong bg-surface-raised px-3 font-body text-sm font-semibold text-text outline-none focus:border-accent"
    >
      {choices.map((choice, index) => (
        <option key={choice.label} value={index} className="bg-surface-raised">
          {choice.label}
        </option>
      ))}
    </select>
  );
}

/** Pill buttons (Library filter style); the active one has `aria-pressed`. */
export function SegmentedControl<V>({ controlId, choices, value, onChange }: ChoiceControlProps<V>) {
  return (
    <div id={controlId} role="group" aria-labelledby={`${controlId}-label`} className="flex flex-wrap gap-2">
      {choices.map((choice) => {
        const active = choice.value === value;
        return (
          <button
            key={choice.label}
            type="button"
            aria-pressed={active}
            onClick={() => onChange(choice.value)}
            className={`h-9 cursor-pointer rounded-[18px] border px-3.5 font-body text-sm font-medium ${
              active
                ? "border-text bg-text text-app-bg"
                : "border-border-strong bg-transparent text-text-soft hover:border-[#4A4A55] hover:text-white"
            }`}
          >
            {choice.label}
          </button>
        );
      })}
    </div>
  );
}
