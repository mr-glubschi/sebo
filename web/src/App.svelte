<script lang="ts">
  import { api, onUnauthorized } from "./lib/api";
  import Footer from "./lib/Footer.svelte";
  import Home from "./lib/Home.svelte";
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

  // Sitzung abgelaufen: zurück zur Anmeldung.
  onUnauthorized(() => {
    if (view.name === "home") refresh();
  });

  refresh();
</script>

<div class="page" class:home={view.name === "home"}>
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
    <Home onlogout={logout} />
  {/if}

  {#if view.name !== "home"}
    <Footer />
  {/if}
</div>

<style>
  .page {
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
    padding: 0 1rem;
  }

  .page.home {
    display: block;
    padding: 0;
  }

  .page > :global(footer) {
    margin-top: auto;
  }

  .center {
    margin: 20vh auto 0;
    text-align: center;
  }
</style>
