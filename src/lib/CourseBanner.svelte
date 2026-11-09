<script lang="ts">
  import type { CourseMeta } from "#lib/api.ts";
  import { codeOf, colorVars, instructorOf } from "#lib/course.ts";
  import type { Snippet } from "svelte";

  let {
    id,
    title,
    meta,
    origin,
    subtitle = "",
    compact = false,
    aside,
  }: { id: string; title: string; meta: CourseMeta | null; origin: string; subtitle?: string; compact?: boolean; aside?: Snippet } = $props();
</script>

<header class="banner" class:compact style={colorVars(meta, id)}>
  <div class="text">
    <span class="code">{codeOf(meta, title)}{#if meta?.term}{" "}· {meta.term}{/if}</span>
    {#if compact}
      <strong class="title">{title || "Untitled course"}</strong>
    {:else}
      <h1 class="title">{title || "Untitled course"}</h1>
    {/if}
    <span class="who">Instructor: {instructorOf(meta, origin)}{#if subtitle}{" "}· {subtitle}{/if}</span>
  </div>
  {#if aside}<div class="aside">{@render aside()}</div>{/if}
</header>

<style>
  .banner {
    --course: #3b4cb8;
    position: relative;
    overflow: hidden;
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 1.5rem;
    flex-wrap: nowrap;
    padding: 1.6rem 1.6rem 1.3rem;
    border-radius: var(--radius);
    color: #fff;
    background:
      radial-gradient(circle at 92% -20%, rgb(255 255 255 / 0.22) 0 7rem, transparent 7.05rem),
      radial-gradient(circle at 78% 130%, rgb(255 255 255 / 0.12) 0 6rem, transparent 6.05rem),
      repeating-linear-gradient(135deg, rgb(255 255 255 / 0.05) 0 2px, transparent 2px 14px),
      var(--course);
    margin-bottom: 1rem;
  }
  .compact {
    padding: 0.9rem 1rem 0.8rem;
    margin: 0;
    border-radius: var(--radius) var(--radius) 0 0;
    min-height: 6.2rem;
  }
  @media (max-width: 640px) {
    .banner {
      flex-wrap: wrap;
    }
  }
  .text {
    flex: 1 1 auto;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    min-width: 0;
  }
  .code {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 0.8rem;
    letter-spacing: 0.06em;
    opacity: 0.9;
    text-transform: uppercase;
  }
  .title {
    margin: 0;
    font-size: 1.9rem;
    line-height: 1.15;
    color: #fff;
  }
  .compact .title {
    font-size: 1.05rem;
    line-height: 1.25;
  }
  .who {
    font-size: 0.88rem;
    opacity: 0.92;
  }
  .compact .who {
    font-size: 0.8rem;
  }
  .aside {
    flex: none;
  }
</style>
