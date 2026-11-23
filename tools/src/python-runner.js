// Python playground for interactive figures: a code box, a Run button and the output, running real Python
// (Brython) inside the widget's sealed frame. Loaded after brython.min.js and brython_stdlib.js.
//
//   PyRun.mount(element, code, {input: "lines for input()", height: 12, readOnly: false, check: fn})
//
// `check(output)` is optional: return a message to show under the output (for example "Correct!").
(function () {
  "use strict";
  let ready = false;
  function start() {
    if (!ready) {
      // No cache and no remote imports: everything comes from the standard library bundle.
      window.brython({ debug: 1, cache: false, pythonpath: [] });
      ready = true;
    }
  }

  const RUNNER = [
    "import sys, traceback",
    "from browser import window",
    "class _Out:",
    "    def __init__(self, err): self.err = err",
    "    def write(self, s): window.pyrunWrite(str(s), self.err)",
    "    def flush(self): pass",
    "sys.stdout = _Out(False)",
    "sys.stderr = _Out(True)",
    "_lines = [str(x) for x in window.pyrunInput]",
    "def _input(prompt=''):",
    "    sys.stdout.write(str(prompt))",
    "    v = _lines.pop(0) if _lines else ''",
    "    sys.stdout.write(v + '\\n')",
    "    return v",
    // A loop that never ends would freeze the lesson, so the program is traced and stopped once it has run for
    // five seconds.
    "import time",
    "_t0 = time.time()",
    "def _tick():",
    "    if time.time() - _t0 > 5:",
    "        raise TimeoutError('stopped after 5 seconds: is there a loop that never ends?')",
    "_n = [0]",
    "def _trace(frame, event, arg):",
    "    _n[0] += 1",
    "    if _n[0] % 2000 == 0:",
    "        _tick()",
    "    return _trace",
    "try:",
    "    _code = compile(window.pyrunCode, '<string>', 'exec')",
    "    sys.settrace(_trace)",
    "    try:",
    "        exec(_code, {'__name__': '__main__', 'input': _input})",
    "    finally:",
    "        sys.settrace(None)",
    "except SystemExit:",
    "    pass",
    "except BaseException as _e:",
    "    # Only the learner's own lines, not this runner's.",
    "    _parts = [p for p in traceback.format_exception(_e) if '#pyrun' not in p and 'VFS.' not in p]",
    "    sys.stderr.write(''.join(_parts).replace('File \"<string>\", ', ''))",
  ].join("\n");

  /** A readable line for an error thrown out of the runner itself (a Python exception object or a JS error). */
  function describe(e) {
    if (e && e.__class__) {
      const name = (e.__class__.__name__ || (e.__class__.$infos && e.__class__.$infos.__name__) || "Error");
      const args = Array.isArray(e.args) ? e.args.map(String).join(", ") : "";
      return args ? `${name}: ${args}` : name;
    }
    return String(e && e.message ? e.message : e);
  }

  /** Run `code` with `input` lines; returns {out, err} as text. */
  function run(code, input) {
    start();
    let out = "";
    let err = "";
    window.pyrunWrite = (s, isErr) => {
      if (isErr) err += s;
      else out += s;
    };
    window.pyrunInput = String(input || "").split(/\r?\n/).filter((l, i, a) => l !== "" || i < a.length - 1);
    window.pyrunCode = String(code);
    try {
      window.__BRYTHON__.runPythonSource(RUNNER, { id: "pyrun" + Date.now() });
    } catch (e) {
      err += describe(e) + "\n";
    }
    return { out, err };
  }

  function el(tag, attrs, text) {
    const n = document.createElement(tag);
    for (const k in attrs || {}) n.setAttribute(k, attrs[k]);
    if (text != null) n.textContent = text;
    return n;
  }

  let styled = false;
  function style() {
    if (styled) return;
    styled = true;
    const s = el("style");
    s.textContent = `
      .pyrun { display: grid; gap: 6px; font: 14px/1.4 system-ui, sans-serif; color: var(--text); }
      .pyrun textarea { width: 100%; box-sizing: border-box; font: 13px/1.45 ui-monospace, Consolas, monospace;
        background: var(--panel); color: var(--text); border: 1px solid var(--line); border-radius: 6px; padding: 8px;
        tab-size: 4; resize: vertical; }
      .pyrun .bar { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
      .pyrun pre { margin: 0; min-height: 2.6em; max-height: 18em; overflow: auto; white-space: pre-wrap;
        font: 13px/1.45 ui-monospace, Consolas, monospace; background: var(--panel); border: 1px solid var(--line);
        border-radius: 6px; padding: 8px; }
      .pyrun .err { color: var(--bad); }
      .pyrun .verdict { font-weight: 600; }
      .pyrun label { font-size: 12px; color: var(--muted); }`;
    document.head.appendChild(s);
  }

  /** A code box with Run and Reset buttons, an optional input box, and the output under them. */
  function mount(target, code, opts) {
    opts = opts || {};
    style();
    const box = typeof target === "string" ? document.querySelector(target) : target;
    const wrap = el("div", { class: "pyrun" });
    const ta = el("textarea", { rows: String(opts.height || Math.min(18, String(code).split("\n").length + 1)), spellcheck: "false", "aria-label": "Python code" });
    ta.value = code;
    if (opts.readOnly) ta.readOnly = true;
    ta.addEventListener("keydown", (e) => {
      if (e.key === "Tab" && !e.shiftKey && !ta.readOnly) {
        e.preventDefault();
        const a = ta.selectionStart;
        ta.setRangeText("    ", a, ta.selectionEnd, "end");
      } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        go();
      }
    });
    let inp = null;
    if (opts.input != null) {
      inp = el("textarea", { rows: "2", spellcheck: "false", "aria-label": "Input, one line per input() call" });
      inp.value = opts.input;
    }
    const runBtn = el("button", { type: "button" }, "Run");
    const resetBtn = el("button", { type: "button" }, "Reset");
    const hint = el("span", { class: "hint" }, "Ctrl+Enter runs it");
    hint.style.color = "var(--muted)";
    hint.style.fontSize = "12px";
    const outEl = el("pre", { "aria-live": "polite" });
    const verdict = el("div", { class: "verdict" });
    function go() {
      runBtn.disabled = true;
      outEl.textContent = "Running…";
      setTimeout(() => {
        const r = run(ta.value, inp ? inp.value : "");
        outEl.textContent = "";
        if (r.out) outEl.appendChild(document.createTextNode(r.out));
        if (r.err) outEl.appendChild(el("span", { class: "err" }, r.err));
        if (!r.out && !r.err) outEl.textContent = "(no output)";
        verdict.textContent = opts.check && !r.err ? String(opts.check(r.out) || "") : "";
        runBtn.disabled = false;
      }, 10);
    }
    runBtn.addEventListener("click", go);
    resetBtn.addEventListener("click", () => {
      ta.value = code;
      outEl.textContent = "";
      verdict.textContent = "";
    });
    const bar = el("div", { class: "bar" });
    bar.append(runBtn, resetBtn, hint);
    wrap.append(ta);
    if (inp) wrap.append(el("label", {}, "Input (one line for each input() call)"), inp);
    wrap.append(bar, outEl, verdict);
    box.appendChild(wrap);
    return { run: go, textarea: ta, output: outEl };
  }

  window.PyRun = { run, mount };
})();
