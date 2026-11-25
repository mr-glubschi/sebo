<script lang="ts">
  import { api } from "./lib/api";
  import Footer from "./lib/Footer.svelte";
  import Login from "./lib/Login.svelte";
  import RecoveryCodes from "./lib/RecoveryCodes.svelte";

  type View =
    | { name: "loading" }
    | { name: "offline" }
    | { name: "login"; setupRequired: boolean }
    | { name: "recovery"; codes: string[] }
    | { name: "home" };

  let view = $state<View>({ name: "loading" });

  async function refresh() {
    try {
      const status = await api.status();
      view = status.authenticated
        ? { name: "home" }
        : { name: "login", setupRequired: status.setup_required };
    } catch {
      view = { name: "offline" };
    }
  }

  function loggedIn(codes: string[] | undefined) {
    view = codes ? { name: "recovery", codes } : { name: "home" };
  }

  async function logout() {
    await api.logout().catch(() => {});
    await refresh();
  }

  refresh();
</script>

<div class="page">
  {#if view.name === "loading"}
    <p class="center">Lade …</p>
  {:else if view.name === "offline"}
    <div class="center">
      <p>Server nicht erreichbar.</p>
      <button type="button" onclick={refresh}>Erneut versuchen</button>
    </div>
  {:else if view.name === "login"}
    <Login setupRequired={view.setupRequired} onSuccess={loggedIn} />
  {:else if view.name === "recovery"}
    <RecoveryCodes codes={view.codes} onDone={() => (view = { name: "home" })} />
  {:else}
    <header>
      <strong>SEBO</strong>
      <button type="button" class="secondary" onclick={logout}>Abmelden</button>
    </header>
    <main class="center">
      <p>Angemeldet. Die Bookmarks folgen in den nächsten Phasen.</p>
    </main>
  {/if}

  <Footer />
</div>

<style>
  .page {
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
    padding: 0 1rem;
  }

  .page > :global(footer) {
    margin-top: auto;
  }

  .center {
    margin: 20vh auto 0;
    text-align: center;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 0;
    border-bottom: 1px solid var(--border);
  }

  header strong {
    color: var(--accent);
    font-size: 1.2rem;
  }
</style>
