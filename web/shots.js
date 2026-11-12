// Скриншоты для README и релиза: открывает собранный PerfoCut.html в Chrome без окна
// и снимает несколько экранов. Нужен playwright-core и Chrome/Chromium.
//
//   npm i --no-save playwright-core
//   CHROME=/usr/bin/google-chrome node web/shots.js dist/PerfoCut.html docs/screenshots [имя ...]
//
// Интерфейс рисует Slint на canvas, поэтому координаты нажатий заданы в точках окна.
// Размеры окон постоянные, раскладка от машины не зависит.
const fs = require('fs');
const path = require('path');
const { chromium } = require('playwright-core');

const [html = 'dist/PerfoCut.html', out = 'docs/screenshots', ...only] = process.argv.slice(2);
const CHROME = process.env.CHROME || ['/usr/bin/google-chrome', '/usr/bin/chromium', '/usr/local/bin/chromium']
  .find(p => fs.existsSync(p));
const SLOW = Number(process.env.SHOTS_SLOW || 1); // множитель пауз для медленных машин

const DESK = { w: 1440, h: 900, dsf: 1 };
const PHONE = { w: 412, h: 892, dsf: 2, mobile: true };

// Точки окна 1440 × 900
const D = {
  example: [735, 551], draw: [851, 551],
  metal: [385, 80], contours: [503, 80], original: [622, 80],
  gear: [1412, 28], themeDark: [626, 399], themeLight: [813, 399], themeAmoled: [813, 429], close: [893, 324],
  lines: [228, 299],
  done: [1083, 80], eraser: [473, 80],
  size: { y: 137, x0: 336, x1: 578 }, soft: { y: 137, x0: 613, x1: 855 }, tone: { y: 137, x0: 897, x1: 1120 },
};

const sleep = ms => new Promise(r => setTimeout(r, ms * SLOW));

async function open(browser, dev, scheme = 'dark') {
  const ctx = await browser.newContext({ viewport: { width: dev.w, height: dev.h }, deviceScaleFactor: dev.dsf,
    isMobile: !!dev.mobile, hasTouch: !!dev.mobile, locale: 'ru-RU', colorScheme: scheme });
  // Chrome без окна отдаёт размеры canvas в пикселях без учёта масштаба экрана
  await ctx.addInitScript(() => {
    Object.defineProperty(ResizeObserverEntry.prototype, 'devicePixelContentBoxSize', { get() {
      return Array.from(this.contentBoxSize, s => ({ inlineSize: Math.round(s.inlineSize * devicePixelRatio),
        blockSize: Math.round(s.blockSize * devicePixelRatio) }));
    } });
  });
  const page = await ctx.newPage();
  page.on('pageerror', e => console.log('  ошибка страницы:', e.message));
  await page.goto('file://' + path.resolve(html));
  await page.waitForSelector('canvas', { timeout: 60000 });
  await sleep(5000);
  const cdp = dev.mobile ? await ctx.newCDPSession(page) : null;
  const u = {
    page,
    async click([x, y], wait = 700) {
      if (dev.mobile) await page.touchscreen.tap(x, y); else await page.mouse.click(x, y);
      await sleep(wait);
    },
    async wheel([x, y], dy, wait = 900) { await page.mouse.move(x, y); await page.mouse.wheel(0, dy); await sleep(wait); },
    // Ползунок: тянем от старого положения к доле f
    async slide(s, from, to) {
      await u.stroke([[s.x0 + (s.x1 - s.x0) * from, s.y], [s.x0 + (s.x1 - s.x0) * to, s.y]], 300);
    },
    async stroke(pts, wait = 500) {
      if (dev.mobile) {
        const tp = ([x, y]) => [{ x, y, id: 1, radiusX: 4, radiusY: 4, force: 1 }];
        await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: tp(pts[0]) });
        for (const p of pts.slice(1)) await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: tp(p) });
        await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
      } else {
        await page.mouse.move(pts[0][0], pts[0][1]);
        await page.mouse.down();
        for (const p of pts.slice(1)) await page.mouse.move(p[0], p[1]);
        await page.mouse.up();
      }
      await sleep(wait);
    },
    async theme(btn) { await u.click(D.gear, 900); await u.click(btn, 700); await u.click(D.close, 900); },
    async shot(name) {
      if (!dev.mobile) await page.mouse.move(dev.w - 2, dev.h - 2);
      await sleep(400);
      await page.screenshot({ path: path.join(out, name + '.png') });
      console.log('  ' + name + '.png');
    },
    async close() { await ctx.close(); },
  };
  return u;
}

// Кривые для рисунка, в точках окна
const range = (n, f) => Array.from({ length: n }, (_, i) => f(i / (n - 1)));
const wave = (x0, x1, y, a, periods, n = 36) => range(n, t => [x0 + (x1 - x0) * t, y + a * Math.sin(t * periods * 2 * Math.PI)]);
const spiral = (cx, cy, r0, r1, turns, n = 40) => range(n, t => {
  const a = t * turns * 2 * Math.PI, r = r0 + (r1 - r0) * t;
  return [cx + r * Math.cos(a), cy + r * Math.sin(a)];
});
const circle = (cx, cy, r, n = 28) => range(n, t => [cx + r * Math.cos(t * 2 * Math.PI), cy + r * Math.sin(t * 2 * Math.PI)]);
const logPos = (v, max = 1000 / 3) => Math.log(v / 0.5) / Math.log(max / 0.5);

// Рисунок на чистом листе 1000 × 600 мм: волна, спираль и кольца
async function paint(u) {
  await u.slide(D.size, logPos(8), logPos(45));
  await u.slide(D.soft, 0.25, 0.7);
  await u.stroke(wave(400, 1060, 420, 45, 1.5), 900);
  await u.stroke(wave(400, 1060, 520, 45, 1.5), 900);
  await u.slide(D.size, logPos(45), logPos(22));
  await u.slide(D.soft, 0.7, 0.15);
  await u.stroke(spiral(560, 640, 8, 70, 2.2), 900);
  await u.slide(D.tone, 1, 0.55);
  await u.stroke(circle(880, 650, 55), 700);
  await u.stroke(circle(880, 650, 25), 900);
}

// Точки окна телефона 412 × 892
const P = { example: [350, 568], draw: [205, 626], summary: [206, 457], size: { y: 161, x0: 22, x1: 122 } };

const scenes = {
  async 'desktop'(b) {
    const u = await open(b, DESK);
    await u.click(D.example, 6000);
    await u.shot('desktop');
    await u.close();
  },
  async 'desktop-full'(b) {
    const u = await open(b, DESK);
    await u.click(D.example, 6000);
    // Кнопка «на весь экран» в левом нижнем углу холста
    await u.click([340, 884], 2500);
    await u.shot('desktop-full');
    await u.close();
  },
  async 'contours'(b) {
    const u = await open(b, DESK);
    await u.click(D.example, 6000);
    await u.click(D.contours, 1500);
    await u.shot('contours');
    await u.close();
  },
  async 'lines-light'(b) {
    const u = await open(b, DESK, 'light');
    await u.theme(D.themeLight);
    await u.click(D.example, 6000);
    await u.wheel([158, 500], 700, 1500);
    await u.click(D.lines, 6000);
    await u.shot('lines-light');
    await u.close();
  },
  async 'drawing'(b) {
    const u = await open(b, DESK);
    await u.click(D.draw, 2000);
    await paint(u);
    await u.page.mouse.move(700, 470);
    await sleep(500);
    await u.page.screenshot({ path: path.join(out, 'drawing.png') });
    console.log('  drawing.png');
    await u.click(D.done, 6000);
    await u.shot('drawing-result');
    await u.close();
  },
  async 'phone-settings'(b) {
    const u = await open(b, PHONE);
    await u.click(P.example, 6000);
    await u.shot('phone-settings');
    await u.click(P.summary, 1500);
    await u.shot('phone-summary');
    await u.close();
  },
  async 'phone-drawing'(b) {
    const u = await open(b, PHONE);
    await u.click(P.draw, 2000);
    await u.slide(P.size, logPos(8), logPos(90));
    await u.stroke(wave(50, 362, 385, 22, 1.5, 30), 900);
    await u.stroke(wave(50, 362, 450, 22, 1.5, 30), 900);
    await u.stroke(circle(206, 500, 14, 20), 900);
    await u.shot('phone-drawing');
    await u.close();
  },
};

(async () => {
  if (!CHROME) throw new Error('не найден Chrome: задайте CHROME=/путь/к/chrome');
  fs.mkdirSync(out, { recursive: true });
  const browser = await chromium.launch({ executablePath: CHROME,
    args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  let failed = 0;
  for (const [name, run] of Object.entries(scenes)) {
    if (only.length && !only.includes(name)) continue;
    console.log(name);
    try { await run(browser); } catch (e) { failed++; console.log('  не вышло:', e.message); }
  }
  await browser.close();
  process.exit(failed ? 1 : 0);
})().catch(e => { console.log(e.message); process.exit(1); });
