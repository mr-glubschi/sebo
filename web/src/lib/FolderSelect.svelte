<script lang="ts">
  import { folderOptions, type Tree } from "./library";

  interface Props {
    tree: Tree;
    value: number | null;
    /** Ordner, die nicht gewählt werden dürfen (z. B. der Ordner selbst und seine Unterordner). */
    exclude?: Set<number>;
    /** Beschriftung für „kein Ordner“. */
    noneLabel: string;
    label: string;
  }

  let { tree, value = $bindable(), exclude, noneLabel, label }: Props = $props();

  const options = $derived(folderOptions(tree, exclude));
</script>

<label>
  {label}
  <select
    value={value === null ? "" : String(value)}
    onchange={(event) => {
      const selected = event.currentTarget.value;
      value = selected === "" ? null : Number(selected);
    }}
  >
    <option value="">{noneLabel}</option>
    {#each options as option (option.id)}
      <option value={String(option.id)}>{"   ".repeat(option.depth)}{option.name}</option>
    {/each}
  </select>
</label>
