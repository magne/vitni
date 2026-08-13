/*
 * Shared chrome + keyboard/a11y layer for the Phase 5 mockups.
 *
 * A page opts into the app shell by defining `window.MOCK` and putting its
 * content in <div id="workarea">…</div>. This script builds the rail, topbar,
 * optional record-tab strip, and status bar around it with ARIA landmarks, and
 * wires the keyboard model: theme toggle, ⌘K search, `?` help overlay,
 * `g`-prefix navigation, Esc, and roving focus in the rail. Standalone doc pages
 * (index, design-system, shortcuts) omit MOCK and just get the theme bootstrap.
 *
 * This is a mockup harness only — the real chrome + shortcut dispatcher are RSX
 * in crates/vitni-ui-dioxus (plan PR1/PR2).
 */
(function () {
  "use strict";

  var NAV = {
    entities: [
      { id: "dashboard", icon: "⌂", label: "Dashboard", href: "app-shell.html", key: "d" },
      { id: "people", icon: "👤", label: "People", href: "person.html", count: "1,284", key: "p" },
      { id: "families", icon: "👪", label: "Families", href: "family.html", count: "642", key: "f" },
      { id: "events", icon: "📅", label: "Events", href: "event.html", count: "3,910", key: "e" },
      { id: "places", icon: "📍", label: "Places", href: "place.html", count: "517", key: "l" },
      { id: "sources", icon: "📚", label: "Sources", href: "source.html", count: "208", key: "s" },
      { id: "citations", icon: "❝", label: "Citations", href: "citation.html", count: "4,021", key: "c" },
      { id: "repositories", icon: "🏛", label: "Repositories", href: "repository.html", count: "37", key: "r" },
      { id: "media", icon: "🖼", label: "Media", href: "media.html", count: "896", key: "m" },
      { id: "notes", icon: "🗒", label: "Notes", href: "note.html", count: "1,150", key: "n" },
      { id: "research-notes", icon: "🧾", label: "Research notes", href: "research-note.html", count: "46", key: "a" },
      { id: "tags", icon: "🏷", label: "Tags", href: "tag.html", count: "24", key: "t" },
      { id: "dna-tests", icon: "🧬", label: "DNA tests", href: "dna-test.html", count: "12" },
      { id: "dna-matches", icon: "🔗", label: "DNA matches", href: "dna-match.html", count: "318" }
    ],
    tools: [
      { id: "pedigree", icon: "🌳", label: "Pedigree", href: "pedigree.html" },
      { id: "import", icon: "📥", label: "Import", href: "import.html" },
      { id: "export", icon: "📤", label: "Export", href: "export.html" },
      { id: "merge", icon: "⇄", label: "Compare / merge", href: "merge.html" },
      { id: "geography", icon: "🗺", label: "Geography", href: "geography.html" },
      { id: "strengths", icon: "★", label: "Why this app", href: "strengths.html" },
      { id: "shortcuts", icon: "⌨", label: "Shortcuts", href: "shortcuts.html" },
      { id: "plugins", icon: "🧩", label: "Plugins", href: "plugin-manager.html" },
      { id: "preferences", icon: "⚙", label: "Preferences", href: "preferences.html" }
    ]
  };

  // Shortcut map driving both the `?` overlay and the key handlers.
  var SHORTCUTS = [
    { group: "Global", items: [
      ["Command palette", "⌘ K"],
      ["New (context)", "⌘ N"],
      ["Find / filter", "⌘ F"],
      ["Undo / redo", "⌘ Z / ⌘⇧ Z"],
      ["Save record (when dirty)", "⌘ S"],
      ["Switch record tab", "⌘ 1…9"],
      ["Shortcut help", "?"],
      ["Close / clear", "Esc"]
    ]},
    { group: "Go to (press g, then…)", items: [
      ["Dashboard", "g d"],
      ["People", "g p"],
      ["Families", "g f"],
      ["Events", "g e"],
      ["Places", "g l"],
      ["Sources", "g s"],
      ["Citations", "g c"],
      ["Repositories", "g r"],
      ["Media", "g m"],
      ["Notes", "g n"],
      ["Research notes", "g a"],
      ["Tags", "g t"]
    ]},
    { group: "Within a screen", items: [
      ["Move selection", "↑ ↓"],
      ["Open record", "Enter"],
      ["Prev / next record", "[ / ]"],
      ["Move across tabs", "← → · Home/End"],
      ["Add source (on a fact)", "s"],
      ["Edit (on a fact)", "e"]
    ]},
    { group: "Editing a record", items: [
      ["Edit focused field", "e / F2"],
      ["Commit field to draft", "Enter"],
      ["Commit + next / prev field", "Tab / ⇧Tab"],
      ["Reset field to original", "⌘ ⌫"],
      ["Cancel field / record", "Esc"],
      ["Save record", "⌘ S"]
    ]}
  ];

  function el(tag, cls, html) {
    var n = document.createElement(tag);
    if (cls) n.className = cls;
    if (html != null) n.innerHTML = html;
    return n;
  }
  function attr(node, map) { Object.keys(map).forEach(function (k) { node.setAttribute(k, map[k]); }); return node; }

  function applyTheme(t) {
    document.documentElement.setAttribute("data-theme", t);
    try { localStorage.setItem("vitni-mockup-theme", t); } catch (e) {}
  }
  function initTheme() {
    var t = null;
    try { t = localStorage.getItem("vitni-mockup-theme"); } catch (e) {}
    if (!t) {
      t = window.matchMedia && window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
    }
    applyTheme(t);
  }

  function navItem(item, active) {
    var n = el("a", "nav-item" + (item.id === active ? " active" : ""));
    n.href = item.href || "#";
    if (item.id === active) n.setAttribute("aria-current", "page");
    n.innerHTML =
      '<span class="ico" aria-hidden="true">' + item.icon + "</span>" +
      "<span>" + item.label + "</span>" +
      (item.count ? '<span class="count" aria-hidden="true">' + item.count + "</span>" : "");
    if (item.count) n.setAttribute("aria-label", item.label + " (" + item.count + ")");
    return n;
  }

  function buildRail(active) {
    var rail = el("aside", "rail");
    attr(rail, { role: "navigation", "aria-label": "Primary" });
    rail.appendChild(el("div", "brand", '<span class="logo" aria-hidden="true">V</span><span>Vitni</span>'));
    var nav = el("nav");
    nav.appendChild(attr(el("div", "nav-group-label", "Entities"), { id: "grp-entities" }));
    var entWrap = attr(el("div"), { "aria-label": "Entities" });
    NAV.entities.forEach(function (i) { entWrap.appendChild(navItem(i, active)); });
    nav.appendChild(entWrap);
    nav.appendChild(attr(el("div", "nav-sep"), { "aria-hidden": "true" }));
    nav.appendChild(attr(el("div", "nav-group-label", "Tools"), { id: "grp-tools" }));
    var toolWrap = attr(el("div"), { "aria-label": "Tools" });
    NAV.tools.forEach(function (i) { toolWrap.appendChild(navItem(i, active)); });
    nav.appendChild(toolWrap);
    rail.appendChild(nav);

    // Roving focus: ↑/↓ move between nav items while focus is in the rail.
    rail.addEventListener("keydown", function (ev) {
      if (ev.key !== "ArrowDown" && ev.key !== "ArrowUp") return;
      var items = Array.prototype.slice.call(rail.querySelectorAll(".nav-item"));
      var idx = items.indexOf(document.activeElement);
      if (idx === -1) return;
      ev.preventDefault();
      var next = ev.key === "ArrowDown" ? Math.min(idx + 1, items.length - 1) : Math.max(idx - 1, 0);
      items[next].focus();
    });
    return rail;
  }

  function buildTopbar(cfg) {
    var top = el("header", "topbar");
    attr(top, { role: "banner" });
    var crumbs = (cfg.crumb || []).map(function (c, i, arr) {
      var last = i === arr.length - 1;
      return (last ? "<b>" + c + "</b>" : c) + (last ? "" : ' <span class="sep" aria-hidden="true">›</span> ');
    }).join("");
    var bc = el("nav", "breadcrumb", crumbs);
    attr(bc, { "aria-label": "Breadcrumb" });
    top.appendChild(bc);

    var search = el("div", "search",
      '<span aria-hidden="true">🔍</span>' +
      '<label class="sr-only" for="global-search">Search</label>' +
      '<input id="global-search" placeholder="Search people, places, sources…">' +
      '<kbd aria-hidden="true">⌘K</kbd>');
    attr(search, { role: "search" });
    top.appendChild(search);

    var theme = el("button", "icon-btn", "◐");
    attr(theme, { type: "button", "aria-label": "Toggle light or dark theme", title: "Toggle light / dark" });
    theme.onclick = function () {
      applyTheme(document.documentElement.getAttribute("data-theme") === "dark" ? "light" : "dark");
    };
    top.appendChild(theme);

    var help = el("button", "icon-btn", "?");
    attr(help, { type: "button", "aria-label": "Keyboard shortcuts", title: "Keyboard shortcuts (?)" });
    help.onclick = toggleHelp;
    top.appendChild(help);
    return top;
  }

  function buildTabstrip(tabs) {
    var strip = el("div", "tabstrip");
    attr(strip, { role: "tablist", "aria-label": "Open records" });
    // Tabs live in a `.tabs-scroll` viewport so they scroll horizontally on overflow instead of
    // pushing the window wider; the new-record button stays outside it, always visible.
    var scroll = el("div", "tabs-scroll");
    tabs.forEach(function (t) {
      // `unsaved: true` marks a tab holding work that closing would discard — an unsaved draft, or a
      // saved record with an in-progress edit. The dot glyph and the aria-label carry the state; the
      // tint is only reinforcement (WCAG 1.4.1).
      var rt = el("div", "rtab" + (t.active ? " active" : "") + (t.unsaved ? " unsaved" : ""));
      attr(rt, { role: "tab", tabindex: t.active ? "0" : "-1", "aria-selected": t.active ? "true" : "false" });
      if (t.unsaved) attr(rt, { "aria-label": t.label + " — unsaved changes" });
      rt.innerHTML =
        (t.unsaved ? '<span class="unsaved-dot" aria-hidden="true">●</span>' : "") + "<span>" + t.label + "</span>";
      var close = el("span", "close", "✕");
      attr(close, { role: "button", tabindex: "0", "aria-label": "Close " + t.label });
      function closeTab(ev) {
        ev.stopPropagation();
        var wasActive = rt.classList.contains("active");
        var next = rt.nextElementSibling, prev = rt.previousElementSibling;
        rt.remove();
        if (!wasActive) return;
        var neighbor = (next && next.classList.contains("rtab") && !next.classList.contains("add")) ? next : prev;
        if (neighbor && neighbor.classList.contains("rtab")) {
          neighbor.classList.add("active");
          neighbor.setAttribute("aria-selected", "true");
          neighbor.setAttribute("tabindex", "0");
        }
      }
      close.addEventListener("click", closeTab);
      close.addEventListener("keydown", function (ev) {
        if (ev.key === "Enter" || ev.key === " ") { ev.preventDefault(); closeTab(ev); }
      });
      rt.appendChild(close);
      scroll.appendChild(rt);
    });
    strip.appendChild(scroll);
    var add = el("div", "rtab add");
    attr(add, { role: "button", tabindex: "0", "aria-label": "Open another record" });
    add.textContent = "+";
    strip.appendChild(add);
    return strip;
  }

  function buildStatus(cfg) {
    var sb = el("footer", "statusbar");
    attr(sb, { role: "contentinfo", "aria-label": "Status" });
    sb.innerHTML =
      '<span>Workspace: <b>family-tree</b></span>' +
      (cfg.record ? '<span>Active: <span class="active-record">' + cfg.record + "</span></span>" : "") +
      '<span class="sb-right"><span>SQLite</span><span>en · dark</span><span>v0.5-dev</span></span>';
    return sb;
  }

  function wireTabs(scope) {
    scope.querySelectorAll(".tabs").forEach(function (tabs) {
      attr(tabs, { role: "tablist" });
      var tabEls = Array.prototype.slice.call(tabs.querySelectorAll(".tab"));
      tabEls.forEach(function (tab, i) {
        var id = tab.getAttribute("data-tab");
        attr(tab, { role: "tab", tabindex: tab.classList.contains("active") ? "0" : "-1",
          "aria-selected": tab.classList.contains("active") ? "true" : "false", "aria-controls": "pane-" + id, id: "tab-" + id });
        function activate() {
          tabEls.forEach(function (t) { t.classList.remove("active"); t.setAttribute("aria-selected", "false"); t.setAttribute("tabindex", "-1"); });
          tab.classList.add("active"); tab.setAttribute("aria-selected", "true"); tab.setAttribute("tabindex", "0");
          var host = tabs.closest(".detail") || document;
          host.querySelectorAll(".tab-pane").forEach(function (p) {
            p.classList.toggle("active", p.getAttribute("data-pane") === id);
          });
        }
        tab.addEventListener("click", activate);
        tab.addEventListener("keydown", function (ev) {
          var ni = null;
          if (ev.key === "ArrowRight") ni = Math.min(i + 1, tabEls.length - 1);
          else if (ev.key === "ArrowLeft") ni = Math.max(i - 1, 0);
          else if (ev.key === "Home") ni = 0;
          else if (ev.key === "End") ni = tabEls.length - 1;
          else if (ev.key === "Enter" || ev.key === " ") { ev.preventDefault(); activate(); return; }
          if (ni !== null) { ev.preventDefault(); tabEls[ni].focus(); tabEls[ni].click(); }
        });
      });
    });
    // Tie tabpanels to their tab for screen readers.
    scope.querySelectorAll(".tab-pane").forEach(function (p) {
      var id = p.getAttribute("data-pane");
      attr(p, { role: "tabpanel", id: "pane-" + id, "aria-labelledby": "tab-" + id, tabindex: "0" });
    });
  }

  // ---- Help (`?`) overlay ----
  var helpEl = null, lastFocus = null, inertedApp = null;
  var FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), ' +
    'textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
  function trapTab(container, ev) {
    if (ev.key !== "Tab") return;
    var items = Array.prototype.slice.call(container.querySelectorAll(FOCUSABLE));
    if (!items.length) return;
    var first = items[0], last = items[items.length - 1];
    if (ev.shiftKey && document.activeElement === first) { ev.preventDefault(); last.focus(); }
    else if (!ev.shiftKey && document.activeElement === last) { ev.preventDefault(); first.focus(); }
  }
  function buildHelp() {
    var ov = el("div", "overlay");
    attr(ov, { role: "presentation" });
    var sheet = el("div", "help-sheet");
    attr(sheet, { role: "dialog", "aria-modal": "true", "aria-label": "Keyboard shortcuts" });
    var head = el("div", "h-head", "<h3>Keyboard shortcuts</h3>");
    var close = el("button", "btn sm", "Close (Esc)");
    attr(close, { type: "button", "aria-label": "Close shortcuts" });
    close.onclick = toggleHelp;
    var sp = el("span", "spacer"); head.appendChild(sp); head.appendChild(close);
    sheet.appendChild(head);
    var body = el("div", "h-body");
    SHORTCUTS.forEach(function (g) {
      var col = el("div", "shortcut-list");
      col.appendChild(el("h4", null, g.group));
      g.items.forEach(function (it) {
        var row = el("div", "shortcut-row");
        row.innerHTML = "<span>" + it[0] + '</span><span class="keys">' +
          it[1].split(" ").map(function (k) { return "<kbd>" + k + "</kbd>"; }).join(" ") + "</span>";
        col.appendChild(row);
      });
      body.appendChild(col);
    });
    sheet.appendChild(body);
    sheet.addEventListener("keydown", function (ev) { trapTab(sheet, ev); });
    ov.appendChild(sheet);
    ov.addEventListener("click", function (e) { if (e.target === ov) toggleHelp(); });
    ov._close = close;
    return ov;
  }
  function toggleHelp() {
    if (helpEl) {
      helpEl.remove(); helpEl = null;
      if (inertedApp) { inertedApp.removeAttribute("inert"); inertedApp.removeAttribute("aria-hidden"); inertedApp = null; }
      if (lastFocus) lastFocus.focus();
      return;
    }
    lastFocus = document.activeElement;
    helpEl = buildHelp();
    // Appended to <body> (a sibling of .app, not a descendant) so inerting .app below
    // cannot also inert the overlay itself; .overlay has no positioned ancestor either
    // way, so this doesn't change its layout.
    document.body.appendChild(helpEl);
    inertedApp = document.querySelector(".app");
    if (inertedApp) { inertedApp.setAttribute("inert", ""); inertedApp.setAttribute("aria-hidden", "true"); }
    helpEl._close.focus();
  }

  function isTyping(t) {
    return t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
  }

  var gPending = false, gTimer = null;
  function initKeyboard() {
    document.addEventListener("keydown", function (ev) {
      // Esc always closes the help overlay / clears.
      if (ev.key === "Escape") {
        if (helpEl) { toggleHelp(); ev.preventDefault(); }
        else if (isTyping(document.activeElement)) document.activeElement.blur();
        return;
      }
      if (isTyping(document.activeElement)) return;

      // ⌘K / Ctrl+K → focus global search (palette in the real app).
      if ((ev.metaKey || ev.ctrlKey) && (ev.key === "k" || ev.key === "K")) {
        var s = document.getElementById("global-search");
        if (s) { ev.preventDefault(); s.focus(); }
        return;
      }
      if (ev.metaKey || ev.ctrlKey || ev.altKey) return;

      if (ev.key === "?") { ev.preventDefault(); toggleHelp(); return; }

      // g-prefix navigation: press g, then a category key.
      if (gPending) {
        gPending = false; clearTimeout(gTimer);
        var dest = null;
        NAV.entities.forEach(function (i) { if (i.key === ev.key) dest = i.href; });
        if (dest) { ev.preventDefault(); window.location.href = dest; }
        return;
      }
      if (ev.key === "g") { gPending = true; gTimer = setTimeout(function () { gPending = false; }, 1200); }
    });
  }

  // Arrow-key roving selection for role="radio" groups, Space/Enter toggle for
  // role="switch" — delegated on document so it works on every page (shell-driven
  // or standalone, e.g. design-system.html, preferences.html, plugin-manager.html)
  // without needing per-page wiring. Mirrors wireTabs' arrow-key pattern.
  function initRadioSwitch() {
    document.addEventListener("keydown", function (ev) {
      var t = ev.target;
      if (!t || !t.getAttribute) return;
      var role = t.getAttribute("role");
      if (role === "radio") {
        var group = t.closest('[role="radiogroup"]');
        if (!group) return;
        var items = Array.prototype.slice.call(group.querySelectorAll('[role="radio"]'));
        var idx = items.indexOf(t);
        if (idx === -1) return;
        var ni = null;
        if (ev.key === "ArrowLeft" || ev.key === "ArrowUp") ni = (idx - 1 + items.length) % items.length;
        else if (ev.key === "ArrowRight" || ev.key === "ArrowDown") ni = (idx + 1) % items.length;
        if (ni === null) return;
        ev.preventDefault();
        items.forEach(function (it) { it.setAttribute("aria-checked", "false"); it.setAttribute("tabindex", "-1"); });
        items[ni].setAttribute("aria-checked", "true");
        items[ni].setAttribute("tabindex", "0");
        items[ni].focus();
      } else if (role === "switch") {
        if (ev.key !== " " && ev.key !== "Enter") return;
        ev.preventDefault();
        t.setAttribute("aria-checked", t.getAttribute("aria-checked") === "true" ? "false" : "true");
      }
    });
  }

  function build() {
    initTheme();
    initKeyboard();
    initRadioSwitch();
    var cfg = window.MOCK;
    if (!cfg) { return; } // standalone doc page

    var work = document.getElementById("workarea");
    var app = el("div", "app");

    // Two shell shapes: an entity category shows `rail | Explorer (list) | work-area` with a record
    // tabstrip; a tool/Dashboard/Help destination shows `rail | work-area` (no list, no tabstrip).
    var isEntity = cfg.active && NAV.entities.some(function (e) {
      return e.id === cfg.active && e.id !== "dashboard";
    });

    var skip = el("a", "skip-link", "Skip to content");
    skip.href = "#main";
    app.appendChild(skip);

    app.appendChild(buildRail(cfg.active));

    // Hoist the entity list out of the page's master-detail into a shell-level Explorer column so the
    // work area holds only the editor (detail) pane — mirroring the app's `Explorer` + editor host.
    if (isEntity && work) {
      var listEl = work.querySelector(".master-detail > .list");
      if (listEl) {
        app.classList.add("has-explorer");
        app.appendChild(listEl);
      }
    }

    var shell = el("div", "shell");
    shell.appendChild(buildTopbar(cfg));
    if (isEntity && cfg.tabstrip) {
      shell.style.gridTemplateRows = "var(--topbar-h) auto minmax(0, 1fr) var(--statusbar-h)";
      shell.appendChild(buildTabstrip(cfg.tabstrip));
    }
    var wa = el("main", "workarea");
    attr(wa, { id: "main", role: "main", tabindex: "-1" });
    if (cfg.label) wa.setAttribute("aria-label", cfg.label);
    if (work) { wa.appendChild(work); work.style.display = "block"; }
    shell.appendChild(wa);
    shell.appendChild(buildStatus(cfg));

    app.appendChild(shell);
    document.body.innerHTML = "";
    document.body.appendChild(app);
    wireTabs(document);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", build);
  } else {
    build();
  }
})();
