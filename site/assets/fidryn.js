// Fidryn site enhancements: theme, copy buttons, heading links, the guides
// drawer, docs search, the on-this-page scrollspy, and the landing specimen.
// Every page works without this file; it only enhances.
(function () {
  "use strict";

  var MAX_RESULTS = 8;
  var WEIGHT_HEADING = 10;
  var WEIGHT_PAGE = 4;
  var WEIGHT_TEXT = 1;

  // Lowercased search words: runs of [a-z0-9_]. Everything else separates
  // words, so regex metacharacters in a query are plain separators.
  function tokens(query) {
    return String(query == null ? "" : query)
      .toLowerCase()
      .split(/[^a-z0-9_]+/)
      .filter(function (t) { return t.length > 0; });
  }

  function hasPrefix(words, token) {
    for (var i = 0; i < words.length; i++) {
      if (words[i].lastIndexOf(token, 0) === 0) return true;
    }
    return false;
  }

  // Sum of field weights over every token; 0 when any token matches no word.
  function score(entry, qs) {
    if (!entry || !qs || qs.length === 0) return 0;
    var h = tokens(entry.h);
    var p = tokens(entry.p);
    var t = tokens(entry.t);
    var total = 0;
    for (var i = 0; i < qs.length; i++) {
      var s = 0;
      if (hasPrefix(h, qs[i])) s += WEIGHT_HEADING;
      if (hasPrefix(p, qs[i])) s += WEIGHT_PAGE;
      if (hasPrefix(t, qs[i])) s += WEIGHT_TEXT;
      if (s === 0) return 0;
      total += s;
    }
    return total;
  }

  // Up to eight entries with a positive score, best first; ties keep index order.
  function rank(index, query) {
    var qs = tokens(query);
    if (qs.length === 0 || !Array.isArray(index)) return [];
    var hits = [];
    for (var i = 0; i < index.length; i++) {
      var s = score(index[i], qs);
      if (s > 0) hits.push({ entry: index[i], score: s, order: i });
    }
    hits.sort(function (a, b) { return b.score - a.score || a.order - b.order; });
    return hits.slice(0, MAX_RESULTS).map(function (hit) { return hit.entry; });
  }

  if (typeof document === "undefined") {
    module.exports = { tokens: tokens, score: score, rank: rank };
    return;
  }

  var root = document.documentElement;
  var THEME_KEY = "fidryn-theme";
  var PHONE = "(max-width: 719px)";
  var WIDE = "(min-width: 720px)";

  function all(selector, scope) {
    return Array.prototype.slice.call((scope || document).querySelectorAll(selector));
  }

  function onMedia(mq, fn) {
    if (mq.addEventListener) mq.addEventListener("change", fn);
    else if (mq.addListener) mq.addListener(fn);
  }

  function isTextField(el) {
    if (!el) return false;
    if (el.isContentEditable) return true;
    var tag = el.tagName;
    return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
  }

  // ---- clipboard ----

  function copyWithSelection(text) {
    var area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.top = "-1000px";
    document.body.appendChild(area);
    area.select();
    var ok = false;
    try { ok = document.execCommand("copy"); } catch (e) { ok = false; }
    document.body.removeChild(area);
    return ok;
  }

  function copyText(text) {
    var fallback = function () {
      if (!copyWithSelection(text)) throw new Error("copy failed");
    };
    if (navigator.clipboard && window.isSecureContext) {
      return navigator.clipboard.writeText(text).catch(fallback);
    }
    return new Promise(function (resolve) { fallback(); resolve(); });
  }

  function flagCopied(el, label) {
    if (el._copiedTimer) clearTimeout(el._copiedTimer);
    var original = el._copyLabel || el.textContent;
    el._copyLabel = original;
    el.setAttribute("data-copied", "");
    if (label) el.textContent = label;
    el._copiedTimer = setTimeout(function () {
      el.removeAttribute("data-copied");
      if (label) el.textContent = original;
      el._copiedTimer = null;
    }, 1400);
  }

  function setupCopy() {
    all("[data-copy]").forEach(function (button) {
      var host = button.closest(".code, .cmd");
      var code = host && host.querySelector("code");
      if (!code) return;
      button.hidden = false;
      button.addEventListener("click", function () {
        copyText(code.textContent).then(function () { flagCopied(button, "Copied"); announce("Copied"); }, function () {});
      });
    });
    all(".anchor").forEach(function (anchor) {
      anchor.addEventListener("click", function () {
        copyText(anchor.href).then(function () { flagCopied(anchor, null); announce("Link copied"); }, function () {});
      });
    });
  }

  // ---- status announcements ----

  var statusRegion = null;
  var statusTimer = null;

  // One polite live region, created at start so later changes are announced.
  function setupStatus() {
    statusRegion = document.createElement("div");
    statusRegion.className = "sr-only";
    statusRegion.setAttribute("role", "status");
    document.body.appendChild(statusRegion);
  }

  // Announce results that are otherwise only visual; the text clears after two
  // seconds so the same message can be announced again later.
  function announce(text) {
    if (!statusRegion || !text || statusRegion.textContent === text) return;
    statusRegion.textContent = text;
    if (statusTimer) clearTimeout(statusTimer);
    statusTimer = setTimeout(function () {
      statusRegion.textContent = "";
      statusTimer = null;
    }, 2000);
  }

  // ---- theme ----

  function setupTheme() {
    var toggles = all("[data-theme-toggle]");
    if (toggles.length === 0) return;
    var dark = window.matchMedia ? window.matchMedia("(prefers-color-scheme: dark)") : null;
    function current() {
      var t = root.getAttribute("data-theme");
      if (t === "light" || t === "dark") return t;
      return dark && dark.matches ? "dark" : "light";
    }
    var metas = all("meta[name='theme-color']");
    function paint() {
      var label = current() === "dark" ? "Switch to light theme" : "Switch to dark theme";
      toggles.forEach(function (b) { b.setAttribute("aria-label", label); });
      // An explicit choice overrides the system scheme, so the browser chrome follows it too.
      if (root.hasAttribute("data-theme")) {
        var paper = window.getComputedStyle(root).getPropertyValue("--paper").trim();
        if (paper) metas.forEach(function (m) { m.setAttribute("content", paper); });
      }
    }
    toggles.forEach(function (b) {
      b.hidden = false;
      b.addEventListener("click", function () {
        var next = current() === "dark" ? "light" : "dark";
        root.setAttribute("data-theme", next);
        try { localStorage.setItem(THEME_KEY, next); } catch (e) { /* storage unavailable */ }
        paint();
      });
    });
    if (dark) onMedia(dark, paint);
    paint();
  }

  // ---- guides drawer (phones) ----

  function setupDrawer() {
    var drawer = document.getElementById("drawer");
    var opener = document.querySelector("[data-drawer-open]");
    var backdrop = document.querySelector(".backdrop");
    if (!drawer || !opener) return;
    var phone = window.matchMedia(PHONE);
    opener.hidden = false;

    function isOpen() { return document.body.classList.contains("drawer-open"); }
    function focusables() {
      return all("a[href], button:not([disabled]), [tabindex]:not([tabindex='-1'])", drawer)
        .filter(function (el) { return el.offsetWidth > 0 || el.offsetHeight > 0; });
    }
    function open() {
      document.body.classList.add("drawer-open");
      if (backdrop) backdrop.hidden = false;
      opener.setAttribute("aria-expanded", "true");
      var first = drawer.querySelector("[data-drawer-close]") || focusables()[0];
      if (first) first.focus();
    }
    function close(restoreFocus) {
      if (!isOpen()) return;
      document.body.classList.remove("drawer-open");
      if (backdrop) backdrop.hidden = true;
      opener.setAttribute("aria-expanded", "false");
      if (restoreFocus) opener.focus();
    }

    opener.addEventListener("click", function () {
      if (isOpen()) close(true); else open();
    });
    all("[data-drawer-close]").forEach(function (el) {
      el.addEventListener("click", function () { close(true); });
    });
    drawer.addEventListener("click", function (e) {
      if (e.target.closest && e.target.closest("a[href]")) close(false);
    });
    document.addEventListener("keydown", function (e) {
      if (!isOpen()) return;
      if (e.key === "Escape") {
        e.preventDefault();
        close(true);
        return;
      }
      if (e.key !== "Tab") return;
      var items = focusables();
      if (items.length === 0) return;
      var first = items[0];
      var last = items[items.length - 1];
      if (!drawer.contains(document.activeElement)) {
        e.preventDefault();
        first.focus();
      } else if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    });
    onMedia(phone, function () { if (!phone.matches) close(false); });
  }

  // ---- search ----

  function highlight(target, text, qs) {
    var re = /[A-Za-z0-9_]+/g;
    var last = 0;
    var m;
    while ((m = re.exec(text)) !== null) {
      var word = m[0].toLowerCase();
      var len = 0;
      for (var i = 0; i < qs.length; i++) {
        if (qs[i].length > len && word.lastIndexOf(qs[i], 0) === 0) len = qs[i].length;
      }
      if (len === 0) continue;
      if (m.index > last) target.appendChild(document.createTextNode(text.slice(last, m.index)));
      var mark = document.createElement("mark");
      mark.textContent = text.slice(m.index, m.index + len);
      target.appendChild(mark);
      last = m.index + len;
    }
    if (last < text.length) target.appendChild(document.createTextNode(text.slice(last)));
  }

  // Only same-site paths from the index become links.
  function isSitePath(url) {
    return typeof url === "string" && /^\/(?![\/\\])/.test(url) && !/[\t\n\r]/.test(url);
  }

  function span(className, text) {
    var el = document.createElement("span");
    el.className = className;
    if (text != null) el.textContent = text;
    return el;
  }

  function setupSearch() {
    var input = document.getElementById("site-search");
    var list = document.getElementById("search-results");
    if (!input || !list) return;
    var form = input.form;
    var index = null;
    var loading = null;
    var failed = false;
    var results = [];
    var active = -1;

    function load() {
      if (loading) return loading;
      loading = fetch("/search-index.json", { credentials: "same-origin" })
        .then(function (r) {
          if (!r.ok) throw new Error("HTTP " + r.status);
          return r.json();
        })
        .then(function (data) {
          failed = false;
          index = Array.isArray(data) ? data : [];
          render();
        }, function () {
          failed = true;
          loading = null;
          render();
        });
      return loading;
    }

    function isShown() { return !list.hidden; }
    function show() {
      if (document.activeElement !== input) return;
      list.hidden = false;
      input.setAttribute("aria-expanded", "true");
    }
    function hide() {
      list.hidden = true;
      input.setAttribute("aria-expanded", "false");
      input.removeAttribute("aria-activedescendant");
      active = -1;
    }
    function message(text) {
      var li = document.createElement("li");
      li.className = "empty";
      li.setAttribute("role", "option");
      li.setAttribute("aria-disabled", "true");
      li.setAttribute("aria-selected", "false");
      li.textContent = text;
      list.appendChild(li);
    }

    function render() {
      var query = input.value;
      list.textContent = "";
      results = [];
      active = -1;
      input.removeAttribute("aria-activedescendant");
      if (query.trim() === "") {
        hide();
        return;
      }
      if (failed) {
        message("Search is unavailable right now.");
      } else if (!index) {
        message("Loading the index…");
      } else {
        var qs = tokens(query);
        results = rank(index, query).filter(function (entry) { return isSitePath(entry.u); });
        if (results.length === 0) message("No matching sections.");
        results.forEach(function (entry, i) {
          var li = document.createElement("li");
          li.id = "sr-" + i;
          li.setAttribute("role", "option");
          li.setAttribute("aria-selected", "false");
          var a = document.createElement("a");
          a.href = entry.u;
          a.tabIndex = -1;
          a.appendChild(span("n", entry.n || ""));
          var h = span("h", null);
          highlight(h, String(entry.h || ""), qs);
          a.appendChild(h);
          var page = String(entry.p || "");
          var text = String(entry.t || "");
          a.appendChild(span("t", text ? page + " · " + text : page));
          li.appendChild(a);
          li.addEventListener("mousemove", function () { if (active !== i) select(i); });
          list.appendChild(li);
        });
      }
      if (document.activeElement === input) {
        announce(failed ? "Search is unavailable right now."
          : !index ? ""
          : results.length === 0 ? "No matching sections."
          : results.length === 1 ? "1 result" : results.length + " results");
      }
      show();
    }

    function select(i) {
      var items = all("li[role='option']:not(.empty)", list);
      if (items.length === 0) return;
      active = (i + items.length) % items.length;
      items.forEach(function (li, j) { li.setAttribute("aria-selected", j === active ? "true" : "false"); });
      input.setAttribute("aria-activedescendant", items[active].id);
      var el = items[active];
      if (el.scrollIntoView) el.scrollIntoView({ block: "nearest" });
    }

    function openActive() {
      var entry = results[active >= 0 ? active : 0];
      if (!entry) return;
      hide();
      window.location.href = entry.u;
    }

    input.addEventListener("focus", function () {
      load();
      if (input.value.trim() !== "") render();
    });
    input.addEventListener("input", function () {
      if (!failed) load();
      render();
    });
    input.addEventListener("keydown", function (e) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        if (!isShown()) render();
        if (results.length === 0) return;
        e.preventDefault();
        if (e.key === "ArrowDown") select(active < 0 ? 0 : active + 1);
        else select(active < 0 ? results.length - 1 : active - 1);
      } else if (e.key === "Enter") {
        e.preventDefault();
        openActive();
      } else if (e.key === "Escape") {
        if (isShown()) {
          e.preventDefault();
          hide();
        }
      }
    });
    (form || input.parentNode).addEventListener("focusout", function (e) {
      if (!(form || input.parentNode).contains(e.relatedTarget)) hide();
    });
    list.addEventListener("mousedown", function (e) { e.preventDefault(); });
    list.addEventListener("click", function (e) {
      if (e.target.closest && e.target.closest("a")) hide();
    });
    if (form) {
      form.addEventListener("submit", function (e) {
        e.preventDefault();
        openActive();
      });
    }
    document.addEventListener("click", function (e) {
      if (isShown() && !(form || input).contains(e.target)) hide();
    });
    document.addEventListener("keydown", function (e) {
      var slash = e.key === "/" && !e.ctrlKey && !e.metaKey && !e.altKey;
      var cmdK = (e.key === "k" || e.key === "K") && (e.ctrlKey || e.metaKey) && !e.altKey;
      if (!slash && !cmdK) return;
      if (isTextField(document.activeElement)) return;
      if (document.body.classList.contains("drawer-open")) return;
      e.preventDefault();
      input.focus();
      input.select();
    });
  }

  // ---- on-this-page scrollspy ----

  function setupScrollspy() {
    var links = all(".onpage a[href^='#']");
    if (links.length === 0 || !("IntersectionObserver" in window)) return;
    var byId = {};
    var headings = [];
    links.forEach(function (a) {
      var id = a.getAttribute("href").slice(1);
      try { id = decodeURIComponent(id); } catch (e) { /* keep the raw fragment */ }
      var h = document.getElementById(id);
      if (h) {
        byId[id] = a;
        headings.push(h);
      }
    });
    if (headings.length === 0) return;
    var visible = {};
    var currentId = null;
    var bandId = null;
    function mark(id) {
      if (id === currentId) return;
      currentId = id;
      links.forEach(function (a) { a.removeAttribute("aria-current"); });
      if (id && byId[id]) byId[id].setAttribute("aria-current", "true");
    }
    // A short last section never reaches the band, so the bottom of a scrolling page marks it.
    function sync() {
      var doc = document.documentElement;
      var scrolls = doc.scrollHeight - window.innerHeight > 2;
      var bottom = scrolls && window.innerHeight + window.scrollY >= doc.scrollHeight - 2;
      mark(bottom ? headings[headings.length - 1].id : bandId);
    }
    var observer = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        var id = entry.target.id;
        if (entry.isIntersecting) {
          visible[id] = true;
        } else {
          delete visible[id];
          // Scrolling up past the current heading hands the mark to the one before it.
          if (id === bandId && entry.boundingClientRect.top > 0) {
            var i = headings.indexOf(entry.target);
            bandId = i > 0 ? headings[i - 1].id : null;
          }
        }
      });
      for (var i = 0; i < headings.length; i++) {
        if (visible[headings[i].id]) {
          bandId = headings[i].id;
          break;
        }
      }
      sync();
    }, { rootMargin: "0px 0px -70% 0px" });
    headings.forEach(function (h) { observer.observe(h); });
    window.addEventListener("scroll", sync, { passive: true });
  }

  // ---- landing specimen ----

  function setupSpecimen(box) {
    var tabs = all("[role='tab']", box);
    var panels = tabs.map(function (t) { return document.getElementById(t.getAttribute("aria-controls")); });
    if (tabs.length === 0 || panels.some(function (p) { return !p; })) return;
    var scroller = box.querySelector(".spec-panels");
    var dots = all(".spec-dots i", box);
    var wide = window.matchMedia(WIDE);
    var current = Math.max(0, tabs.findIndex(function (t) { return t.getAttribute("aria-selected") === "true"; }));

    function select(i, focus) {
      current = i;
      tabs.forEach(function (t, j) {
        t.setAttribute("aria-selected", j === i ? "true" : "false");
        t.tabIndex = j === i ? 0 : -1;
      });
      if (wide.matches) panels.forEach(function (p, j) { p.hidden = j !== i; });
      dots.forEach(function (d, j) { d.classList.toggle("on", j === i); });
      if (focus) tabs[i].focus();
    }

    function syncFromScroll() {
      if (wide.matches || !scroller) return;
      var frame = scroller.getBoundingClientRect();
      var centre = frame.left + frame.width / 2;
      var best = 0;
      var bestDistance = Infinity;
      panels.forEach(function (p, j) {
        var r = p.getBoundingClientRect();
        var d = Math.abs(r.left + r.width / 2 - centre);
        if (d < bestDistance) {
          bestDistance = d;
          best = j;
        }
      });
      if (best !== current) select(best, false);
    }

    function apply() {
      if (wide.matches) {
        select(current, false);
      } else {
        panels.forEach(function (p) { p.hidden = false; });
        if (scroller) scroller.scrollLeft = panels[current].offsetLeft - panels[0].offsetLeft;
        syncFromScroll();
        dots.forEach(function (d, j) { d.classList.toggle("on", j === current); });
      }
    }

    tabs.forEach(function (t, i) {
      t.addEventListener("click", function () { select(i, false); });
    });
    box.querySelector("[role='tablist']").addEventListener("keydown", function (e) {
      var i = tabs.indexOf(document.activeElement);
      if (i < 0) return;
      var next = null;
      if (e.key === "ArrowRight") next = (i + 1) % tabs.length;
      else if (e.key === "ArrowLeft") next = (i - 1 + tabs.length) % tabs.length;
      else if (e.key === "Home") next = 0;
      else if (e.key === "End") next = tabs.length - 1;
      if (next === null) return;
      e.preventDefault();
      select(next, true);
    });
    if (scroller) {
      var pending = false;
      scroller.addEventListener("scroll", function () {
        if (pending) return;
        pending = true;
        window.requestAnimationFrame(function () {
          pending = false;
          syncFromScroll();
        });
      }, { passive: true });
    }
    onMedia(wide, apply);
    apply();
  }

  function start() {
    [
      setupStatus,
      setupTheme,
      setupCopy,
      setupDrawer,
      setupSearch,
      setupScrollspy,
      function () { all("[data-specimen]").forEach(setupSpecimen); },
    ].forEach(function (setup) {
      // Each feature stands alone: one that throws must not stop the others.
      try { setup(); } catch (err) { if (window.console) window.console.error(err); }
    });
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
})();
