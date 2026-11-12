<script lang="ts">
  // Line icons drawn for SelfSemester on a 24 px grid, in the current text colour.
  let { name, size = 18 }: { name: string; size?: number } = $props();

  const paths: Record<string, string> = {
    today: "M4 7h16M4 7v12a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1V7M4 7V5a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v2M8 2v4M16 2v4M8 12h3v3H8z",
    courses: "M4 5a2 2 0 0 1 2-2h13v15H6a2 2 0 0 0-2 2zM4 20a2 2 0 0 0 2 1h13v-3M9 8h6",
    new: "M12 5v14M5 12h14",
    builder: "M4 20h4L19 9l-4-4L4 16zM13 7l4 4",
    insights: "M4 20V10M10 20V4M16 20v-7M22 20H2",
    transcript: "M12 3a5 5 0 1 1 0 10a5 5 0 1 1 0 -10zM9 12.5 8 21l4-2.2 4 2.2-1-8.5",
    settings: "M12 9a3 3 0 1 1 0 6a3 3 0 1 1 0 -6zM12 5a7 7 0 1 1 0 14a7 7 0 1 1 0 -14zM19.00 12.00L21.50 12.00M16.95 16.95L18.72 18.72M12.00 19.00L12.00 21.50M7.05 16.95L5.28 18.72M5.00 12.00L2.50 12.00M7.05 7.05L5.28 5.28M12.00 5.00L12.00 2.50M16.95 7.05L18.72 5.28",
    search: "M10.5 4.0a6.5 6.5 0 1 1 0 13.0a6.5 6.5 0 1 1 0 -13.0zM15.3 15.3 20.5 20.5",
    collapse: "M15 18l-6-6 6-6",
    expand: "M9 18l6-6-6-6",
    flame: "M12 22c4 0 7-3 7-7 0-3-2-5.5-3.5-7-.4 2-1.5 3-2.5 3 0-3-1.5-6-4-8 0 3.5-4 6.5-4 12 0 4 3 7 7 7z",
    clock: "M12 3a9 9 0 1 1 0 18a9 9 0 1 1 0 -18zM12 7.5V12l3 2",
    check: "M5 12.5l4.5 4.5L19 7.5",
    target: "M12 3a9 9 0 1 1 0 18a9 9 0 1 1 0 -18zM12 6.5a5.5 5.5 0 1 1 0 11.0a5.5 5.5 0 1 1 0 -11.0zM12 10a2 2 0 1 1 0 4a2 2 0 1 1 0 -4z",
    sun: "M12 8a4 4 0 1 1 0 8a4 4 0 1 1 0 -8zM18.50 12.00L21.00 12.00M16.60 16.60L18.36 18.36M12.00 18.50L12.00 21.00M7.40 16.60L5.64 18.36M5.50 12.00L3.00 12.00M7.40 7.40L5.64 5.64M12.00 5.50L12.00 3.00M16.60 7.40L18.36 5.64",
    moon: "M15 3.5a8.5 8.5 0 1 0 5.5 12.5a6.5 6.5 0 0 1-5.5-12.5z",
    calendar: "M4 6.5h16v13H4zM4 10.5h16M8.5 4v4M15.5 4v4",
    keyboard: "M2.5 7h19v10h-19zM6 10.5h1M9.5 10.5h1M13 10.5h1M16.5 10.5h1M7 14h10",
    sparkle: "M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8zM19 17l.7 1.8L21.5 19.5l-1.8.7L19 22l-.7-1.8-1.8-.7 1.8-.7z",
  };
</script>

<svg
  class="icon"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  <path d={paths[name] ?? ""} />
</svg>

<style>
  .icon {
    flex: none;
    display: block;
  }
</style>
