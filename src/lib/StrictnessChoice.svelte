<script lang="ts">
  import type { Strictness } from "#lib/api.ts";

  let { value = $bindable("structured") }: { value: Strictness } = $props();
</script>

<fieldset class="field">
  <legend>How strict</legend>
  <label class="radio">
    <input type="radio" bind:group={value} value="structured" />
    <span><strong>Structured</strong>: one attempt per assessment, marked strictly, and the late policy above applies. Like a real course.</span>
  </label>
  <label class="radio">
    <input type="radio" bind:group={value} value="sandbox" />
    <span><strong>Sandbox</strong>: resubmit as often as you like, with no late penalties. The answer key stays hidden until you choose to see it, which closes that item. Late work is taken until a week after the last deadline.</span>
  </label>
  <p class="small muted">The mode can't be changed once the course starts. Sandbox courses are marked as such on your transcript.</p>
</fieldset>

<style>
  fieldset {
    border: 0;
    padding: 0;
  }
  legend {
    font-weight: 600;
    margin-bottom: 0.3rem;
  }
  .radio {
    font-weight: 400;
    display: flex;
    gap: 0.5rem;
    align-items: baseline;
    margin: 0.25rem 0;
  }
  .radio input {
    width: auto;
  }
</style>
