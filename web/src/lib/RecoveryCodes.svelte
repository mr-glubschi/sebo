<script lang="ts">
  interface Props {
    codes: string[];
    onDone: () => void;
  }

  let { codes, onDone }: Props = $props();

  let saved = $state(false);
  let copied = $state(false);

  const text = $derived(
    `SEBO Wiederherstellungscodes\nJeder Code funktioniert genau einmal.\n\n${codes.join("\n")}\n`,
  );

  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
    } catch {
      copied = false;
    }
  }

  function download() {
    const url = URL.createObjectURL(new Blob([text], { type: "text/plain" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = "sebo-wiederherstellungscodes.txt";
    link.click();
    URL.revokeObjectURL(url);
  }
</script>

<main class="card">
  <h1>Wiederherstellungscodes</h1>
  <p>
    Die Zwei-Faktor-Authentifizierung ist eingerichtet. Bewahre diese Codes sicher auf. Mit ihnen
    kommst du rein, falls du keinen Zugriff auf die Authenticator-App hast. Jeder Code funktioniert
    genau einmal. <strong>Sie werden nur jetzt angezeigt.</strong>
  </p>

  <ol>
    {#each codes as code (code)}
      <li><code>{code}</code></li>
    {/each}
  </ol>

  <div class="actions">
    <button type="button" class="secondary" onclick={copy}>{copied ? "Kopiert" : "Kopieren"}</button>
    <button type="button" class="secondary" onclick={download}>Als Datei speichern</button>
  </div>

  <label class="confirm">
    <input type="checkbox" bind:checked={saved} />
    Ich habe die Codes gesichert.
  </label>

  <button type="button" disabled={!saved} onclick={onDone}>Weiter</button>
</main>

<style>
  .card {
    width: min(100%, 28rem);
    margin: 6vh auto 0;
    padding: 2rem 1.5rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    display: grid;
    gap: 1rem;
  }

  h1 {
    margin: 0;
    font-size: 1.4rem;
  }

  p {
    margin: 0;
  }

  ol {
    margin: 0;
    padding-left: 2rem;
    columns: 2;
    column-gap: 2rem;
  }

  li {
    padding: 0.2rem 0;
  }

  code {
    font-size: 1rem;
    letter-spacing: 0.03em;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .confirm {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  @media (max-width: 26rem) {
    ol {
      columns: 1;
    }
  }
</style>
