/* Unified AI generation runtime bridge (frontend).
   One module drives every generation panel: theme / SVG component / poster
   HTML / cloud image / ComfyUI. Each run gets a backend job (streaming,
   cancellable, validate-retry loop) and renders a shared console widget:
   status line + elapsed + stop button + collapsible stream log.
   Self-contained like pet.js: no path dependencies, exposes window.AIJobs. */
(function () {
  "use strict";

  var invoke = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : null;

  var STR = {
    "zh-CN": {
      submitted: "已提交", generating: "生成中…", thinking: "思考中",
      stopped: "已停止", stop: "停止", stopping: "停止中…",
      done_in: "完成", retry: "重试", failed: "失败",
      demo: "浏览器演示模式不可用，请在桌面客户端中使用",
      show_log: "输出日志", chars: "字",
    },
    en: {
      submitted: "submitted", generating: "generating…", thinking: "thinking",
      stopped: "stopped", stop: "Stop", stopping: "stopping…",
      done_in: "done in", retry: "Retry", failed: "failed",
      demo: "Unavailable in browser demo mode; use the desktop app",
      show_log: "output log", chars: " chars",
    },
  };
  function lang() {
    try { return localStorage.getItem("wxwright-lang") || "zh-CN"; } catch (e) { return "zh-CN"; }
  }
  function s(key) { return (STR[lang()] || STR["zh-CN"])[key]; }

  /* single global listener dispatching events to running jobs by id */
  var handlers = new Map();
  var listening = false;
  function ensureListener() {
    if (listening || !window.__TAURI__) return;
    listening = true;
    window.__TAURI__.event.listen("ai-job", function (e) {
      var p = e.payload || {};
      var h = handlers.get(p.id);
      if (h) h(p);
    });
  }

  var LOG_CAP = 6000;

  /* console widget: mounts into `container`, one per run */
  function consoleWidget(container) {
    var box = document.createElement("div");
    box.className = "job-console";
    box.innerHTML =
      '<div class="jc-head">' +
      '<span class="jc-spin" aria-hidden="true"></span>' +
      '<span class="jc-status"></span>' +
      '<span class="jc-elapsed"></span>' +
      '<button type="button" class="jc-stop">' + s("stop") + "</button>" +
      "</div>" +
      '<pre class="jc-log" hidden></pre>';
    container.innerHTML = "";
    container.appendChild(box);
    var statusEl = box.querySelector(".jc-status");
    var elapsedEl = box.querySelector(".jc-elapsed");
    var logEl = box.querySelector(".jc-log");
    var stopBtn = box.querySelector(".jc-stop");
    var spin = box.querySelector(".jc-spin");
    var jobId = null, timer = null, t0 = 0, logLen = 0, settled = false;
    function tick() { elapsedEl.textContent = ((Date.now() - t0) / 1000).toFixed(0) + "s"; }
    stopBtn.addEventListener("click", function () {
      if (jobId == null || settled) return;
      stopBtn.disabled = true;
      statusEl.textContent = s("stopping");
      if (invoke) invoke("ai_job_stop", { id: jobId }).catch(function () {});
    });
    return {
      begin: function (id) {
        jobId = id; t0 = Date.now(); tick();
        timer = setInterval(tick, 500);
        statusEl.textContent = s("submitted");
      },
      status: function (text) { statusEl.textContent = text; },
      think: function (chars) {
        statusEl.textContent = s("thinking") + "… (" + chars + s("chars") + ")";
      },
      delta: function (d) {
        if (logEl.hidden) logEl.hidden = false;
        logLen += d.length;
        if (logLen > LOG_CAP) {
          var over = logLen - LOG_CAP;
          logEl.textContent = logEl.textContent.slice(logEl.textContent.length - (LOG_CAP - 500)) ;
          logLen = LOG_CAP - 500;
        }
        logEl.textContent += d;
        logEl.scrollTop = logEl.scrollHeight;
        statusEl.textContent = s("generating");
      },
      finish: function (ok, seconds) {
        settled = true;
        clearInterval(timer);
        spin.remove();
        stopBtn.remove();
        if (ok) {
          statusEl.textContent = s("done_in") + " " + Number(seconds || 0).toFixed(1) + "s";
          box.classList.add("jc-ok");
        } else {
          box.classList.add("jc-err");
        }
        setTimeout(function () { if (box.parentNode) box.parentNode.removeChild(box); }, ok ? 4000 : 8000);
      },
      fail: function (msg) {
        settled = true;
        clearInterval(timer);
        spin.remove();
        stopBtn.remove();
        box.classList.add("jc-err");
        statusEl.textContent = msg;
        // keep the box so the user can read the error
      },
      markStopped: function () {
        settled = true;
        clearInterval(timer);
        spin.remove();
        stopBtn.remove();
        statusEl.textContent = s("stopped");
        box.classList.add("jc-ok");
        setTimeout(function () { if (box.parentNode) box.parentNode.removeChild(box); }, 4000);
      },
    };
  }

  /* Run a job: AIJobs.run(kind, params, containerEl) -> Promise
     resolves { result, elapsed } | { stopped: true }; rejects Error(msg). */
  function run(kind, params, container) {
    return new Promise(function (resolve, reject) {
      if (!invoke) {
        var d0 = document.createElement("div");
        d0.className = "section-hint";
        d0.textContent = s("demo");
        container.innerHTML = "";
        container.appendChild(d0);
        reject(new Error(s("demo")));
        return;
      }
      ensureListener();
      var ui = consoleWidget(container);
      var id = null;
      invoke("ai_job_start", { kind: kind, params: params || {} })
        .then(function (jobId) {
          id = jobId;
          ui.begin(id);
          handlers.set(id, function (p) {
            var data = p.data || {};
            if (p.ev === "status") ui.status(data.text || "");
            else if (p.ev === "think") ui.think(data.chars || 0);
            else if (p.ev === "delta") ui.delta(String(data));
            else if (p.ev === "done") {
              handlers.delete(id);
              if (data.stopped) { ui.markStopped(); resolve({ stopped: true }); }
              else {
                ui.finish(true, data.elapsed);
                resolve({ result: data.result, elapsed: data.elapsed });
              }
            } else if (p.ev === "error") {
              handlers.delete(id);
              ui.fail(String(data));
              reject(new Error(String(data)));
            }
          });
        })
        .catch(function (e) {
          ui.fail(String(e));
          reject(new Error(String(e)));
        });
    });
  }

  window.AIJobs = {
    run: run,
    stop: function (id) { if (invoke) return invoke("ai_job_stop", { id: id }); },
  };
})();
