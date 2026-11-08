<script lang="ts">
  // An editable list of short lines (learning outcomes, unit objectives).
  let { items = $bindable([]), label, placeholder = "", add = "Add" }: { items: string[]; label: string; placeholder?: string; add?: string } = $props();
</script>

<div class="list-editor" role="group" aria-label={label}>
  {#each items as _, i}
    <div class="line">
      <span class="n" aria-hidden="true">{i + 1}.</span>
      <input bind:value={items[i]} {placeholder} aria-label="{label} {i + 1}" />
      <button type="button" class="ghost small" onclick={() => items.splice(i, 1)} aria-label="Remove {label.toLowerCase()} {i + 1}">×</button>
    </div>
  {/each}
  <button type="button" class="ghost small" onclick={() => items.push("")}>{add}</button>
</div>

<style>
  .list-editor {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    align-items: flex-start;
  }
  .line {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    width: 100%;
  }
  .n {
    width: 1.4rem;
    text-align: right;
    color: var(--muted);
    font-size: 0.85rem;
  }
  .line input {
    flex: 1;
  }
</style>
