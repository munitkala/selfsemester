// Runs before the page draws, so the chosen theme is there from the first frame.
// Mirrors apply() in src/lib/prefs.svelte.ts; keep the two in step.
(function () {
  var p = {};
  try {
    p = JSON.parse(localStorage.getItem("selfsemester.prefs.v1") || "{}") || {};
  } catch (e) {
    p = {};
  }
  var mode = p.mode || "system";
  var dark = mode === "dark" || (mode === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
  var motion = p.motion || "system";
  var reduced = motion === "off" || (motion === "system" && matchMedia("(prefers-reduced-motion: reduce)").matches) || p.effects === "minimal";
  var hues = { blue: 262, teal: 200, green: 150, violet: 295, rose: 355, coral: 30, amber: 70 };
  var root = document.documentElement;
  root.dataset.scheme = dark ? "dark" : "light";
  root.dataset.theme = dark ? "dark-" + (p.darkTheme || "classic") : "light-" + (p.lightTheme || "classic");
  root.dataset.density = p.density || "comfortable";
  root.dataset.corners = p.corners || "rounded";
  root.dataset.width = p.width || "normal";
  root.dataset.effects = p.effects || "balanced";
  root.dataset.motion = reduced ? "reduced" : "full";
  if (hues[p.accent]) root.style.setProperty("--hue", String(hues[p.accent]));
  root.style.setProperty("--text-scale", String((p.textScale || 100) / 100));
})();
