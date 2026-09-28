/* ── MiaHUB frontend — G HUB style three-screen flow ────────────── */

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
          <span class="batt-txt">${battText()}</span>
        </span>
      </div>`;
    card.onclick = () => openDevice(d);
    wrap.appendChild(card);
  }

  if (!usable.length) {
    setUnconnected();
    renderBt();
    return;
  }
  renderBt();
  if (auto || !device) await connect(usable[0]);
}

/* 空态只在「既没有可控设备，也没有蓝牙设备」时出现 */
function updateHomeEmpty() {
  const hasCard = $("#device-cards").children.length > 0;
  const hasBt = $("#bt-panel").classList.contains("show");
  $("#home-empty").classList.toggle("show", !hasCard && !hasBt);
}

/* ── 蓝牙电量 ──────────────────────────────────────────────────────────
   蓝牙（BLE HID）下键盘没有厂商通道，控灯/改键全部不可用；
   但 Windows 通过标准 GATT 电量服务（0x180F）拿到了电量，缓存在设备节点属性里，
   这里直接读它 —— 这也是这台键盘唯一能显示电量的途径（2.4G 厂商通道是只写的）。 */
let bt = null;

/* 卡片右下角那格：有电量就报电量，没就只报连接状态。
   注意：这个电量只能来自蓝牙信道（固件在 USB / 2.4G 下不回传），
   所以卡片显示「接收器」时它其实是上一次蓝牙连接的读数 —— 用 ≈ 标出来。 */
function battText() {
  if (!bt || typeof bt.percent !== "number") return "已连接";
  return `${bt.percent}%${bt.charging ? " ⚡" : ""}`;
}

function battTitle() {
  return bt && typeof bt.percent === "number"
    ? "电量来自蓝牙信道的 GATT 电量服务（0x180F）。USB / 2.4G 通道固件不回传电量。"
    : "固件在 USB / 2.4G 通道不回传电量；切到蓝牙模式并配对后即可显示";
}

function renderBt() {
  const pct = bt && typeof bt.percent === "number" ? bt.percent : null;
  const known = pct !== null;
  const panel = $("#bt-panel");
  // 只有在没有可控设备时才把蓝牙面板当主角展示
  const asPanel = known && $("#device-cards").children.length === 0;

  panel.classList.toggle("show", asPanel);
  if (asPanel) {
    $("#bt-pct").innerHTML = `${pct}<span>%</span>`;
    $("#bt-addr").textContent = bt.address ? "· " + bt.address : "";
    const bar = $("#bt-meter");
    bar.style.width = `${Math.max(2, Math.min(100, pct))}%`;
    bar.className = pct <= 15 ? "low" : "";
  }

  // 设备卡片上的电池格跟着刷新
  $$(".dcard-batt").forEach((el) => {
    el.querySelector(".batt-txt").textContent = battText();
    el.title = battTitle();
    el.classList.toggle("low", known && pct <= 15);
  });

  const el = $("#set-batt");
  el.textContent = known ? `${pct}%${bt.charging ? " · 充电中" : ""}` : "—";
  el.title = battTitle();
  updateHomeEmpty();
}

async function refreshBt() {
  try {
    bt = await invoke("bt_battery");
  } catch {
    bt = null;
  }
  renderBt();
}

function setUnconnected() {
  device = null;
  $("#dev-name").textContent = "—";
  $("#set-name").textContent = "—";
  $("#set-conn").textContent = "未连接";
}

/* ── 应用级设置（落盘在 settings.json） ─────────────────────────────────
   键盘不回读配置（PROTOCOL.md §4）—— 官方软件也是自己拿 ledeffect.xml 记状态。
   所以“当前灯效”只能由应用侧记住，这里就是那份记住的账。 */
let appSettings = {
  close_to_tray: true,
  apply_on_connect: false,
  lighting: null,
  keep_awake: false,
  idle_step: 0,
  theme_mode: "dark",
  accent: "classic",
  background: "default",
};

async function loadSettings() {
  try {
    appSettings = await invoke("get_settings");
  } catch { /* 读不到就用默认值 */ }
}

/* 把记住的灯效铺回界面 —— 只改 UI，**不发帧**。
   这样打开 App 不会把你键盘当前的灯效盖掉（以前就是这么回退成常亮的）。 */
function restoreLighting() {
  const l = appSettings.lighting;
  if (l) {
    if (typeof l.effect === "number") effect = l.effect;
    if (typeof l.brightness === "number") brightness = l.brightness;
    if (typeof l.speed === "number") speed = l.speed;
  }
  $("#brightness").value = brightness;
  $("#brightness-val").textContent = String(brightness);
  $("#speed").value = speed;
  $("#speed-val").textContent = String(speed);
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

    restoreLighting();
    renderEffects();
    applyPreview();
    // 默认**不下发**。键盘不回读配置，我们手上这个状态只是“上次你选的”，
    // 不一定等于键盘现在的；无条件 push 就会把当前灯效盖成记忆值。
    // 想要“打开就恢复我的灯效”就在应用设置里打开那个开关。
    if (appSettings.apply_on_connect) await push();
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

/* 发送会合并：2.4G 下每帧要间隔 1500ms，连点效果时不必逐帧排队，
   只需要把最后一次的最终值发出去 */
let sending = false;
let sendPending = false;

async function push() {
  if (!device) return;
  // 任何手动的设置动作都算「有操作」，取消空闲熄灯
  lastActivity = Date.now();
  idleDimmed = false;
  if (sending) {
    sendPending = true;
    return;
  }
  sending = true;
  try {
    do {
      sendPending = false;
      const spd = speed;
      try {
        await invoke("set_lighting", { effect, brightness, speed: spd });
      } catch (e) {
        toast(String(e), true);
      }
    } while (sendPending);
  } finally {
    sending = false;
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
  // 恢复上次的位置（IDLE_STEPS 的下标）
  const step = Math.min(Math.max(appSettings.idle_step | 0, 0), IDLE_STEPS.length - 1);
  slider.value = String(step);
  idleMinutes = IDLE_STEPS[step];
  slider.addEventListener("change", () => {
    const i = Number(slider.value);
    invoke("set_idle_step", { step: i }).then((s) => (appSettings = s)).catch(() => {});
  });
  slider.addEventListener("input", () => {
    idleMinutes = IDLE_STEPS[Number(slider.value)] ?? 0;
    lastActivity = Date.now();
    out.textContent = idleLabel(idleMinutes);
  });
  out.textContent = idleLabel(idleMinutes);

  const ka = $("#keepawake");
  keepAwake = !!appSettings.keep_awake;
  ka.classList.toggle("on", keepAwake);
  $("#keepawake-val").textContent = keepAwake ? "已开启" : "关闭";
  ka.onclick = () => {
    keepAwake = !keepAwake;
    ka.classList.toggle("on", keepAwake);
    $("#keepawake-val").textContent = keepAwake ? "已开启" : "关闭";
    invoke("set_keep_awake", { enabled: keepAwake })
      .then((s) => (appSettings = s))
      .catch(() => {});
    toast(keepAwake ? "已开启保持唤醒，每 60 秒续一次" : "已关闭保持唤醒");
  };

  // idle watchdog: dim the backlight after N minutes without input.
  // Needs the boot-keyboard input collection, which the 2.4 GHz dongle does
  // not let us read from user mode — so this only runs on the wired link.
  setInterval(async () => {
    if (!device || idleMinutes === 0 || device.link_mode !== "feature") {
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

/* ── 外观：主题色 / 浅色模式 / 背景皮肤 ───────────────────────────────
   全部只改 CSS 变量，组件规则一行不动。预设都是自己定的，没照抄官方那几组。 */

/* surface 决定"界面底色"的观感（双色卡上半块 + 背景微光），accent 是高亮色 */
const ACCENTS = [
  { id: "classic", zh: "经典蓝", surface: "#1c2026", accent: "#0a84ff" },
  { id: "graphite", zh: "墨岩", surface: "#2b2b30", accent: "#e8402f" },
  { id: "provence", zh: "普罗旺斯", surface: "#7b8fd4", accent: "#f2a0b5" },
  { id: "indigo", zh: "黛蓝", surface: "#2f4d6b", accent: "#00a9a5" },
  { id: "amber", zh: "丹霞", surface: "#d0562d", accent: "#1b7fa8" },
  { id: "forest", zh: "松林", surface: "#2f5d3a", accent: "#8fd14f" },
];

/* 内置背景全是 CSS 渐变：不往仓库里塞几 MB 图片，也没有素材版权问题。
   想要真照片就自己上传。 */
const BACKGROUNDS = [
  { id: "default", zh: "默认", css: "none" },
  {
    id: "aurora", zh: "极光",
    css: "radial-gradient(1200px 720px at 18% -6%, rgba(88,120,255,.30), transparent 62%)," +
      "radial-gradient(900px 620px at 92% 104%, rgba(0,200,190,.24), transparent 58%)",
  },
  {
    id: "ember", zh: "余烬",
    css: "radial-gradient(1000px 640px at 84% -10%, rgba(255,120,60,.26), transparent 60%)," +
      "radial-gradient(820px 560px at 6% 106%, rgba(200,40,60,.22), transparent 58%)",
  },
  {
    id: "mist", zh: "雾霭",
    css: "linear-gradient(155deg, rgba(120,130,160,.20) 0%, rgba(20,22,30,0) 46%)," +
      "radial-gradient(760px 520px at 50% 118%, rgba(160,170,200,.18), transparent 62%)",
  },
  {
    id: "grid", zh: "网格",
    css: "repeating-linear-gradient(0deg, rgba(140,150,180,.09) 0 1px, transparent 1px 34px)," +
      "repeating-linear-gradient(90deg, rgba(140,150,180,.09) 0 1px, transparent 1px 34px)",
  },
  {
    id: "vignette", zh: "暗角",
    css: "radial-gradient(120% 92% at 50% 44%, rgba(0,0,0,0) 38%, rgba(0,0,0,.62) 100%)",
  },
];

let customBg = null; // 上传的背景图 data URL

function hexToRgb(hex) {
  const h = hex.replace("#", "");
  const full = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
  const n = parseInt(full, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function mix(hex, towardWhite, amount) {
  const [r, g, b] = hexToRgb(hex);
  const t = towardWhite ? 255 : 0;
  const m = (x) => Math.round(x + (t - x) * amount);
  return `rgb(${m(r)}, ${m(g)}, ${m(b)})`;
}

function currentAccent() {
  return ACCENTS.find((a) => a.id === appSettings.accent) || ACCENTS[0];
}

function applyTheme() {
  const light = appSettings.theme_mode === "light";
  document.documentElement.dataset.theme = light ? "light" : "dark";

  const a = currentAccent();
  const [r, g, b] = hexToRgb(a.accent);
  const [sr, sg, sb] = hexToRgb(a.surface);
  const root = document.documentElement.style;

  root.setProperty("--accent", a.accent);
  root.setProperty("--accent-rgb", `${r}, ${g}, ${b}`);
  root.setProperty("--accent-soft", `rgba(${r}, ${g}, ${b}, ${light ? 0.13 : 0.16})`);
  root.setProperty("--accent-ink", mix(a.accent, !light, light ? 0.42 : 0.72));
  root.setProperty("--tint", a.surface);
  root.setProperty(
    "--wash-image",
    `radial-gradient(1100px 640px at 78% -12%, rgba(${sr}, ${sg}, ${sb}, ${light ? 0.14 : 0.22}), transparent 72%)`
  );

  const preset = BACKGROUNDS.find((x) => x.id === appSettings.background) || BACKGROUNDS[0];
  const image = customBg ? `url("${customBg}")` : preset.css;
  const veil = customBg
    ? `linear-gradient(rgba(${light ? "255,255,255,.74" : "0,0,0,.58"}), rgba(${light ? "255,255,255,.74" : "0,0,0,.58"}))`
    : "none";
  root.setProperty("--bg-image", image);
  // 有照片时必须压一层薄纱，否则前景文字对比度不够
  root.setProperty("--bg-veil-image", veil);
}

function markPicked(sel, attr, value) {
  $$(sel).forEach((el) => el.classList.toggle("is-picked", el.dataset[attr] === value));
}

function renderAppearance() {
  const acc = $("#accent-grid");
  acc.innerHTML = "";
  for (const a of ACCENTS) {
    const b = document.createElement("button");
    b.className = "accent-card";
    b.dataset.accent = a.id;
    b.innerHTML =
      `<span class="accent-half top" style="background:${a.surface}"></span>` +
      `<span class="accent-half bottom" style="background:${a.accent}"></span>` +
      `<span class="accent-name">${a.zh}</span>` +
      `<span class="pick-tick">已应用</span>`;
    b.onclick = () => saveTheme({ accent: a.id });
    acc.appendChild(b);
  }

  const bg = $("#bg-grid");
  bg.innerHTML = "";
  const tiles = BACKGROUNDS.slice();
  if (customBg) tiles.push({ id: "custom", zh: "自定义", css: `url("${customBg}")` });
  for (const t of tiles) {
    const b = document.createElement("button");
    b.className = "bg-tile";
    b.dataset.bg = t.id;
    b.style.backgroundImage = t.css;
    b.innerHTML = `<span class="bg-name">${t.zh}</span><span class="pick-tick">已应用</span>`;
    b.onclick = () => {
      if (t.id === "custom" && !customBg) return;
      saveTheme({ background: t.id });
    };
    bg.appendChild(b);
  }

  markPicked(".accent-card", "accent", currentAccent().id);
  markPicked(".bg-tile", "bg", appSettings.background);
  markPicked(".mode-card", "mode", appSettings.theme_mode);

  const has = !!customBg;
  $("#bg-clear").disabled = !has;
  $("#bg-clear").style.opacity = has ? "1" : ".45";
}

async function saveTheme(patch) {
  const next = {
    mode: patch.mode ?? appSettings.theme_mode,
    accent: patch.accent ?? appSettings.accent,
    background: patch.background ?? appSettings.background,
  };
  try {
    appSettings = await invoke("set_theme", next);
    applyTheme();
    renderAppearance();
  } catch (e) {
    toast(String(e), true);
  }
}

async function initAppearance() {
  applyTheme();
  renderAppearance();

  // 左分类切换
  $$(".ov-nav-item").forEach((btn) => {
    btn.onclick = () => {
      $$(".ov-nav-item").forEach((b) => b.classList.toggle("active", b === btn));
      $$(".ov-pane").forEach((p) =>
        p.classList.toggle("active", p.dataset.pane === btn.dataset.pane)
      );
    };
  });

  $$(".mode-card").forEach((c) => {
    c.onclick = () => saveTheme({ mode: c.dataset.mode });
  });

  $("#bg-upload").onclick = () => $("#bg-file").click();
  $("#bg-file").onchange = async (e) => {
    const file = e.target.files && e.target.files[0];
    e.target.value = "";
    if (!file) return;
    if (!/^image\//.test(file.type)) return toast("请选一张图片", true);
    if (file.size > 8 * 1024 * 1024) return toast("图片太大，建议 4MB 以内", true);
    try {
      const data = await fileToBase64(file);
      appSettings = await invoke("save_background", { data, mime: file.type });
      customBg = `data:${file.type};base64,${data}`;
      applyTheme();
      renderAppearance();
      toast("背景已应用");
    } catch (err) {
      toast(String(err), true);
    }
  };

  $("#bg-clear").onclick = async () => {
    try {
      appSettings = await invoke("clear_background");
      customBg = null;
      appSettings.background = "default";
      applyTheme();
      renderAppearance();
      toast("已移除自定义背景");
    } catch (e) {
      toast(String(e), true);
    }
  };
}

function fileToBase64(file) {
  return new Promise((resolve, reject) => {
    const fr = new FileReader();
    fr.onerror = () => reject("读取文件失败");
    fr.onload = () => {
      const s = String(fr.result);
      resolve(s.slice(s.indexOf(",") + 1)); // 去掉 data:...;base64, 前缀
    };
    fr.readAsDataURL(file);
  });
}

/* ── 应用设置（持久化到 settings.json，与键盘无关） ───────────────── */

function initUiSettings() {
  const bind = (sel, key, cmd, onMsg, offMsg) => {
    const el = $(sel);
    if (!el) return;
    el.classList.toggle("on", !!appSettings[key]);
    el.onclick = async () => {
      const next = !el.classList.contains("on");
      el.classList.toggle("on", next);
      try {
        appSettings = await invoke(cmd, { enabled: next });
        toast(next ? onMsg : offMsg);
      } catch (e) {
        el.classList.toggle("on", !next);
        toast(String(e), true);
      }
    };
  };

  bind("#ov-tray", "close_to_tray", "set_close_to_tray",
    "点 ✕ 将最小化到托盘", "点 ✕ 将直接退出程序");
  bind("#ov-apply-connect", "apply_on_connect", "set_apply_on_connect",
    "连接键盘时会自动下发记住的灯效", "连接键盘时不再改动灯效");
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

  await loadSettings();
  // 自定义背景图存在本机配置文件旁边，启动时取回来（只读一次，不塞进设置 JSON）
  try {
    customBg = await invoke("background_image");
  } catch { /* 没上传过 */ }
  effects = await invoke("list_effects");
  renderEffects();
  initSliders();
  initPowerControls();
  initUiSettings();
  initAppearance();
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
  await refreshBt();
  setInterval(() => { if (!device) refreshDevices(true); }, 4000);
  // 电量变得很慢，一分钟一次足够了
  setInterval(refreshBt, 60_000);

  // 键盘休眠 / 掉线后 HID 句柄会失效，这里定期探活，
  // 一旦发觉对方不在了就归零，交给上面的重扫重新连上。
  // 注意：2.4G 通道没有 feature report，read_info 必然失败 —— 那不是掉线，
  // 如果在这里误判就会变成每 5 秒重连一次，直接把无线链路挤爆（表现为卡键）。
  setInterval(async () => {
    if (!device || device.link_mode !== "feature") return;
    try {
      await invoke("read_info");
    } catch {
      setUnconnected();
      toast("键盘已休眠或断开，正在等待重新连接…");
    }
  }, 5000);
});
