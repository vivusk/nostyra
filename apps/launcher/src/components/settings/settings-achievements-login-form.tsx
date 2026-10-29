import { useState, type FormEvent } from "react";
import { errorMessage } from "../../api/tauri-commands";
import { useRaLogin } from "../../hooks/use-achievements-queries";

const INPUT =
  "h-10 min-w-0 flex-1 rounded-control border border-border-strong bg-surface-raised px-3 font-body text-sm text-text outline-none focus:border-accent";

/** Username + password → `ra_login`. The password lives only in this form's state and
 *  is cleared after every attempt; the backend stores the token, never the password. */
export function SettingsAchievementsLoginForm() {
  const login = useRaLogin();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!username.trim() || !password || login.isPending) return;
    login.mutate({ username: username.trim(), password }, { onSettled: () => setPassword("") });
  };

  return (
    <form onSubmit={submit} className="flex flex-col gap-3" aria-label="RetroAchievements login">
      <div className="flex flex-wrap gap-3">
        <input
          type="text"
          autoComplete="username"
          placeholder="Username"
          aria-label="RetroAchievements username"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          className={INPUT}
        />
        <input
          type="password"
          autoComplete="current-password"
          placeholder="Password"
          aria-label="RetroAchievements password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          className={INPUT}
        />
        <button
          type="submit"
          disabled={!username.trim() || !password || login.isPending}
          className="h-10 cursor-pointer rounded-control border-0 bg-accent-strong px-5 font-body text-sm font-bold text-white hover:bg-accent disabled:cursor-default disabled:opacity-50"
        >
          {login.isPending ? "Logging in…" : "Log in"}
        </button>
      </div>
      {login.isError && (
        <p role="alert" className="m-0 text-sm text-danger">
          {errorMessage(login.error)}
        </p>
      )}
      <p className="m-0 text-xs text-muted">
        No account? Create one for free at retroachievements.org.
      </p>
    </form>
  );
}
