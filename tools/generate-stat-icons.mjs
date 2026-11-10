// Original, dependency-free SVG assets. Run: node tools/generate-stat-icons.mjs
// This only generates assets and their review gallery; it does not change the UI.
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const output = resolve(dirname(fileURLToPath(import.meta.url)), '../assets/stat-icons');
const shapes = {
  gold: '<ellipse cx="9" cy="6.5" rx="6" ry="2.5"/><path d="M3 6.5v8c0 1.4 2.7 2.5 6 2.5M3 10.5c0 1.4 2.7 2.5 6 2.5"/><ellipse cx="15" cy="14" rx="6" ry="2.5"/><path d="M9 14v4c0 1.4 2.7 2.5 6 2.5s6-1.1 6-2.5v-4"/>',
  population: '<circle cx="9" cy="7" r="3"/><path d="M3 21v-3a6 6 0 0 1 12 0v3M16 4a3 3 0 0 1 0 6M18 14a5 5 0 0 1 3 4v3"/>',
  food: '<path d="M4 19a2 2 0 0 1-2-2v-5C2 6 7 3 12 3s10 3 10 9v5a2 2 0 0 1-2 2Z"/><path d="m7 7-2 4m8-5-2 4m8-2-2 4M3 15h18"/>',
  rations: '<circle cx="13" cy="12" r="6"/><path d="M2 3v6c0 2 4 2 4 0V3M4 3v18M22 3c-2 2-3 5-3 8h3m0-8v18"/>',
  tax: '<path d="M5 3h14v18l-3-2-4 2-4-2-3 2Z"/><path d="m9 14 6-7"/><circle cx="9" cy="7" r=".8" fill="currentColor" stroke="none"/><circle cx="15" cy="14" r=".8" fill="currentColor" stroke="none"/>',
  popularity: '<circle cx="12" cy="12" r="9"/><path d="M8 14a4.5 4.5 0 0 0 8 0"/><circle cx="8.5" cy="9" r="1" fill="currentColor" stroke="none"/><circle cx="15.5" cy="9" r="1" fill="currentColor" stroke="none"/>',
  fear: '<path d="M12 3v18M12 3a9 9 0 0 0 0 18M12 3a9 9 0 0 1 0 18M6 10l3-1m6 0 3 1M6 14c.5 2 2 3 4 3m4 0c1-2 2-3 4-3"/>',
  wood: '<path d="m4 10 9-6c3-2 8 1 8 5 0 2-1 3-2 4l-9 6"/><ellipse cx="7" cy="15" rx="5" ry="6" transform="rotate(-30 7 15)"/><path d="m10 9 7-4m-3 8 6-4"/>',
  stone: '<path d="m3 17 2-9 8-5 7 5 2 9-6 4H7Z"/><path d="m5 8 7 3 8-3m-8 3 4 10m-4-10-9 6"/>',
  iron: '<path d="m6 6 12 0 4 10-6 4H8l-6-4Z"/><path d="m6 6 2 8h8l2-8M2 16l6-2m8 0 6 2M8 14v6m8-6v6"/>',
  army: '<path d="m3 3 4 1 10 10-3 3L4 7Zm18 0-4 1-5 5 3 3 5-5ZM9 12l-5 5 3 3 5-5Z" fill="currentColor" stroke="none"/><path d="m12 18 6-6m-12 0 6 6m4 0 4 4M8 18l-4 4"/>',
  bow: '<path d="M7 3c12 3 12 15 0 18l5-9Z"/><path d="M3 12h18m-3-3 3 3-3 3"/>',
  horse: '<path d="m4 20 2-7-3-3 5-5 1-3 4 3c5 1 7 5 6 10v5Z"/><path d="m8 6 2 4-4 3m8-7 1 7m-8 4h11"/><circle cx="11" cy="7.5" r=".8" fill="currentColor" stroke="none"/>',
  camel: '<path d="M3 20v-6l3-2c1-5 4-5 5-1 2-5 5-4 6 0l1-5 3-2 1 2-2 3v11M3 16h16M6 16v4m9-4v4"/>',
  knight: '<path d="M6 20V9a6 6 0 0 1 12 0v11l-6 2Z"/><path d="M6 9h12m-6 0v13m-5-9h2m5 0h2M10 3V1h5"/>',
  archer: '<path d="M8 3c10 3 10 15 0 18l4-9Z"/><path d="M2 12h20m-3-3 3 3-3 3"/>',
  arabian: '<path d="M3 8c0-6 9-7 10-1M3 8h10M6 4l4 3M15 9c7 2 7 9 0 12l3-6Z"/><path d="M9 15h13m-2-2 2 2-2 2"/>',
  crossbow: '<path d="M3 8c5-5 13-5 18 0L12 12ZM12 3v18m-3-3h6M9 9l3 3 3-3"/>',
  skirmisher: '<path d="m13 20 6-15 2-2v5l-6 13M17 9l3 1"/><path d="M3 9h9v6c0 3-4.5 6-4.5 6S3 18 3 15Z"/>',
  slinger: '<path d="m5 3 5 8c1 2 3 2 4 0l5-8M9 10c-3 4 0 6 3 6s6-2 3-6M12 16v5"/><circle cx="12" cy="9" r="2"/>',
  shield: '<path d="M5 3h14v11c0 4-7 8-7 8s-7-4-7-8Z"/><path d="M8 6h8v8c0 2-4 5-4 5s-4-3-4-5Z"/>',
  catapult: '<circle cx="6" cy="19" r="2"/><circle cx="18" cy="19" r="2"/><path d="M3 16h18M8 16l3-9 5 9M7 3l10 10M5 3h5v3H5Z"/>',
  killed: '<path d="m4 3 4 1 8 8-4 4L4 8ZM3 14l7 7m-7 0 3-3M17 20v-7m-3 3 3-3 3 3"/>',
  lost: '<path d="M3 3h12v9c0 4-6 8-6 8s-6-4-6-8ZM8 3 7 9l4 3-3 4M20 13v8m-3-3 3 3 3-3"/>',
  workshop: '<path d="m3 10 9-7 9 7v11H3Z"/><path d="m8 17 7-7m-3-2 5 5m-3-6 4 4"/>',
  fletcher: '<path d="M7 3c12 3 12 15 0 18l5-9ZM2 12h20m-3-3 3 3-3 3M3 9l3 3-3 3"/>',
  poleturner: '<path d="M7 22V6L5 4l2-3 2 3-2 2M17 22V8m-3 0V3l3 2 3-2v5Z"/>',
  armourer: '<path d="m8 3 4 2 4-2 5 5-4 4v9H7v-9L3 8Z"/><path d="m8 3 1 5h6l1-5M10 12v5m4-5v5"/>',
  blacksmith: '<path d="M3 9h18l-4 5h-3v4h4v3H6v-3h4v-4H7ZM6 3h9v3H6Zm4 3v3"/>',
  baker: '<path d="M3 21V10a9 9 0 0 1 18 0v11Z"/><path d="M7 21v-7a5 5 0 0 1 10 0v7M9 17c0-2 3-1 3-4 3 3 4 6 0 6-2 0-3-1-3-2"/>',
  mill: '<path d="m8 22 2-8h4l2 8ZM5 22h14M11 19h2v3M12 10l4-8 4 3ZM12 10l8 4-3 4ZM12 10l-4 8-4-3ZM12 10 4 6l3-4Z"/>',
  woodcutter: '<path d="m6 21 9-17m-3 1 6-3 4 6-6 1M2 21h9M4 21v-5h5v5"/>',
  ticks: '<path d="M3 4h18M3 20h18M6 4v4c0 2 6 4 6 4s6-2 6-4V4M6 20v-4c0-2 6-4 6-4s6 2 6 4v4M9 17h6"/>',
  calendar: '<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M7 3v4m10-4v4M3 10h18M7 14h3m4 0h3M7 17h3"/>',
  time: '<circle cx="12" cy="13" r="8"/><path d="M9 2h6m-3 0v3m6 1 2-2M12 8v5l3 2"/>',
  speed: '<path d="M4 19a10 10 0 1 1 16 0M5 15h2M7 7l2 2m3-5v3m5 0-2 2m4 6h-2M12 15l5-6"/><circle cx="12" cy="15" r="1.5"/>',
};

const plus = '<g stroke-width="1.8"><path d="M18.5 15v7m-3.5-3.5h7"/></g>';
const total = (shape) => `<g transform="translate(0 .5) scale(.76)">${shapes[shape]}</g>${plus}`;
const mounted = (shape, weapon) => `<g transform="translate(0 5) scale(.84)">${shapes[shape]}</g>${weapon}`;
const icons = [
  ['gold', 'Gold', 'Economy', shapes.gold, ['gold']],
  ['population', 'Population', 'Economy', shapes.population, ['population']],
  ['food-stock', 'Food stock', 'Economy', shapes.food, ['food-stock']],
  ['food-ration', 'Food popularity effect', 'Economy', shapes.rations, ['food-ratio-text']],
  ['tax', 'Tax popularity effect', 'Economy', shapes.tax, ['tax-text']],
  ['popularity', 'Popularity', 'Economy', shapes.popularity, ['popularity', 'popularity-text']],
  ['fear-factor', 'Fear factor', 'Economy', shapes.fear, ['ff', 'ff-text']],
  ['wood', 'Wood', 'Economy', shapes.wood, ['wood']],
  ['stone', 'Stone', 'Economy', shapes.stone, ['stone']],
  ['army', 'Army', 'Army', shapes.army, ['army']],
  ['horse-archer', 'Horse archers', 'Army', mounted('horse', '<path d="M14 3h8m-2-2 2 2-2 2"/>'), ['ha']],
  ['camel-lancer', 'Camel lancers', 'Army', mounted('camel', '<path d="M3 4h17l2-2v4l-2-2"/>'), ['camel-lancers']],
  ['knight', 'Knights', 'Army', shapes.knight, ['knights']],
  ['shooters', 'Shooters', 'Army', shapes.bow, ['shooters']],
  ['archer', 'Archers', 'Army', shapes.archer, ['archers']],
  ['arabian-archer', 'Arabian archers', 'Army', shapes.arabian, ['arab-bows']],
  ['crossbowman', 'Crossbowmen', 'Army', shapes.crossbow, ['xbows']],
  ['skirmisher', 'Skirmishers', 'Army', shapes.skirmisher, ['skirmishers']],
  ['slinger', 'Slingers', 'Army', shapes.slinger, ['slingers']],
  ['portable-shield', 'Portable shields', 'Army', shapes.shield, ['shields']],
  ['catapult', 'Catapults', 'Army', shapes.catapult, ['catapults']],
  ['army-killed', 'Enemy army killed', 'Army', shapes.killed, ['army-killed']],
  ['army-lost', 'Own army lost', 'Army', shapes.lost, ['army-lost']],
  ['gold-acquired', 'Gold acquired', 'Totals', total('gold'), ['stat-gold-earned', 'acquired-gold-text']],
  ['food-produced', 'Food produced', 'Totals', total('food'), ['stat-food-produced', 'produced-food-text']],
  ['weapons-produced', 'Weapons produced', 'Totals', total('army'), ['stat-weapons-produced', 'produced-weapons-text']],
  ['wood-produced', 'Wood produced', 'Totals', total('wood'), ['stat-wood-produced', 'produced-wood-text']],
  ['stone-produced', 'Stone produced', 'Totals', total('stone'), ['stat-stone-produced', 'produced-stone-text']],
  ['iron-produced', 'Iron produced', 'Totals', total('iron'), ['stat-iron-produced', 'produced-iron-text']],
  ['workshop', 'Workshops', 'Buildings', shapes.workshop, ['workshops']],
  ['fletcher', 'Fletchers', 'Buildings', shapes.fletcher, ['fletchers-count']],
  ['poleturner', 'Poleturners', 'Buildings', shapes.poleturner, ['poleturners-count']],
  ['armourer', 'Armourers', 'Buildings', shapes.armourer, ['armourers-count']],
  ['blacksmith', 'Blacksmiths', 'Buildings', shapes.blacksmith, ['blacksmiths-count']],
  ['baker', 'Bakers', 'Buildings', shapes.baker, ['bakers-count']],
  ['mill', 'Mills', 'Buildings', shapes.mill, ['mills-count']],
  ['woodcutter', 'Woodcutters', 'Buildings', shapes.woodcutter, ['woodcutters-count']],
  ['ticks', 'Game ticks', 'Time', shapes.ticks, ['ticks-text']],
  ['calendar', 'Game date', 'Time', shapes.calendar, ['day-text', 'month-text', 'year-text']],
  ['elapsed-time', 'Elapsed time', 'Time', shapes.time, ['elapsed-text']],
  ['game-speed', 'Game speed', 'Time', shapes.speed, ['game-speed-text']],
];

const escape = (value) => value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('"', '&quot;');
function svg(label, shape) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" role="img"><title>${escape(label)}</title>${shape}</svg>\n`;
}

await mkdir(output, { recursive: true });
for (const [id, label, , shape] of icons) {
  await writeFile(join(output, `${id}.svg`), svg(label, shape));
}
await writeFile(join(output, 'manifest.json'), JSON.stringify({
  viewBox: '0 0 24 24',
  strokeWidth: 1.8,
  style: 'monochrome-outline',
  totalMark: 'A small plus denotes the acquired/produced total, not current stock.',
  icons: icons.map(([id, label, group, , fields]) => ({ id, label, group, file: `${id}.svg`, fields })),
}, null, 2) + '\n');

const groups = [...new Set(icons.map((icon) => icon[2]))];
const cards = (group, color) => icons.filter((icon) => icon[2] === group).map(([id, label, , shape]) => `
  <div class="card"><div class="sizes" style="color:${color}">
    ${[16, 20, 24, 32].map((size) => `<span class="size">${svg(label, shape).replace('width="24" height="24"', `width="${size}" height="${size}"`)}<small>${size}</small></span>`).join('')}
    </div><a href="${id}.svg">${label}</a><code>${id}.svg</code></div>`).join('');
const theme = (name, color) => `<section class="theme ${name}"><h2>${name === 'dark' ? 'Dark theme' : 'Light theme'}</h2>${groups.map((group) => `<h3>${group}</h3><div class="grid">${cards(group, color)}</div>`).join('')}</section>`;
const sample = (size, color) => `<div class="sample" style="font-size:${size}px;color:${color}"><b>Rustam</b>${[['gold','18,000'],['population','350'],['army','240'],['food-stock','920'],['popularity','100']].map(([id, value]) => `<span>${svg(id, icons.find((i) => i[0] === id)[3])}${value}</span>`).join('')}<small>${size}px text</small></div>`;
await writeFile(join(output, 'preview.html'), `<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>SHC Stats — SVG icon review</title><style>
*{box-sizing:border-box}body{margin:0;background:#e8e8e8;color:#181818;font:14px 'Segoe UI',sans-serif}
header{padding:32px 36px;background:#fff;border-bottom:1px solid #ccc}h1{margin:0 0 12px;font-size:26px}p{max-width:920px;line-height:1.6;margin:7px 0}
h2{margin:0 0 22px;font-size:22px}h3{margin:24px 0 12px;font-size:13px;text-transform:uppercase;letter-spacing:.1em;opacity:.6}
.theme{padding:32px 36px}.dark{background:#171717;color:#f5f5f5}.light{background:#fafafa;color:#202020}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(208px,1fr));gap:12px}.card{padding:16px;border:1px solid #8884;border-radius:8px}
.sizes{height:56px;display:flex;align-items:start;gap:18px}.size{height:56px;display:flex;flex-direction:column;align-items:center;gap:6px;min-width:20px}
small{font-size:10px;opacity:.5}a{display:block;color:inherit;text-decoration:none;font-weight:600;margin-top:8px}a:hover{text-decoration:underline}
code{display:block;font:11px Consolas,monospace;opacity:.5;margin-top:5px}.samples{padding:24px 36px;background:#171717}
.sample{display:flex;align-items:center;gap:1.2em;padding:12px 0;white-space:nowrap}.sample span{display:flex;align-items:center;gap:.4em}
.sample svg{width:1em;height:1em}.sample b{font-weight:600}.sample small{font-size:11px}pre{background:#f0f0f0;padding:16px;overflow:auto;line-height:1.5}
</style><header><h1>SHC Stats · simple SVG icons</h1>
<p>${icons.length} original vector icons. Transparent backgrounds, one stroke weight, no embedded pictures or fonts. Each card shows 16, 20, 24 and 32px. Click its name to open the individual SVG.</p>
<p><b>Totals:</b> the small plus means acquired/produced over the match. A plain resource icon means current stock. These are review assets; the existing application interface has not been changed.</p>
<p><b>Scaling:</b> set the icon's width and height from the same font-size property as the adjacent number. SVG currentColor works in this HTML preview; in Slint, explicitly set Image.colorize to the text color.</p>
<pre>Image {
    source: @image-url("../assets/stat-icons/gold.svg");
    width: root.overlay_font_size * 1px;
    height: self.width;
    image-fit: contain;
    colorize: #f5f5f5;
}</pre></header>
<div class="samples">${[16,24,32].map((size) => sample(size, '#f5f5f5')).join('')}</div>
${theme('dark', '#f5f5f5')}${theme('light', '#202020')}</html>\n`);

// A portable vector contact sheet, also used to render the PNG review image.
const sheetWidth = 1456;
const sheetHeight = 298 + Math.ceil(icons.length / 6) * 114;
const iconAt = (icon, x, y, size, color) => svg(icon[1], icon[3])
  .replace('<svg ', `<svg x="${x}" y="${y}" color="${color}" `)
  .replace('width="24" height="24"', `width="${size}" height="${size}"`);
const sheet = [
  `<rect width="${sheetWidth}" height="${sheetHeight}" fill="#101010"/>`,
  '<text x="26" y="44" fill="#f8f8f8" font-size="28" font-weight="600">SHC Stats · simple SVG icons</text>',
  `<text x="26" y="74" fill="#a0a0a0" font-size="15">${icons.length} monochrome vectors · 24 × 24 viewBox · transparent · scales with the text</text>`,
  '<text x="26" y="103" fill="#a0a0a0" font-size="13">Each card: 16 / 24 / 32px on dark, 24px on light. Small + marks acquired / produced totals.</text>',
];
for (const [row, size] of [16, 24, 32].entries()) {
  const y = 130 + row * 40;
  sheet.push(`<text x="26" y="${y + size * .8}" fill="#777" font-size="12">${size}px</text>`);
  for (const [column, [id, value]] of [['gold', '18,000'], ['population', '350'], ['army', '240'], ['food-stock', '920'], ['popularity', '100']].entries()) {
    const x = 96 + column * 242;
    sheet.push(iconAt(icons.find((icon) => icon[0] === id), x, y, size, '#f4f4f4'));
    sheet.push(`<text x="${x + size + 8}" y="${y + size * .8}" fill="#f4f4f4" font-size="${size}">${value}</text>`);
  }
}
for (const [index, icon] of icons.entries()) {
  const x = 26 + (index % 6) * 236;
  const y = 266 + Math.floor(index / 6) * 114;
  sheet.push(`<rect x="${x}" y="${y}" width="224" height="102" rx="7" fill="#1c1c1c" stroke="#333"/>`);
  sheet.push(`<rect x="${x + 170}" y="${y + 9}" width="44" height="44" rx="5" fill="#f6f6f6"/>`);
  sheet.push(iconAt(icon, x + 14, y + 23, 16, '#f5f5f5'));
  sheet.push(iconAt(icon, x + 54, y + 19, 24, '#f5f5f5'));
  sheet.push(iconAt(icon, x + 100, y + 15, 32, '#f5f5f5'));
  sheet.push(iconAt(icon, x + 180, y + 19, 24, '#202020'));
  sheet.push(`<text x="${x + 13}" y="${y + 75}" fill="#ededed" font-size="14">${escape(icon[1])}</text>`);
  sheet.push(`<text x="${x + 13}" y="${y + 92}" fill="#888" font-size="11">${escape(icon[2])}</text>`);
}
await writeFile(join(output, 'preview.svg'), `<svg xmlns="http://www.w3.org/2000/svg" width="${sheetWidth}" height="${sheetHeight}" viewBox="0 0 ${sheetWidth} ${sheetHeight}" font-family="Segoe UI, Arial, sans-serif">${sheet.join('')}</svg>\n`);
console.log(`Generated ${icons.length} SVG icons, manifest.json, preview.html and preview.svg in ${output}`);
