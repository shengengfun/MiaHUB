/* ── MiaKeyDrv frontend — G HUB style three-screen flow ────────────── */

const { invoke } = window.__TAURI__.core;

let device = null;       // connected DeviceInfo
let effects = [];        // [{index, zh, en, has_speed}]
let effect = 0;
let brightness = 5;
let speed = 2;
let fxFilter = "";

const $ = (s) => document.querySelector(s);
const $$ = (s) => document.querySelectorAll(s);

/* which animation class + colour previews each effect.
   官方只对部分灯效显示速度滑块，这里每条灯效都有动画 */
const FX_CLASS = {
  0: "fx-lit", 1: "fx-gaming", 2: "fx-breathe", 3: "fx-breathe",
  4: "fx-cycle", 5: "fx-cycle", 6: "fx-cycle", 7: "fx-sweep",
  8: "fx-breathe", 9: "fx-sweep", 10: "fx-cycle", 11: "fx-sweep",
  12: "fx-cycle", 13: "fx-sweep", 14: "fx-cycle", 15: "fx-sweep",
  16: "fx-breathe", 17: "fx-sweep", 18: "fx-cycle", 19: "fx-cycle",
};
const FX_COLOR = {
  0: "#0a84ff", 1: "#ff3b30", 2: "#4d7cff", 3: "#ff9f0a", 4: "#00ffc8",
  5: "#7b5cff",
  6: "#ff4fa3", 7: "#26c6da", 8: "#ffd166", 9: "#ff5c5c", 10: "#8d9bff",
  11: "#00e5a0", 12: "#ffcc00", 13: "#0a84ff", 14: "#ff6b3d", 15: "#33d17a",
  16: "#c7d2fe", 17: "#f97316", 18: "#a855f7", 19: "#22d3ee",
};

function toast(msg, err = false) {
  const el = $("#toast");
  el.textContent = msg;
  el.classList.toggle("err", err);
  el.classList.add("show");
  clearTimeout(el._t);
  el._t = setTimeout(() => el.classList.remove("show"), 2600);
}

const modeLabel = (c) =>
  ({ wired: "USB 有线", dongle: "2.4G 无线", unknown: "未知" }[c] ?? c);

/* keys lit by effect 1 (指点江山 / Gaming Special Key) */
const GAMING_KEYS = new Set([
  "W", "A", "S", "D", "Q", "E", "R", "F", "Space",
  "Up", "Down", "Left", "Right",
  "1", "2", "3", "4", "5", "6", "Tab", "Esc",
]);

/* ── screens ────────────────────────────────────────────────────────── */

function show(screen) {
  $$(".screen").forEach((s) => s.classList.remove("active"));
  $(`#screen-${screen}`).classList.add("active");
  if (screen === "settings") requestAnimationFrame(() => layoutStaticBoard());
}

/* ── keyboard rendering ─────────────────────────────────────────────── */

function makeBoard(pad = 2, scaleKeys = 1) {
  const box = window.KEYBOARD_BOX;
  const board = document.createElement("div");
  board.className = "board";
  board.style.width = (box.w + pad * 2) * scaleKeys + "px";
  board.style.height = (box.h + pad * 2) * scaleKeys + "px";

  for (const k of window.KEYBOARD_LAYOUT) {
    const el = document.createElement("div");
    el.className = "key";
    el.style.left = (k.x + pad) * scaleKeys + "px";
    el.style.top = (k.y + pad) * scaleKeys + "px";
    el.style.width = (k.w - 2) * scaleKeys + "px";
    el.style.height = (k.h - 2) * scaleKeys + "px";
    el.title = k.n;
    el.dataset.k = k.n;
    // per-key glow variance so the surface looks alive
    el.style.setProperty("--key-glow", (0.35 + ((k.x * 7 + k.y * 3) % 45) / 100).toFixed(2));
    board.appendChild(el);
  }
  return board;
}

function fitMainBoard() {
  const stage = $(".kbd-stage");
  const board = $("#board");
  if (!stage || !board) return;
  const box = window.KEYBOARD_BOX;
  const s = Math.min(
    (stage.clientWidth - 20) / (box.w + 4),
    stage.clientHeight / (box.h + 4)
  );
  board.style.transform = `scale(${Math.max(s, 0.2).toFixed(3)})`;
  board.style.transformOrigin = "center";
}


function layoutStaticBoard() {
  const holder = $("#board-static");
  const board = holder?.querySelector(".board");
  if (!board) return;
  const s = Math.min(
    (holder.clientWidth || 460) / (window.KEYBOARD_BOX.w + 8),
    (holder.clientHeight || 220) / (window.KEYBOARD_BOX.h + 8)
  );
  board.style.transform = `scale(${Math.max(s, 0.2).toFixed(3)})`;
  board.style.transformOrigin = "center";
}

function applyPreview() {
  const boxes = [$("#board"), $("#board-static")].filter(Boolean);
  const color = FX_COLOR[effect] || "#0a84ff";
  const cls = FX_CLASS[effect];

  document.documentElement.style.setProperty("--fx", color);
  // 速度 0/1/2 → 动画周期，让预览真的跟着速度变（官方速度量程就是 0..2）
  document.documentElement.style.setProperty(
    "--fx-dur",
    `${[3.4, 1.9, 1.0][speed] ?? 1.9}s`
  );

  for (const board of boxes) {
    board.className = "board";
    if (cls) board.classList.add(cls);
    board.style.opacity = brightness === 0 ? "0.35" : "1";
    const g = brightness === 0 ? 0 : 0.2 + (brightness / 5) * 0.55;
    const gaming = effect === 1; // 指点江山 only lights the gaming cluster
    board.querySelectorAll(".key").forEach((el) => {
      const on = !gaming || GAMING_KEYS.has(el.dataset.k);
      el.style.setProperty("--key-glow", (on ? g : g * 0.05).toFixed(2));
    });
  }

  const glow = $("#kbd-glow");
  if (glow) {
    glow.style.background = `radial-gradient(closest-side, ${color}, transparent 72%)`;
    glow.style.opacity = brightness === 0 ? "0" : String(0.14 + brightness * 0.045);
  }
}

/* ── device list (home) ─────────────────────────────────────────────── */

async function refreshDevices(auto = false) {
  let list = [];
  try {
    list = await invoke("list_devices");
  } catch (e) {
    toast("扫描失败：" + e, true);
    return;
  }
  const usable = list.filter((d) => d.supported);

  const wrap = $("#device-cards");
  wrap.innerHTML = "";
  $("#home-empty").classList.toggle("show", usable.length === 0);

  for (const d of usable) {
    const card = document.createElement("div");
    card.className = "dcard";
    card.innerHTML = `
      <div class="dcard-name">${d.name.replace(/^AULA\s*/, "")}</div>
      <div class="dcard-meta">
        <span class="dcard-chip">${modeLabel(d.connection)}${
          d.verified ? "" : " · 未验证"
        }</span>
        <span class="dcard-batt">
          <svg viewBox="0 0 26 14" fill="none" stroke="currentColor" stroke-width="1.3">
            <rect x="1" y="2" width="20" height="10" rx="2.5"/>
            <rect x="3" y="4" width="14" height="6" rx="1" fill="currentColor" stroke="none"/>
            <path d="M23 5.5v3"/>
          </svg>
          已连接
        </span>
      </div>`;
    card.onclick = () => openDevice(d);
    wrap.appendChild(card);
  }

  if (!usable.length) {
    setUnconnected();
    return;
  }
  if (auto || !device) await connect(usable[0]);
}

function setUnconnected() {
  device = null;
  $("#dev-name").textContent = "—";
  $("#set-name").textContent = "—";
  $("#set-conn").textContent = "未连接";
}

/* ── connect + push ─────────────────────────────────────────────────── */

async function connect(d) {
  try {
    const info = await invoke("connect", { path: d.path });
    device = info;

    $("#dev-name").textContent = info.name.replace(/^AULA\s*/, "");
    $("#set-name").textContent = info.name.replace(/^AULA\s*/, "");
    $("#set-conn").textContent = modeLabel(info.connection);
    $("#set-vidpid").textContent =
      `${info.vid.toString(16).toUpperCase().padStart(4, "0")} : ` +
      `${info.pid.toString(16).toUpperCase().padStart(4, "0")}`;
    $("#set-serial").textContent = info.serial || "—";

    renderEffects();
    applyPreview();
    await push();
  } catch (e) {
    toast(String(e), true);
  }
}

async function openDevice() {
  if (!device) {
    const devices = await invoke("list_devices");
    const usable = devices.filter((d) => d.supported);
    if (!usable.length) return toast("没有可用设备，请插上 USB 线", true);
    await connect(usable[0]);
  }
  if (!device) return;
  show("device");
  requestAnimationFrame(() => {
    fitMainBoard();
    applyPreview();
  });
}

/* ── effects panel ──────────────────────────────────────────────────── */

function renderEffects() {
  const list = $("#fx-list");
  const q = fxFilter.trim().toLowerCase();
  const shown = effects.filter(
    (f) => !q || f.zh.toLowerCase().includes(q) || f.en.toLowerCase().includes(q)
  );

  list.innerHTML = "";
  for (const fx of shown) {
    const b = document.createElement("button");
    b.className = "fx-item" + (fx.index === effect ? " active" : "");
    b.innerHTML = `${fx.zh}${
      fx.hidden ? '<span class="fx-tag">隐藏</span>' : ""
    }`;
    b.onclick = () => {
      effect = fx.index;
      renderEffects();
      applyPreview();
      push();
    };
    list.appendChild(b);
  }

  const count = $("#fx-count");
  if (count) count.textContent = q ? `${shown.length}/${effects.length}` : "";
  $("#fx-empty").classList.toggle("show", shown.length === 0);

  const cur = effects.find((f) => f.index === effect);
  $("#work-title").textContent = cur ? `${cur.zh}（${cur.en}）` : "—";

  // 速度对每条灯效都可调。官方是按灯效分页决定要不要显示滑块，
  // 这里不限制（不能比原版选项少），只对静态灯效加一句提示。
  const rowSpeed = $("#row-speed");
  rowSpeed.classList.toggle("hint", cur ? !cur.has_speed : false);
  $("#speed").disabled = false;
  rowSpeed.querySelector(".toggle").classList.add("on");
}

async function push() {
  if (!device) return;
  // 任何手动的设置动作都算「有操作」，取消空闲熄灯
  lastActivity = Date.now();
  idleDimmed = false;
  const spd = speed;
  try {
    await invoke("set_lighting", { effect, brightness, speed: spd });
  } catch (e) {
    toast(String(e), true);
  }
}

/* ── power / performance (app-side, firmware has no support) ────────── */

/* slider index -> idle minutes; 0 means "off" */
const IDLE_STEPS = [0, 1, 3, 5, 10, 15, 30];

let idleMinutes = 0;
let idleDimmed = false;
let lastActivity = Date.now();
let keepAwake = false;

const idleLabel = (m) => (m === 0 ? "关闭" : `${m} 分钟`);

function initPowerControls() {
  const slider = $("#idle-min");
  const out = $("#idle-val");
  slider.addEventListener("input", () => {
    idleMinutes = IDLE_STEPS[Number(slider.value)] ?? 0;
    lastActivity = Date.now();
    out.textContent = idleLabel(idleMinutes);
  });
  out.textContent = idleLabel(IDLE_STEPS[Number(slider.value)] ?? 0);

  const ka = $("#keepawake");
  ka.onclick = () => {
    keepAwake = !keepAwake;
    ka.classList.toggle("on", keepAwake);
    $("#keepawake-val").textContent = keepAwake ? "已开启" : "关闭";
    toast(keepAwake ? "已开启保持唤醒，每 60 秒续一次" : "已关闭保持唤醒");
  };

  // idle watchdog: dim the backlight after N minutes without input
  setInterval(async () => {
    if (!device || idleMinutes === 0) {
      if (idleDimmed) {
        idleDimmed = false;
        push();
      }
      return;
    }
    let active = false;
    try {
      active = await invoke("probe_activity", { waitMs: 120 });
    } catch { /* device busy — treat as idle */ }
    if (active) lastActivity = Date.now();

    const idle = Date.now() - lastActivity;
    if (!idleDimmed && idle > idleMinutes * 60_000) {
      idleDimmed = true;
      try { await invoke("set_light_off"); } catch { /* ignore */ }
    } else if (idleDimmed && active) {
      idleDimmed = false;
      push();
    }
  }, 2500);

  // keep-awake: re-assert the current lighting so the MCU never idles out.
  // NOTE: deliberately not push(), which would reset the idle timer.
  setInterval(() => {
    if (!keepAwake || !device || idleDimmed) return;
    const spd = effects.find((f) => f.index === effect)?.has_speed ? speed : 0;
    invoke("set_lighting", { effect, brightness, speed: spd }).catch(() => {});
  }, 60_000);

  $("#btn-poll").onclick = async () => {
    if (!device) return toast("请先连接键盘", true);
    if (!device.inputPath) return toast("这台设备没有暴露键盘输入集合", true);
    const btn = $("#btn-poll");
    const out2 = $("#poll-out");
    btn.disabled = true;
    out2.textContent = "测量中… 按住一个键别放";
    try {
      const s = await invoke("measure_polling", { seconds: 5 });
      out2.textContent = s.median_ms
        ? `${Math.round(s.hz)} Hz · 间隔 ${s.median_ms.toFixed(2)} ms`
        : `${Math.round(s.hz)} Hz · 报告 ${s.reports} 份`;
      toast(s.hz < 1 ? "几乎没收到报告，请按住一个键再试" : "测量完成");
    } catch (e) {
      out2.textContent = "—";
      toast(String(e), true);
    }
    btn.disabled = false;
  };

  $("#btn-probe").onclick = () =>
    toast("探测需要单独跑 tools/probe_registers.py（会逐个试未知寄存器，有风险）");
}

/* ── boot ───────────────────────────────────────────────────────────── */

function initSliders() {
  const bind = (id, valId, set) => {
    const el = $(`#${id}`);
    const out = $(`#${valId}`);
    const sync = () => (out.textContent = el.value);
    el.addEventListener("input", () => {
      set(Number(el.value));
      sync();
      applyPreview();
    });
    el.addEventListener("change", () => {
      set(Number(el.value));
      sync();
      applyPreview();
      push();
    });
    sync();
  };
  bind("brightness", "brightness-val", (v) => (brightness = v));
  bind("speed", "speed-val", (v) => (speed = v));
}

window.addEventListener("DOMContentLoaded", async () => {
  // main keyboard preview
  const placeholder = $("#board");
  const mainBoard = makeBoard(3, 1);
  mainBoard.id = "board";
  placeholder.replaceWith(mainBoard);
  // settings preview
  $("#board-static").appendChild(makeBoard(3, 1));

  effects = await invoke("list_effects");
  renderEffects();
  initSliders();
  initPowerControls();
  applyPreview();
  fitMainBoard();
  window.addEventListener("resize", () => {
    fitMainBoard();
    layoutStaticBoard();
  });

  // navigation
  $("#dev-back").onclick = () => show("home");
  $("#set-back").onclick = () => show("device");
  $("#go-settings").onclick = () => {
    show("settings");
    requestAnimationFrame(layoutStaticBoard);
  };
  $("#home-settings").onclick = () => $("#app-settings").classList.add("show");
  $("#ov-close").onclick = () => $("#app-settings").classList.remove("show");
  $("#app-settings").onclick = (e) => {
    if (e.target.id === "app-settings") $("#app-settings").classList.remove("show");
  };
  $("#btn-rescan").onclick = () => {
    toast("重新扫描…");
    refreshDevices();
  };
  $("#fx-search").oninput = (e) => {
    fxFilter = e.target.value;
    renderEffects();
  };

  $("#btn-off").onclick = async () => {
    if (!device) return toast("请先连接键盘", true);
    brightness = 0;
    $("#brightness").value = 0;
    $("#brightness-val").textContent = "0";
    applyPreview();
    try {
      await invoke("set_light_off");
      toast("灯光已关闭");
    } catch (e) {
      toast(String(e), true);
    }
  };
  $("#btn-reset").onclick = () => {
    brightness = 5;
    speed = 2;
    $("#brightness").value = 5;
    $("#speed").value = 2;
    $("#brightness-val").textContent = "5";
    $("#speed-val").textContent = "2";
    applyPreview();
    push();
    toast("已恢复为最亮 / 最快");
  };
  $("#btn-read").onclick = async () => {
    if (!device) return toast("请先连接键盘", true);
    try {
      $("#set-read").textContent = await invoke("read_info");
      toast("已读取设备应答");
    } catch (e) {
      toast(String(e), true);
    }
  };

  // window controls
  try {
    const win = window.__TAURI__.window.getCurrentWindow();
    $("#btn-min").onclick = () => win.minimize();
    $("#btn-close").onclick = () => win.close();
  } catch { /* not running inside tauri */ }

  // collapse groups
  $$(".group-head").forEach((h) => {
    h.onclick = () => h.closest(".group").classList.toggle("collapsed");
  });

  await refreshDevices(true);
  setInterval(() => { if (!device) refreshDevices(true); }, 4000);

  // 键盘休眠 / 掉线后 HID 句柄会失效，这里定期探活，
  // 一旦发觉对方不在了就归零，交给上面的重扫重新连上。
  setInterval(async () => {
    if (!device) return;
    try {
      await invoke("read_info");
    } catch {
      setUnconnected();
      toast("键盘已休眠或断开，正在等待重新连接…");
    }
  }, 5000);
});
