<script lang="ts">
  let status = $state("prüfe …");

  $effect(() => {
    fetch("/healthz")
      .then((response) => (status = response.ok ? "Server erreichbar" : "Server antwortet nicht"))
      .catch(() => (status = "Server nicht erreichbar"));
  });
</script>

<main>
  <h1>SEBO</h1>
  <p>Self-hosted Bookmarks</p>
  <p class="status">{status}</p>
</main>

<style>
  main {
    max-width: 40rem;
    margin: 0 auto;
    padding: 3rem 1rem;
  }

  h1 {
    margin: 0;
    color: var(--accent);
  }

  .status {
    color: var(--muted);
  }
</style>
