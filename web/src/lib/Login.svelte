<script lang="ts">
  import { api, ApiError } from "./api";

  interface Props {
    setupRequired: boolean;
    onSuccess: (recoveryCodes: string[] | undefined) => void;
  }

  let { setupRequired, onSuccess }: Props = $props();

  let password = $state("");
  let code = $state("");
  let error = $state("");
  let busy = $state(false);

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    busy = true;
    error = "";
    try {
      const result = await api.login(password, code);
      password = "";
      code = "";
      onSuccess(result.recovery_codes);
    } catch (e) {
      error = e instanceof ApiError ? e.message : "Server nicht erreichbar";
      code = "";
    } finally {
      busy = false;
    }
  }
</script>

<main class="card">
  <h1>SEBO</h1>

  {#if setupRequired}
    <div class="hint">
      <strong>Zwei-Faktor-Einrichtung</strong>
      <p>
        Öffne das Log des Containers und scanne den QR-Code mit einer Authenticator-App. Melde
        dich dann mit deinem Passwort und dem Code aus der App an.
      </p>
    </div>
  {/if}

  <form onsubmit={submit}>
    <label>
      Passwort
      <input type="password" bind:value={password} autocomplete="current-password" required />
    </label>

    <label>
      {setupRequired ? "Code aus der App" : "Code aus der App oder Wiederherstellungscode"}
      <input
        type="text"
        bind:value={code}
        autocomplete="one-time-code"
        inputmode={setupRequired ? "numeric" : "text"}
        autocapitalize="characters"
        spellcheck="false"
        required
      />
    </label>

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    <button type="submit" disabled={busy}>
      {busy ? "Anmelden …" : setupRequired ? "Einrichten und anmelden" : "Anmelden"}
    </button>
  </form>
</main>

<style>
  .card {
    width: min(100%, 24rem);
    margin: 10vh auto 0;
    padding: 2rem 1.5rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  h1 {
    margin: 0 0 1.5rem;
    color: var(--accent);
    text-align: center;
  }

  .hint {
    margin-bottom: 1.5rem;
    padding: 0.75rem 1rem;
    border-left: 3px solid var(--accent);
    background: var(--bg);
    font-size: 0.9rem;
  }

  .hint p {
    margin: 0.25rem 0 0;
  }

  form {
    display: grid;
    gap: 1rem;
  }

  label {
    display: grid;
    gap: 0.35rem;
    font-size: 0.9rem;
  }

  .error {
    margin: 0;
    color: var(--danger);
  }
</style>
