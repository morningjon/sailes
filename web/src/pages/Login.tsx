import { useState, type FormEvent } from "react";
import { post, type User } from "../lib/api";
import { Brand } from "../components/Layout";

export function LoginPage({ needsSetup, onSignedIn }: { needsSetup: boolean; onSignedIn: (u: User) => void }) {
  const [email, setEmail] = useState("");
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const user = needsSetup
        ? await post<User>("/auth/setup", { email, name, password })
        : await post<User>("/auth/login", { email, password });
      onSignedIn(user);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="auth">
      <div className="auth-art">
        <Brand />
        <blockquote>Your CRM fills itself in from email, Slack and Teams. You just approve.</blockquote>
        <div className="foot">Self-hosted and open source. Your data stays on your server.</div>
      </div>
      <div className="auth-form">
        <form onSubmit={submit}>
          <h1>{needsSetup ? "Create your account" : "Sign in"}</h1>
          <p>{needsSetup ? "This is the first account on this server. You'll be its admin." : "Welcome back."}</p>
          {error && <div className="error-box">{error}</div>}
          {needsSetup && (
            <div className="field">
              <label htmlFor="name">Your name</label>
              <input id="name" className="input" autoComplete="name" value={name} onChange={(e) => setName(e.target.value)} required />
            </div>
          )}
          <div className="field">
            <label htmlFor="email">Email</label>
            <input
              id="email"
              className="input"
              type="email"
              autoComplete="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
              autoFocus={!needsSetup}
            />
          </div>
          <div className="field">
            <label htmlFor="password">Password</label>
            <input
              id="password"
              className="input"
              type="password"
              autoComplete={needsSetup ? "new-password" : "current-password"}
              minLength={needsSetup ? 10 : undefined}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
            />
            {needsSetup && <span className="hint">At least 10 characters.</span>}
          </div>
          <button className="btn primary" disabled={busy}>
            {busy ? "One moment…" : needsSetup ? "Create account" : "Sign in"}
          </button>
        </form>
      </div>
    </div>
  );
}
