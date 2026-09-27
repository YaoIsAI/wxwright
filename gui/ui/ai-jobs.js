/* Unified AI generation runtime bridge (frontend).
   Minimal interaction per mainstream-agent conventions: the trigger button
   ITSELF becomes the stop control while a job runs - no extra console, no
   thinking box, no separate stop button. Clicking it again cancels the job
   (real disconnect on the backend). Exposes window.AIJobs. */
(function () {
  "use strict";

  var invoke = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : null;

  var STR = {
    "zh-CN": { stop: "停止", demo: "浏览器演示模式不可用，请在桌面客户端中使用" },
    en: { stop: "Stop", demo: "Unavailable in browser demo mode; use the desktop app" },
  };
  function lang() {
    try { return localStorage.getItem("wxwright-lang") || "zh-CN"; } catch (e) { return "zh-CN"; }
  }
  function s(key) { return (STR[lang()] || STR["zh-CN"])[key]; }

  /* single global listener dispatching events to running jobs by id */
  var handlers = new Map();
  /* Events that arrived before their handler existed. The backend registers
     the job, spawns the work and only then returns the id - so a job that
     fails fast (no image model configured, no provider) can emit `done` or
     `error` before `ai_job_start` resolves. Dropping those events left the
     promise unsettled forever and the trigger button stuck on "Stop". */
  var orphan = [];
  var ORPHAN_CAP = 64;
  var listening = false;
  function ensureListener() {
    if (listening || !window.__TAURI__) return;
    listening = true;
    window.__TAURI__.event.listen("ai-job", function (e) {
      var p = e.payload || {};
      var h = handlers.get(p.id);
      if (h) {
        h(p);
        return;
      }
      if (orphan.length < ORPHAN_CAP) orphan.push(p);
    });
  }

  /* Register a handler, then replay anything that raced ahead of it. */
  function registerHandler(id, fn) {
    handlers.set(id, fn);
    if (!orphan.length) return;
    var mine = orphan.filter(function (p) { return p.id === id; });
    if (!mine.length) return;
    orphan = orphan.filter(function (p) { return p.id !== id; });
    mine.forEach(function (p) { fn(p); });
  }

  var SPIN = '<svg class="btn-busy-spin" viewBox="0 0 24 24" width="13" height="13" aria-hidden="true"><circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" stroke-width="3" stroke-dasharray="42 20" stroke-linecap="round"/></svg>';

  /* Run a job. opts.button morphs into the stop control while running.
     Resolves { result, elapsed } | { stopped: true }; rejects Error(msg). */
  function run(kind, params, opts) {
    opts = opts || {};
    return new Promise(function (resolve, reject) {
      if (!invoke) {
        reject(new Error(s("demo")));
        return;
      }
      ensureListener();
      var btn = opts.button || null;
      var id = null;
      var busy = false;
      var savedHtml = null;
      var stopHandler = function (e) {
        if (!busy) return;
        // this click means "cancel": swallow it so the original action
        // handler does not re-fire (its busy guard would also stop it)
        e.stopImmediatePropagation();
        e.preventDefault();
        if (id != null) invoke("ai_job_stop", { id: id }).catch(function () {});
      };
      function setBusyUI(on) {
        if (!btn) return;
        if (on) {
          savedHtml = btn.innerHTML;
          btn.classList.add("btn-busy");
          btn.innerHTML = SPIN + "<span>" + s("stop") + "</span>";
          busy = true;
          btn.addEventListener("click", stopHandler, true); // capture: runs first
        } else {
          busy = false;
          btn.classList.remove("btn-busy");
          btn.removeEventListener("click", stopHandler, true);
          if (savedHtml != null) btn.innerHTML = savedHtml;
        }
      }
      setBusyUI(true);
      invoke("ai_job_start", { kind: kind, params: params || {} })
        .then(function (jobId) {
          id = jobId;
          registerHandler(id, function (p) {
            var data = p.data || {};
            if (p.ev === "status" || p.ev === "think" || p.ev === "delta") {
              if (opts.onProgress) opts.onProgress(p.ev, data); // optional, no UI by default
              return;
            }
            if (p.ev === "done") {
              handlers.delete(id);
              setBusyUI(false);
              if (data.stopped) resolve({ stopped: true });
              else resolve({ result: data.result, elapsed: data.elapsed });
            } else if (p.ev === "error") {
              handlers.delete(id);
              setBusyUI(false);
              reject(new Error(String(data)));
            }
          });
        })
        .catch(function (e) {
          setBusyUI(false);
          reject(new Error(String(e)));
        });
    });
  }

  window.AIJobs = {
    run: run,
    stop: function (id) { if (invoke) return invoke("ai_job_stop", { id: id }); },
  };
})();
