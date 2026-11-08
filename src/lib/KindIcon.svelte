<script lang="ts">
  // Small line icons for each kind of course item, drawn for SelfSemester.
  let { kind, size = 18 }: { kind: string; size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class="kind-icon">
  {#if kind === "lesson"}
    <!-- open book -->
    <path d="M10 5.5C8.5 4.3 6.4 4 3.5 4.2v10.6c2.9-.2 5 .1 6.5 1.3 1.5-1.2 3.6-1.5 6.5-1.3V4.2c-2.9-.2-5 .1-6.5 1.3z" />
    <path d="M10 5.5v10.6" />
  {:else if kind === "quiz"}
    <!-- ticked boxes -->
    <rect x="3.5" y="3.5" width="5" height="5" rx="1" />
    <rect x="3.5" y="11.5" width="5" height="5" rx="1" />
    <path d="M4.8 6l1 1 1.7-1.9M11 6h5.5M11 14h5.5" />
  {:else if kind === "test" || kind === "exam"}
    <!-- sheet with a seal -->
    <path d="M5 2.8h7l3 3v11.4H5z" />
    <path d="M12 2.8v3h3" />
    <circle cx="10" cy="11.5" r="2.4" />
    {#if kind === "exam"}<path d="M8.8 13.6l-.8 2.9 2-1 2 1-.8-2.9" />{/if}
  {:else if kind === "project"}
    <!-- stacked layers -->
    <path d="M10 3l7 3.5-7 3.5-7-3.5z" />
    <path d="M3 10l7 3.5 7-3.5M3 13.5l7 3.5 7-3.5" />
  {:else}
    <!-- pencil on a line: homework, assignments -->
    <path d="M12.8 3.7l3.5 3.5-8.4 8.4-4 .9.9-4z" />
    <path d="M11.2 5.3l3.5 3.5M3.5 17.5h13" />
  {/if}
</svg>

<style>
  .kind-icon {
    flex: none;
    vertical-align: -0.2em;
  }
</style>
