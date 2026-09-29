// The Fidryn mill page: check, run, explore, and render a module through the
// fidryn ui server on 127.0.0.1. Pure helpers come first so `node --test` can
// load this file without a DOM; the page code after them runs in a browser.
(function () {
  "use strict";

  var STATE_KEY = "fidryn-mill";
  var THEME_KEY = "fidryn-theme";
  var BUFFERS = ["module", "case", "template"];
  var VIEWS = ["opinion", "table", "json"];
  var TEXT_FIELDS = ["module", "case", "template", "query", "validAt", "knownAt"];

  // ------------------------------------------------------------ pure helpers

  /** Escape text for HTML element content and quoted attributes. */
  function esc(text) {
    return String(text)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  var RFC3339 = /^(\d{4})-(\d{2})-(\d{2})[Tt ](\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(?:[Zz]|[+-](\d{2}):(\d{2}))$/;
  var MONTH_DAYS = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

  /**
   * True when `text` is an RFC 3339 date-time with a zone (`Z` or `+hh:mm`),
   * the form the server accepts for validAt and knownAt.
   */
  function isRfc3339(text) {
    var m = typeof text === "string" ? RFC3339.exec(text) : null;
    if (!m) return false;
    var year = Number(m[1]);
    var month = Number(m[2]);
    var day = Number(m[3]);
    var leap = (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
    var days = month === 2 && leap ? 29 : MONTH_DAYS[month - 1];
    if (!days || day < 1 || day > days) return false;
    if (Number(m[4]) > 23 || Number(m[5]) > 59 || Number(m[6]) > 60) return false;
    return m[7] === undefined || (Number(m[7]) <= 23 && Number(m[8]) <= 59);
  }

  function isObject(value) {
    return value !== null && typeof value === "object" && !Array.isArray(value);
  }

  /** Compact JSON: the text for any shape the templates below do not know. */
  function compact(value) {
    var text = JSON.stringify(value);
    return text === undefined ? String(value) : text;
  }

  /** A string as itself, anything else as compact JSON. */
  function str(value) {
    return typeof value === "string" ? value : compact(value);
  }

  /**
   * Plain text for a tagged runtime value (`{"kind", "data"}`), written the
   * way the server's opinion sentences write it.
   */
  function valueText(value) {
    if (!isObject(value) || typeof value.kind !== "string") return compact(value);
    var data = value.data;
    switch (value.kind) {
      case "int":
      case "decimal":
      case "bool":
      case "instant":
      case "duration":
        return str(data);
      case "string":
        return typeof data === "string" ? "\"" + data + "\"" : compact(value);
      case "entity":
        return typeof data === "string" ? data : compact(value);
      case "unit":
        return "unit";
      case "ctor":
        return ctorText(value);
      case "set":
        return Array.isArray(data) ? "{" + data.map(valueText).join(", ") + "}" : compact(value);
      case "map":
        if (!isObject(data)) return compact(value);
        return "{" + Object.keys(data).sort().map(function (key) {
          return key + ": " + valueText(data[key]);
        }).join(", ") + "}";
      case "option":
        return data === null || data === undefined ? "none" : valueText(data);
      default:
        return compact(value);
    }
  }

  function ctorText(value) {
    var data = value.data;
    if (!isObject(data) || typeof data.name !== "string") return compact(value);
    var fields = isObject(data.fields) ? data.fields : {};
    var keys = Object.keys(fields).sort();
    if (keys.length === 0) return data.name;
    var positional = keys.every(function (key) { return key.charAt(0) === "_"; });
    return data.name + "(" + keys.map(function (key) {
      return (positional ? "" : key + ": ") + valueText(fields[key]);
    }).join(", ") + ")";
  }

  /** `list` when it is a non-empty array of strings, otherwise null. */
  function stringList(list) {
    var ok = Array.isArray(list) && list.length > 0 &&
      list.every(function (item) { return typeof item === "string"; });
    return ok ? list : null;
  }

  function doctrineNames(list) {
    if (!Array.isArray(list)) return [];
    return list.map(function (item) {
      if (typeof item === "string") return item;
      return isObject(item) && typeof item.name === "string" ? item.name : null;
    }).filter(function (name) { return name !== null; });
  }

  /**
   * Plain text for one request of a suspended outcome, written the way the
   * server's opinion sentences write it.
   */
  function requestText(request) {
    if (!isObject(request) || typeof request.kind !== "string") return compact(request);
    var among;
    switch (request.kind) {
      case "needCustom":
        return typeof request.payload === "string"
          ? request.payload + " (" + str(request.effect) + ")"
          : str(request.effect);
      case "needEvidence":
        return "evidence matching " + str(request.schema);
      case "needInterpretation":
        return "an interpretation of " + str(request.family) +
          (typeof request.source === "string" && request.source !== "" ? " under " + request.source : "");
      case "needJudgment":
        return "a determination under " + str(request.protocol);
      case "needChoice":
        among = stringList(request.options);
        return "a decision under " + str(request.protocol) + (among ? " among " + among.join(", ") : "");
      case "needApplicableLaw":
        among = stringList(request.candidates);
        return "applicable law" + (among ? " among " + among.join(", ") : "");
      case "needConflict":
        among = doctrineNames(request.doctrines);
        return "one applicable conflict doctrine" + (among.length ? " among " + among.join(", ") : "");
      default:
        return request.kind;
    }
  }

  /** A complete mill state from any object, filling gaps with defaults. */
  function copyState(source) {
    var from = isObject(source) ? source : {};
    var out = { v: 1 };
    TEXT_FIELDS.forEach(function (key) {
      out[key] = typeof from[key] === "string" ? from[key] : "";
    });
    out.buffer = BUFFERS.indexOf(from.buffer) >= 0 ? from.buffer : "module";
    out.sample = typeof from.sample === "string" ? from.sample : null;
    out.view = VIEWS.indexOf(from.view) >= 0 ? from.view : "opinion";
    return out;
  }

  /**
   * The saved mill state from `storage` (localStorage or a stand-in), or a
   * copy of `fallback` when nothing usable is saved: storage that is missing
   * or throws, text that is not JSON, another version, or a text field that
   * is missing or not a string. An unknown buffer or view is reset alone.
   */
  function loadState(storage, fallback) {
    var base = copyState(fallback);
    var saved;
    try {
      saved = JSON.parse(storage.getItem(STATE_KEY));
    } catch (err) {
      return base;
    }
    if (!isObject(saved) || saved.v !== 1) return base;
    for (var i = 0; i < TEXT_FIELDS.length; i++) {
      if (typeof saved[TEXT_FIELDS[i]] !== "string") return base;
    }
    return copyState(saved);
  }

  function parseJson(text) {
    try {
      var value = JSON.parse(text);
      return value !== null && typeof value === "object" ? value : null;
    } catch (err) {
      return null;
    }
  }

  /**
   * Fetch `path` and read the body. Never rejects: a failed request resolves
   * to `{ network: true, status: 0, data: null, text: "" }`; any response
   * resolves to `{ network: false, status, data, text }`, where `data` is the
   * parsed JSON object or array (null when the body is not JSON).
   */
  function send(path, init, fetchFn) {
    return new Promise(function (resolve) {
      resolve((fetchFn || fetch)(path, init));
    }).then(function (response) {
      return Promise.resolve(response.text()).then(function (body) {
        return { network: false, status: response.status, data: parseJson(body), text: String(body) };
      });
    }).catch(function () {
      return { network: true, status: 0, data: null, text: "" };
    });
  }

  /**
   * POST `body` as JSON. Resolves like `send`: HTTP errors such as a 400 with
   * a JSON body come back as data, and a server that is gone comes back as
   * `network: true` instead of a rejected promise.
   */
  function postJson(path, body, fetchFn) {
    return send(path, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body)
    }, fetchFn);
  }

  /** UTF-8 byte length of the code point `cp`. */
  function utf8Length(cp) {
    return cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
  }

  /**
   * The string index of UTF-8 byte offset `offset` in `text` (diagnostic
   * spans are byte offsets). An offset inside a character gives that
   * character's start; offsets past the end give `text.length`.
   */
  function byteToIndex(text, offset) {
    var bytes = 0;
    var i = 0;
    while (i < text.length) {
      var cp = text.codePointAt(i);
      var size = utf8Length(cp);
      if (bytes + size > offset) return i;
      bytes += size;
      i += cp > 0xffff ? 2 : 1;
    }
    return text.length;
  }

  /** 1-based line and column of string index `index`; columns count characters. */
  function lineCol(text, index) {
    var end = Math.max(0, Math.min(index, text.length));
    var line = 1;
    var col = 1;
    var i = 0;
    while (i < end) {
      var cp = text.codePointAt(i);
      if (cp === 10) {
        line += 1;
        col = 1;
      } else {
        col += 1;
      }
      i += cp > 0xffff ? 2 : 1;
    }
    return { line: line, col: col };
  }

  var JSON_NUMBER = /-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/y;

  /**
   * Where and why `text` is not JSON, as `{ index, message }`, or null when it
   * is. Browsers word and place JSON.parse errors differently, so the mill
   * finds the first error itself.
   */
  function jsonError(text) {
    var i = 0;
    function fail(message) {
      throw { jsonIndex: i, message: message };
    }
    function space() {
      while (i < text.length && " \t\n\r".indexOf(text.charAt(i)) >= 0) i += 1;
    }
    function word(w) {
      if (text.slice(i, i + w.length) !== w) return false;
      i += w.length;
      return true;
    }
    function string() {
      i += 1;
      while (i < text.length) {
        var c = text.charAt(i);
        if (c === "\"") {
          i += 1;
          return;
        }
        if (c === "\\") {
          var e = text.charAt(i + 1);
          if (e === "u" && /^[0-9a-fA-F]{4}$/.test(text.slice(i + 2, i + 6))) {
            i += 6;
            continue;
          }
          if (e === "" || "\"\\/bfnrt".indexOf(e) < 0) fail("Invalid escape in a string");
          i += 2;
          continue;
        }
        if (c < " ") fail("Line break or control character in a string");
        i += 1;
      }
      fail("Unterminated string");
    }
    function value() {
      space();
      if (i >= text.length) fail("Unexpected end of JSON");
      var c = text.charAt(i);
      if (c === "{") {
        i += 1;
        space();
        if (word("}")) return;
        for (;;) {
          space();
          if (text.charAt(i) !== "\"") fail("Expected a property name in double quotes");
          string();
          space();
          if (!word(":")) fail("Expected : after the property name");
          value();
          space();
          if (word(",")) continue;
          if (word("}")) return;
          fail("Expected , or } after the value");
        }
      }
      if (c === "[") {
        i += 1;
        space();
        if (word("]")) return;
        for (;;) {
          value();
          space();
          if (word(",")) continue;
          if (word("]")) return;
          fail("Expected , or ] after the value");
        }
      }
      if (c === "\"") return string();
      if (c === "-" || (c >= "0" && c <= "9")) {
        JSON_NUMBER.lastIndex = i;
        var m = JSON_NUMBER.exec(text);
        if (!m) fail("Invalid number");
        i += m[0].length;
        return;
      }
      if (word("true") || word("false") || word("null")) return;
      fail("Unexpected " + JSON.stringify(String.fromCodePoint(text.codePointAt(i))));
    }
    try {
      value();
      space();
      if (i < text.length) fail("Unexpected text after the JSON value");
      return null;
    } catch (err) {
      if (err && typeof err.jsonIndex === "number") return { index: err.jsonIndex, message: err.message };
      // Anything else, such as a stack overflow on deeply nested input, falls back to JSON.parse's message.
      return null;
    }
  }

  /** The case JSON to send, or where it fails to parse. Blank text sends `{}`. */
  function parseCase(text) {
    if (text.trim() === "") return { ok: true, value: {} };
    try {
      return { ok: true, value: JSON.parse(text) };
    } catch (err) {
      var found = jsonError(text) || { index: text.length, message: String(err && err.message) };
      var at = lineCol(text, found.index);
      return { ok: false, index: found.index, line: at.line, col: at.col, message: found.message };
    }
  }

  if (typeof document === "undefined") {
    module.exports = {
      esc: esc,
      isRfc3339: isRfc3339,
      valueText: valueText,
      requestText: requestText,
      loadState: loadState,
      postJson: postJson
    };
    return;
  }

  // ------------------------------------------------ theme, before first paint

  function savedTheme() {
    try {
      var theme = window.localStorage.getItem(THEME_KEY);
      return theme === "light" || theme === "dark" ? theme : "";
    } catch (err) {
      return "";
    }
  }

  var initialTheme = savedTheme();
  document.documentElement.classList.add("js");
  if (initialTheme) document.documentElement.dataset.theme = initialTheme;

  // ---------------------------------------------------------------- the page

  var NETWORK_MESSAGE = "Is fidryn ui still running? The mill could not reach 127.0.0.1.";
  var CLOCK_HINT = "Use RFC 3339, for example 2026-09-17T12:00:00Z.";
  var DEFAULT_TEMPLATE = "{{module}}@{{version}}\noutside: {{#each outside_scope}}{{item}} {{/each}}\n";
  var HISTORY_LIMIT = 20;
  var HEALTH_RETRY_MS = 3000;
  var BUSY_DELAY_MS = 400;
  var ACTIONS = ["check", "run", "explore", "render"];
  var BUSY_TEXT = { check: "Checking…", run: "Running…", explore: "Exploring…", render: "Rendering…" };
  var BUFFER_LABELS = { module: "Module source", case: "Case JSON", template: "Template" };
  var SAMPLE_FIELDS = ["id", "title", "blurb", "source", "case", "query", "validAt", "knownAt", "action", "expect"];
  var KINDS = {
    determinate: { cls: "det", label: "Determinate", short: "Det" },
    contingent: { cls: "con", label: "Contingent", short: "Cont" },
    suspended: { cls: "sus", label: "Suspended", short: "Susp" },
    normConflict: { cls: "nc", label: "NormConflict", short: "Conf" },
    outsideCompetence: { cls: "oc", label: "OutsideCompetence", short: "Out" },
    inconsistent: { cls: "inc", label: "Inconsistent", short: "Inc" }
  };

  var el = {};
  var state = null;
  var samples = [];
  var samplesFailed = false;
  var historyEntries = [];
  var entrySeq = 0;
  var shown = null;
  var shownFromHistory = false;
  var pendingSample = null;
  var pendingEntry = null;
  var busy = false;
  var busyTimer = 0;
  var queuedAction = null;
  var healthTimer = 0;
  var editorBuffer = null;
  var bufferViews = {};
  var diagnostics = { source: null, list: [], version: 0 };

  document.addEventListener("DOMContentLoaded", start);

  function start() {
    [
      "health", "samples", "confirm", "confirm-replace", "confirm-cancel", "history", "buffers",
      "buffer-status", "editor-wrap", "gutter", "hl", "editor", "diag-pop", "query", "query-names",
      "validAt", "validAt-err", "knownAt", "knownAt-err", "check", "run", "explore", "render",
      "views", "result-body"
    ].forEach(function (id) {
      el[id.replace(/-([a-z])/g, function (_, c) { return c.toUpperCase(); })] = document.getElementById(id);
    });
    setupTheme();
    showPlatformKeys();
    wireEvents();
    renderHistory();
    checkHealth();
    loadSamples().then(function () {
      var first = samples[0];
      state = loadState(storage(), first ? sampleState(first, null) : copyState({ template: DEFAULT_TEMPLATE }));
      applyInputs();
      setView(state.view);
      markSample();
      editorChanged("load");
      saveState();
    });
  }

  // ------------------------------------------------------------------- theme

  function currentTheme() {
    var theme = document.documentElement.dataset.theme;
    if (theme === "light" || theme === "dark") return theme;
    return window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }

  function setupTheme() {
    var button = document.querySelector("[data-theme-toggle]");
    if (!button) return;
    function label() {
      button.setAttribute("aria-label", currentTheme() === "dark" ? "Switch to light theme" : "Switch to dark theme");
    }
    button.hidden = false;
    label();
    button.addEventListener("click", function () {
      var next = currentTheme() === "dark" ? "light" : "dark";
      document.documentElement.dataset.theme = next;
      try {
        window.localStorage.setItem(THEME_KEY, next);
      } catch (err) {
        // Private windows may refuse storage; the theme still applies now.
      }
      label();
    });
  }

  function showPlatformKeys() {
    var mac = /Mac|iPhone|iPad|iPod/.test(navigator.platform || navigator.userAgent || "");
    if (!mac) return;
    Array.prototype.forEach.call(document.querySelectorAll(".mill-mod"), function (key) {
      key.textContent = "⌘";
      key.title = "Command";
    });
  }

  // ------------------------------------------------------------- DOM helpers

  /** Build an element: `props` become attributes (`text` sets textContent), `kids` are nodes or strings. */
  function h(tag, props, kids) {
    var node = document.createElement(tag);
    Object.keys(props || {}).forEach(function (key) {
      var value = props[key];
      if (value === null || value === undefined || value === false) return;
      if (key === "text") node.textContent = value;
      else if (key === "className") node.className = value;
      else node.setAttribute(key, value === true ? "" : String(value));
    });
    (kids || []).forEach(function (kid) {
      if (kid === null || kid === undefined || kid === false) return;
      node.appendChild(typeof kid === "string" ? document.createTextNode(kid) : kid);
    });
    return node;
  }

  function clear(node) {
    while (node.firstChild) node.removeChild(node.firstChild);
  }

  /** `{ cls, label, short }` for an outcome kind; unknown kinds get a plain stamp. */
  function kindInfo(kind) {
    if (typeof kind === "string" && Object.prototype.hasOwnProperty.call(KINDS, kind)) return KINDS[kind];
    var name = typeof kind === "string" && kind ? kind : "Outcome";
    return { cls: "", label: name, short: name };
  }

  /** A stamp showing `label`; when `full` differs it becomes the accessible name. */
  function stampEl(cls, label, full) {
    var node = h("span", { className: cls ? "stamp " + cls : "stamp" });
    if (full && full !== label) {
      node.title = full;
      node.appendChild(h("span", { "aria-hidden": "true", text: label }));
      node.appendChild(h("span", { className: "sr-only", text: full }));
    } else {
      node.textContent = label;
    }
    return node;
  }

  function clock(date) {
    function two(n) { return (n < 10 ? "0" : "") + n; }
    return two(date.getHours()) + ":" + two(date.getMinutes()) + ":" + two(date.getSeconds());
  }

  // ------------------------------------------------------------------ events

  function wireEvents() {
    el.samples.addEventListener("click", function (event) {
      var button = event.target.closest("[data-sample]");
      if (button) pickSample(button.getAttribute("data-sample"));
    });
    el.samples.addEventListener("focusin", function (event) {
      var button = event.target.closest("[data-sample]");
      if (button && button.scrollIntoView) button.scrollIntoView({ block: "nearest", inline: "nearest" });
    });
    el.confirmReplace.addEventListener("click", function () {
      var entry = pendingEntry;
      var sample = hideConfirm();
      if (entry) {
        restoreInputs(entry);
        focusRestore();
        return;
      }
      if (!sample) return;
      useSample(sample, true);
      focusSample(sample.id);
    });
    el.confirmCancel.addEventListener("click", function () {
      var entry = pendingEntry;
      var sample = hideConfirm();
      if (sample) focusSample(sample.id);
      else if (entry) focusRestore();
    });
    el.confirm.addEventListener("keydown", function (event) {
      if (event.key !== "Escape") return;
      var entry = pendingEntry;
      var sample = hideConfirm();
      if (sample) focusSample(sample.id);
      else if (entry) focusRestore();
    });
    el.buffers.addEventListener("click", function (event) {
      var tab = event.target.closest("[data-buffer]");
      if (tab && state) showBuffer(tab.getAttribute("data-buffer"));
    });
    el.buffers.addEventListener("keydown", onTabKey);
    el.editor.addEventListener("input", function () {
      if (!state) return;
      state[state.buffer] = el.editor.value;
      saveState();
      editorChanged("edit");
    });
    el.query.addEventListener("input", function () { setField("query"); });
    ["validAt", "knownAt"].forEach(function (name) {
      el[name].addEventListener("input", function () {
        setField(name);
        checkClock(name);
      });
    });
    ACTIONS.forEach(function (action) {
      el[action].addEventListener("click", function () { runAction(action); });
    });
    el.views.addEventListener("click", function (event) {
      var button = event.target.closest("[data-view]");
      if (button && !button.disabled && state) setView(button.getAttribute("data-view"));
    });
    el.history.addEventListener("click", function (event) {
      var button = event.target.closest("[data-history]");
      var entry = button && findEntry(button.getAttribute("data-history"));
      if (entry) showEntry(entry, true);
    });
    el.resultBody.addEventListener("click", onResultClick);
    document.addEventListener("keydown", function (event) {
      if (event.key !== "Enter" || !(event.ctrlKey || event.metaKey) || event.altKey) return;
      event.preventDefault();
      runAction(event.shiftKey ? "explore" : "run");
    });
  }

  function onTabKey(event) {
    var tabs = Array.prototype.slice.call(el.buffers.querySelectorAll("[data-buffer]"));
    var at = tabs.indexOf(document.activeElement);
    if (at < 0 || !state) return;
    var next = -1;
    if (event.key === "ArrowRight") next = (at + 1) % tabs.length;
    else if (event.key === "ArrowLeft") next = (at + tabs.length - 1) % tabs.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = tabs.length - 1;
    if (next < 0) return;
    event.preventDefault();
    tabs[next].focus();
    showBuffer(tabs[next].getAttribute("data-buffer"));
  }

  function onResultClick(event) {
    var copy = event.target.closest("[data-copy]");
    if (copy) {
      copyFrom(copy);
      return;
    }
    var restore = event.target.closest("[data-restore]");
    if (restore) {
      var entry = findEntry(restore.getAttribute("data-restore"));
      if (entry) askRestore(entry);
      return;
    }
    var jump = event.target.closest("[data-jump]");
    if (jump) {
      var parts = jump.getAttribute("data-jump").split(":");
      jumpTo(parts[0], Number(parts[1]), Number(parts[2]));
    }
  }

  // ------------------------------------------------------------------- state

  function storage() {
    try {
      return window.localStorage;
    } catch (err) {
      return null;
    }
  }

  function saveState() {
    var store = storage();
    if (!state || !store) return;
    try {
      store.setItem(STATE_KEY, JSON.stringify(state));
    } catch (err) {
      // Storage can be full or refused; the page keeps working without it.
    }
  }

  function sampleState(sample, template) {
    return copyState({
      module: sample.source,
      case: sample.case,
      template: template === null ? DEFAULT_TEMPLATE : template,
      query: sample.query,
      validAt: sample.validAt,
      knownAt: sample.knownAt,
      sample: sample.id
    });
  }

  function setField(name) {
    if (!state) return;
    state[name] = el[name].value;
    saveState();
  }

  function snapshot() {
    var inputs = { sample: state.sample };
    TEXT_FIELDS.forEach(function (key) { inputs[key] = state[key]; });
    return inputs;
  }

  /** True when two input snapshots would send the same request. */
  function sameInputs(a, b) {
    return TEXT_FIELDS.every(function (key) { return a[key] === b[key]; });
  }

  /** Put the saved inputs into the fields and the editor. */
  function applyInputs() {
    el.query.value = state.query;
    el.validAt.value = state.validAt;
    el.knownAt.value = state.knownAt;
    checkClock("validAt");
    checkClock("knownAt");
    editorBuffer = null;
    bufferViews = {};
    showBuffer(state.buffer);
  }

  function checkClock(name) {
    var ok = isRfc3339(el[name].value.trim());
    var err = el[name + "Err"];
    el[name].setAttribute("aria-invalid", ok ? "false" : "true");
    err.textContent = ok ? "" : CLOCK_HINT;
    err.hidden = ok;
    return ok;
  }

  // ----------------------------------------------------------------- samples

  function isSample(value) {
    return isObject(value) && SAMPLE_FIELDS.every(function (key) { return typeof value[key] === "string"; });
  }

  function loadSamples() {
    return send("/api/samples", { cache: "no-store" }).then(function (res) {
      if (res.network) {
        samplesFailed = true;
        networkDown();
        renderSamples();
        return;
      }
      samples = Array.isArray(res.data) ? res.data.filter(isSample) : [];
      samplesFailed = samples.length === 0;
      renderSamples();
    });
  }

  function findSample(id) {
    for (var i = 0; i < samples.length; i++) {
      if (samples[i].id === id) return samples[i];
    }
    return null;
  }

  function renderSamples() {
    clear(el.samples);
    if (samples.length === 0) {
      el.samples.appendChild(h("li", { className: "mill-none", text: samplesFailed ? "Samples are not available." : "Loading samples…" }));
      return;
    }
    samples.forEach(function (sample) {
      var kind = kindInfo(sample.expect);
      el.samples.appendChild(h("li", null, [
        h("button", { type: "button", className: "mill-sample", "data-sample": sample.id, title: sample.blurb }, [
          h("span", { text: sample.title }),
          stampEl(kind.cls, kind.short, kind.label)
        ])
      ]));
    });
    markSample();
  }

  /** Mark the sample the inputs came from. */
  function markSample() {
    Array.prototype.forEach.call(el.samples.querySelectorAll("[data-sample]"), function (button) {
      if (state && button.getAttribute("data-sample") === state.sample) button.setAttribute("aria-current", "true");
      else button.removeAttribute("aria-current");
    });
  }

  function focusSample(id) {
    Array.prototype.forEach.call(el.samples.querySelectorAll("[data-sample]"), function (button) {
      if (button.getAttribute("data-sample") === id) button.focus();
    });
  }

  /** True when loading a sample now would throw away the user's edits. */
  function isDirty() {
    if (!state) return false;
    var sample = findSample(state.sample);
    if (!sample) return state.module.trim() !== "" || state.case.trim() !== "";
    return state.module !== sample.source || state.case !== sample.case || state.query !== sample.query ||
      state.validAt !== sample.validAt || state.knownAt !== sample.knownAt;
  }

  function pickSample(id) {
    var sample = findSample(id);
    if (!sample || !state) return;
    if (!isDirty()) {
      useSample(sample, true);
      return;
    }
    pendingEntry = null;
    pendingSample = sample;
    document.getElementById("confirm-text").textContent = "Replace your edits with " + sample.title + "?";
    el.confirm.hidden = false;
    el.confirmCancel.focus();
  }

  /** Close the Replace or Cancel question; returns the sample it was about. */
  function hideConfirm() {
    var sample = pendingSample;
    pendingSample = null;
    pendingEntry = null;
    el.confirm.hidden = true;
    return sample;
  }

  function useSample(sample, runIt) {
    hideConfirm();
    state = copyState(Object.assign(sampleState(sample, state.template), { buffer: state.buffer, view: state.view }));
    saveState();
    applyInputs();
    markSample();
    editorChanged("load");
    if (runIt) {
      var next = sample.action === "explore" ? "explore" : "run";
      // A request still in flight answers for the old inputs; run this sample's action after it.
      if (busy) queuedAction = next;
      else runAction(next);
    }
  }

  // ----------------------------------------------------------------- buffers

  function showBuffer(name) {
    if (BUFFERS.indexOf(name) < 0) name = "module";
    if (editorBuffer) {
      bufferViews[editorBuffer] = {
        start: el.editor.selectionStart,
        end: el.editor.selectionEnd,
        top: el.editor.scrollTop,
        left: el.editor.scrollLeft
      };
    }
    state.buffer = name;
    editorBuffer = name;
    el.editor.value = state[name];
    el.editor.setAttribute("aria-label", BUFFER_LABELS[name]);
    Array.prototype.forEach.call(el.buffers.querySelectorAll("[data-buffer]"), function (tab) {
      var on = tab.getAttribute("data-buffer") === name;
      tab.setAttribute("aria-selected", on ? "true" : "false");
      tab.setAttribute("tabindex", on ? "0" : "-1");
    });
    el.editorWrap.setAttribute("aria-labelledby", "tab-" + name);
    var view = bufferViews[name];
    if (view) {
      el.editor.setSelectionRange(view.start, view.end);
      el.editor.scrollTop = view.top;
      el.editor.scrollLeft = view.left;
    } else {
      el.editor.setSelectionRange(0, 0);
      el.editor.scrollTop = 0;
      el.editor.scrollLeft = 0;
    }
    saveState();
    editorChanged("buffer");
  }

  /** Select `start`..`end` in `buffer` and bring it into view. */
  function jumpTo(buffer, start, end) {
    if (!state) return;
    showBuffer(buffer);
    var length = el.editor.value.length;
    var from = Math.max(0, Math.min(start || 0, length));
    var to = Math.max(from, Math.min(end || from, length));
    el.editor.focus();
    el.editor.setSelectionRange(from, to);
    var style = window.getComputedStyle(el.editor);
    var lineHeight = parseFloat(style.lineHeight) || 20;
    var top = (lineCol(el.editor.value, from).line - 1) * lineHeight;
    if (top < el.editor.scrollTop || top > el.editor.scrollTop + el.editor.clientHeight - 2 * lineHeight) {
      el.editor.scrollTop = Math.max(0, top - el.editor.clientHeight / 3);
    }
    el.editorWrap.scrollIntoView({ block: "nearest" });
    editorChanged("caret");
  }

  /** Called after the editor's text, buffer, or caret changes and after new diagnostics. */
  function editorChanged() {
    updateBufferStatus();
  }

  function updateBufferStatus() {
    var text = "";
    var mood = "";
    if (state.buffer === "module" && diagnostics.source === state.module) {
      var n = diagnostics.list.length;
      text = n === 0 ? "No problems" : n === 1 ? "1 problem" : n + " problems";
      mood = n === 0 ? "ok" : "err";
    } else if (state.buffer === "template") {
      text = "Used by Render";
    }
    clear(el.bufferStatus);
    el.bufferStatus.appendChild(h("span", { className: "mill-status-main", text: text }));
    el.bufferStatus.setAttribute("data-state", mood);
  }

  /** Remember the diagnostics of a check of `source`. */
  function noteDiagnostics(source, data) {
    if (!isObject(data) || typeof data.ok !== "boolean") return;
    diagnostics = {
      source: source,
      list: data.ok ? [] : (Array.isArray(data.diagnostics) ? data.diagnostics.filter(isObject) : []),
      version: diagnostics.version + 1
    };
    editorChanged("diagnostics");
  }

  // ------------------------------------------------------------------ health

  function setHealth(mood) {
    el.health.setAttribute("data-state", mood);
    var text = el.health.querySelector(".mill-health-text");
    if (text) text.textContent = mood === "ok" ? "ok" : mood === "down" ? "not responding" : "checking";
  }

  function checkHealth() {
    clearTimeout(healthTimer);
    return send("/api/health", { cache: "no-store" }).then(function (res) {
      var ok = !res.network && res.status === 200 && res.text.trim() === "ok";
      setHealth(ok ? "ok" : "down");
      if (!ok) healthTimer = setTimeout(checkHealth, HEALTH_RETRY_MS);
      else if (samplesFailed) loadSamples();
    });
  }

  /** The server answered a request: it is up. */
  function serverAnswered() {
    clearTimeout(healthTimer);
    setHealth("ok");
    if (samplesFailed) loadSamples();
  }

  /** The server stopped answering: turn the status red and poll until it is back. */
  function networkDown() {
    setHealth("down");
    clearTimeout(healthTimer);
    healthTimer = setTimeout(checkHealth, HEALTH_RETRY_MS);
  }

  // ----------------------------------------------------------------- actions

  function setBusy(action, on) {
    busy = on;
    clearTimeout(busyTimer);
    ACTIONS.forEach(function (name) {
      if (on) el[name].setAttribute("aria-disabled", "true");
      else el[name].removeAttribute("aria-disabled");
    });
    if (on) el[action].setAttribute("aria-busy", "true");
    else el[action].removeAttribute("aria-busy");
    el.resultBody.setAttribute("aria-busy", on ? "true" : "false");
    if (on) {
      busyTimer = setTimeout(function () {
        showNodes([h("p", { className: "mill-busy", text: BUSY_TEXT[action] })]);
      }, BUSY_DELAY_MS);
    }
  }

  /** The request for `action`, or `{ error }` when an input is not ready to send. */
  function buildRequest(action, inputs) {
    if (action === "check") return { path: "/api/check", body: { source: inputs.module } };
    if (action === "render") return { path: "/api/render", body: { source: inputs.module, template: inputs.template } };
    var clocks = ["validAt", "knownAt"];
    for (var i = 0; i < clocks.length; i++) {
      if (!checkClock(clocks[i])) return { error: errorBox("Invalid input", clocks[i] + ": " + CLOCK_HINT) };
    }
    var parsed = parseCase(inputs.case);
    if (!parsed.ok) {
      return {
        error: errorBox("Invalid case JSON", "Line " + parsed.line + ", column " + parsed.col + ": " + parsed.message,
          h("div", { className: "actions" }, [
            h("button", { type: "button", className: "btn sec sm", "data-jump": "case:" + parsed.index + ":" + parsed.index, text: "Show in Case JSON" })
          ]))
      };
    }
    return {
      path: "/api/" + action,
      body: {
        source: inputs.module,
        query: inputs.query.trim(),
        case: parsed.value,
        validAt: inputs.validAt.trim(),
        knownAt: inputs.knownAt.trim()
      }
    };
  }

  function runAction(action) {
    if (busy || !state) return;
    var inputs = snapshot();
    var request = buildRequest(action, inputs);
    if (request.error) {
      showMessage(request.error);
      return;
    }
    setBusy(action, true);
    postJson(request.path, request.body).then(function (res) {
      if (res.network) {
        networkDown();
        showMessage(errorBox("No connection", NETWORK_MESSAGE));
        return;
      }
      serverAnswered();
      var entry = makeEntry(action, inputs, res);
      if (action === "check" && res.status === 200) noteDiagnostics(inputs.module, res.data);
      historyEntries.unshift(entry);
      if (historyEntries.length > HISTORY_LIMIT) historyEntries.length = HISTORY_LIMIT;
      renderHistory();
      // An answer for inputs the user has since changed is history, not the current result.
      var current = sameInputs(inputs, snapshot());
      showEntry(entry, !current);
      if (current) revealResult();
    }).catch(function (err) {
      showMessage(errorBox("The page could not show this result", String(err && err.message ? err.message : err)));
    }).then(function () {
      setBusy(action, false);
      if (queuedAction) {
        var next = queuedAction;
        queuedAction = null;
        runAction(next);
      }
    });
  }

  /** Show a message that is not a history entry (an input problem or a lost connection). */
  function showMessage(node) {
    shown = null;
    setViewsEnabled(true);
    showNodes([node]);
    markHistory();
    revealResult();
  }

  /** On a phone the result sits below the editor; scroll to it when it is out of sight. */
  function revealResult() {
    var top = el.resultBody.getBoundingClientRect().top;
    if (top > window.innerHeight - 120) el.resultBody.scrollIntoView({ block: "start" });
  }

  /** Sort a response into what the result area shows for it. */
  function entryKind(action, res) {
    var data = isObject(res.data) ? res.data : null;
    if (!data) return "unexpected";
    if (data.kind === "engineError") return "engine";
    if (res.status >= 500) return "server";
    if (action === "check") return res.status === 200 && typeof data.ok === "boolean" ? "check" : "unexpected";
    if (action === "render") {
      if (data.ok === true && typeof data.text === "string") return "rendered";
      return data.ok === false && typeof data.error === "string" ? "renderError" : "unexpected";
    }
    if (data.ok === true && isObject(data.report)) return "report";
    if (data.error === "check failed" && Array.isArray(data.diagnostics)) return "diagnostics";
    return data.ok === false && typeof data.error === "string" ? "invalid" : "unexpected";
  }

  function makeEntry(action, inputs, res) {
    var entry = {
      id: String(++entrySeq),
      action: action,
      time: new Date(),
      inputs: inputs,
      query: action === "run" || action === "explore" ? inputs.query.trim() : "",
      status: res.status,
      data: res.data,
      text: res.text,
      kind: entryKind(action, res)
    };
    entry.stamp = entryStamp(entry);
    return entry;
  }

  /** `{ cls, label, short }` of the stamp an entry shows in the history list. */
  function entryStamp(entry) {
    switch (entry.kind) {
      case "report":
        return kindInfo(outcomeOf(entry).kind);
      case "check":
        return entry.data.ok ? { cls: "det", label: "ok", short: "ok" } : { cls: "inc", label: "Check failed", short: "Fail" };
      case "diagnostics":
        return { cls: "inc", label: "Check failed", short: "Fail" };
      case "rendered":
        return { cls: "det", label: "Rendered", short: "ok" };
      case "renderError":
        return { cls: "inc", label: "Render failed", short: "Fail" };
      default:
        return { cls: "inc", label: "Error", short: "Error" };
    }
  }

  function outcomeOf(entry) {
    var doc = entry.data.report.outcomeDocument;
    return isObject(doc) && isObject(doc.outcome) ? doc.outcome : {};
  }

  function entryLabel(entry) {
    return entry.action + (entry.query ? " " + entry.query : "");
  }

  function findEntry(id) {
    for (var i = 0; i < historyEntries.length; i++) {
      if (historyEntries[i].id === id) return historyEntries[i];
    }
    return null;
  }

  // ----------------------------------------------------------------- history

  function renderHistory() {
    var active = document.activeElement;
    var focusedId = active && el.history.contains(active) ? active.getAttribute("data-history") : null;
    clear(el.history);
    if (historyEntries.length === 0) {
      el.history.appendChild(h("li", { className: "mill-none", text: "Nothing run yet" }));
      return;
    }
    historyEntries.forEach(function (entry) {
      el.history.appendChild(h("li", null, [
        h("button", { type: "button", className: "mill-hist", "data-history": entry.id }, [
          h("time", { className: "mill-hist-time", datetime: entry.time.toISOString(), text: clock(entry.time) }),
          h("span", { className: "mill-hist-what", text: entryLabel(entry) }),
          stampEl(entry.stamp.cls, entry.stamp.short, entry.stamp.label)
        ])
      ]));
    });
    markHistory();
    if (focusedId) {
      var again = el.history.querySelector("[data-history=\"" + focusedId + "\"]") || el.history.querySelector("[data-history]");
      if (again) again.focus({ preventScroll: true });
    }
  }

  function markHistory() {
    Array.prototype.forEach.call(el.history.querySelectorAll("[data-history]"), function (button) {
      var on = shown !== null && button.getAttribute("data-history") === shown.id;
      if (on) button.setAttribute("aria-current", "true");
      else button.removeAttribute("aria-current");
    });
  }

  function restoreInputs(entry) {
    hideConfirm();
    state = copyState(Object.assign({}, entry.inputs, { buffer: state.buffer, view: state.view }));
    saveState();
    applyInputs();
    markSample();
    editorChanged("load");
  }

  /** Restore an entry's inputs, asking first when that would throw away the user's edits. */
  function askRestore(entry) {
    if (!isDirty() || sameInputs(entry.inputs, snapshot())) {
      restoreInputs(entry);
      return;
    }
    hideConfirm();
    pendingEntry = entry;
    document.getElementById("confirm-text").textContent = "Replace your edits with the inputs of " + entryLabel(entry) + "?";
    el.confirm.hidden = false;
    el.confirmCancel.focus();
  }

  function focusRestore() {
    var button = el.resultBody.querySelector("[data-restore]");
    if (button) button.focus();
  }

  // ------------------------------------------------------------------ result

  function showNodes(nodes) {
    clearTimeout(busyTimer);
    clear(el.resultBody);
    nodes.forEach(function (node) { el.resultBody.appendChild(node); });
  }

  function setView(view) {
    state.view = VIEWS.indexOf(view) >= 0 ? view : "opinion";
    saveState();
    Array.prototype.forEach.call(el.views.querySelectorAll("[data-view]"), function (button) {
      button.setAttribute("aria-pressed", button.getAttribute("data-view") === state.view ? "true" : "false");
    });
    if (shown && shown.kind === "report") showEntry(shown, shownFromHistory);
  }

  /** The view switch only applies to evaluation reports. */
  function setViewsEnabled(on) {
    Array.prototype.forEach.call(el.views.querySelectorAll("[data-view]"), function (button) {
      button.disabled = !on;
    });
  }

  function showEntry(entry, fromHistory) {
    shown = entry;
    shownFromHistory = fromHistory;
    setViewsEnabled(entry.kind === "report");
    var nodes = [];
    if (fromHistory) {
      nodes.push(h("div", { className: "mill-from" }, [
        h("p", { className: "label", text: "From history · " + clock(entry.time) + " · " + entryLabel(entry) }),
        h("button", { type: "button", className: "btn sec sm", "data-restore": entry.id, text: "Restore inputs" })
      ]));
    }
    nodes.push(entryBody(entry));
    showNodes(nodes);
    markHistory();
  }

  function entryBody(entry) {
    var data = entry.data;
    switch (entry.kind) {
      case "report":
        if (state.view === "table") return tableView(data.report);
        if (state.view === "json") return codeFrame("json", JSON.stringify(data.report, null, 2));
        return opinionView(data);
      case "check":
        if (data.ok) return h("div", { className: "mill-ok" }, [stampEl("det", "ok"), h("p", { text: "No diagnostics." })]);
        return diagnosticsView(data.diagnostics, entry.inputs.module);
      case "diagnostics":
        return diagnosticsView(data.diagnostics, entry.inputs.module);
      case "engine":
        return errorBox("Engine error · " + str(data.error), str(data.message));
      case "server":
        return errorBox("Server error", (typeof data.error === "string" ? data.error : "The server could not finish this request.") +
          " (HTTP " + entry.status + ")");
      case "invalid":
        return errorBox("Invalid input", data.error);
      case "rendered":
        return h("article", { className: "mill-doc", "aria-label": "Rendered text" }, [
          h("p", { className: "mill-doc-cap", text: "Rendered template" }),
          h("pre", { className: "mill-rendered-text", text: data.text })
        ]);
      case "renderError":
        return errorBox("Render failed", data.error);
      default:
        return errorBox("Unexpected response", "The mill answered HTTP " + entry.status +
          (entry.text ? ": " + entry.text.slice(0, 300) : "."));
    }
  }

  function errorBox(title, message, extra) {
    return h("div", { className: "mill-error", role: "alert" }, [
      h("p", { className: "mill-error-title", text: title }),
      h("p", { className: "mill-error-msg", text: message }),
      extra || null
    ]);
  }

  /** The printed-opinion view: caption, outcome title, the server's sentences, then module and valid time. */
  function opinionView(data) {
    var report = data.report;
    var doc = isObject(report.outcomeDocument) ? report.outcomeDocument : {};
    var outcome = isObject(doc.outcome) ? doc.outcome : {};
    var asOf = isObject(doc.asOf) ? doc.asOf : {};
    var kind = kindInfo(outcome.kind);
    var sentences = Array.isArray(data.opinion) ? data.opinion.filter(function (s) { return typeof s === "string"; }) : [];
    var article = h("article", { className: "mill-doc", "aria-label": "Opinion" }, [
      h("p", { className: "mill-doc-cap", text: "Evaluation report · " + str(report.executionMode) + " · " + str(report.sourceTrust) }),
      h("h3", { className: "mill-doc-title" }, [h("span", { className: "mill-mark " + kind.cls, "aria-hidden": "true" }), kind.label])
    ]);
    sentences.forEach(function (sentence, i) {
      var boundary = i === sentences.length - 1 && sentence.indexOf("Outside scope: ") === 0;
      article.appendChild(h("p", { className: boundary ? "mill-doc-boundary" : null, text: sentence }));
    });
    if (sentences.length === 0) {
      article.appendChild(h("p", { className: "mill-doc-boundary", text: "The server sent no sentences for this report; see Table or JSON." }));
    }
    article.appendChild(h("footer", { className: "mill-doc-sig" }, [
      h("span", { text: str(doc.module) }),
      h("span", { text: "as of " + str(asOf.validTime) })
    ]));
    return article;
  }

  /** JSON on one line with a space after each comma and colon, so table cells can wrap. */
  function spaced(value) {
    var text = JSON.stringify(value, null, 1);
    return text === undefined ? String(value) : text.replace(/\n\s*/g, " ");
  }

  function completionLabel(key) {
    return key.replace(/^[a-z]:/, "").split("=").join(" = ");
  }

  function pivotNames(pivots) {
    var names = [];
    (Array.isArray(pivots) ? pivots : []).forEach(function (pivot) {
      if (!isObject(pivot)) return;
      var name = typeof pivot.family === "string" ? pivot.family
        : typeof pivot.protocol === "string" ? pivot.protocol : str(pivot.kind);
      if (names.indexOf(name) < 0) names.push(name);
    });
    return names;
  }

  /** The compact-table view: every field of the report, one row each. */
  function tableView(report) {
    var doc = isObject(report.outcomeDocument) ? report.outcomeDocument : {};
    var outcome = isObject(doc.outcome) ? doc.outcome : {};
    var boundary = isObject(doc.modelBoundary) ? doc.modelBoundary : {};
    var asOf = isObject(doc.asOf) ? doc.asOf : {};
    var kind = kindInfo(outcome.kind);
    var rows = [["kind", stampEl(kind.cls, kind.label)]];
    var done = { kind: true, trace: true };
    function row(label, value) { rows.push([label, value]); }
    if ("value" in outcome) {
      row("value", valueText(outcome.value));
      done.value = true;
    }
    if (isObject(outcome.alternatives)) {
      Object.keys(outcome.alternatives).sort().forEach(function (key) {
        row(completionLabel(key), valueText(outcome.alternatives[key]));
      });
      done.alternatives = true;
    }
    if (Array.isArray(outcome.pivots)) {
      row("pivots", pivotNames(outcome.pivots).join(", ") || "none");
      done.pivots = true;
    }
    if (Array.isArray(outcome.requests)) {
      outcome.requests.forEach(function (request) { row("request", requestText(request)); });
      done.requests = true;
    }
    if ("request" in outcome) {
      row("request", requestText(outcome.request));
      done.request = true;
    }
    if (Array.isArray(outcome.doctrines)) {
      row("doctrines", doctrineNames(outcome.doctrines).join(", ") || "none");
      done.doctrines = true;
    }
    if (Array.isArray(outcome.core)) {
      row("core", outcome.core.map(str).join(", ") || "none");
      done.core = true;
    }
    Object.keys(outcome).sort().forEach(function (key) {
      if (!done[key]) row(key, typeof outcome[key] === "string" ? outcome[key] : spaced(outcome[key]));
    });
    row("query", str(doc.query));
    row("module", str(doc.module));
    row("sourceSnapshot", str(doc.sourceSnapshot));
    row("asOf", "valid " + str(asOf.validTime) + " · record " + str(asOf.recordTime));
    row("outsideScope", Array.isArray(boundary.outsideScope) && boundary.outsideScope.length
      ? boundary.outsideScope.map(str).join(", ") : "none");
    row("admissibleCompletions", spaced(boundary.admissibleCompletions));
    row("mode / trust", str(report.executionMode) + " · " + str(report.sourceTrust));
    row("verificationMethod", str(report.verificationMethod));
    row("coverage", spaced(report.coverage));
    row("assumptions", Array.isArray(report.assumptions) && report.assumptions.length
      ? spaced(report.assumptions) : "none");
    row("trace", h("span", { className: "mill-copy" }, [
      h("code", { text: str(outcome.trace) }),
      h("button", { type: "button", className: "copy", "data-copy": "", text: "Copy" })
    ]));
    return h("div", { className: "table-wrap", tabindex: "0", role: "region", "aria-label": "Report fields" }, [
      h("table", { className: "mill-table" }, [
        h("tbody", null, rows.map(function (r) {
          return h("tr", null, [h("th", { scope: "row", text: r[0] }), h("td", null, [r[1]])]);
        }))
      ])
    ]);
  }

  /** A framed code block with a Copy button, as on the site. */
  function codeFrame(lang, source) {
    var body = h("code");
    body.textContent = source;
    return h("figure", { className: "code", "data-lang": lang }, [
      h("figcaption", null, [
        h("span", { text: lang }),
        h("button", { type: "button", className: "copy", "data-copy": "", text: "Copy" })
      ]),
      h("pre", null, [body])
    ]);
  }

  /** A failed check: each diagnostic with its code, message, line:column, and a Jump button. */
  function diagnosticsView(list, source) {
    var items = Array.isArray(list) ? list.filter(isObject) : [];
    var ol = h("ol", { className: "mill-diag-list" });
    items.forEach(function (d) {
      var span = isObject(d.primary_span) ? d.primary_span : null;
      var at = null;
      if (span && typeof span.start === "number") {
        var from = byteToIndex(source, span.start);
        var to = byteToIndex(source, typeof span.end === "number" ? span.end : span.start);
        at = { from: from, to: Math.max(from, to), pos: lineCol(source, from) };
      }
      ol.appendChild(h("li", null, [
        h("code", { className: "mill-diag-code", text: str(d.code) }),
        h("span", { className: "mill-diag-msg", text: str(d.message) }),
        at ? h("span", { className: "mill-diag-at", text: at.pos.line + ":" + at.pos.col }) : null,
        at ? h("button", { type: "button", className: "btn sec sm", "data-jump": "module:" + at.from + ":" + at.to, text: "Jump" }) : null,
        typeof d.suggestion === "string" && d.suggestion ? h("span", { className: "mill-diag-hint", text: d.suggestion }) : null
      ]));
    });
    return h("div", { className: "mill-diags" }, [
      h("p", { className: "mill-res-head" }, [
        stampEl("inc", "Check failed"),
        h("span", { className: "label", text: items.length === 1 ? "1 problem" : items.length + " problems" })
      ]),
      ol
    ]);
  }

  // -------------------------------------------------------------------- copy

  function copyFrom(button) {
    var box = button.closest(".code, .mill-copy");
    var code = box && box.querySelector("code");
    if (!code) return;
    writeClipboard(code.textContent).then(function (ok) {
      if (!ok) return;
      button.setAttribute("data-copied", "");
      button.textContent = "Copied";
      setTimeout(function () {
        button.removeAttribute("data-copied");
        button.textContent = "Copy";
      }, 1400);
    });
  }

  function writeClipboard(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(text).then(function () { return true; }, function () { return legacyCopy(text); });
    }
    return Promise.resolve(legacyCopy(text));
  }

  function legacyCopy(text) {
    var active = document.activeElement;
    var area = h("textarea", { className: "sr-only", "aria-hidden": "true", tabindex: "-1" });
    area.value = text;
    document.body.appendChild(area);
    area.select();
    var ok = false;
    try {
      ok = document.execCommand("copy");
    } catch (err) {
      ok = false;
    }
    document.body.removeChild(area);
    if (active && active.focus) active.focus();
    return ok;
  }
})();
