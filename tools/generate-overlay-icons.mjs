// Generates the colored SVG assets used by the floating overlay headers.
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const output = resolve(dirname(fileURLToPath(import.meta.url)), '../assets/overlay-icons');
const art = {
  gold: '<path d="M2.5 7v8c0 2 11 2 11 0V7" fill="#D89925"/><ellipse cx="8" cy="7" rx="5.5" ry="2.5" fill="#FFD663"/><path d="M3 11c2 1.4 7 1.4 10 0" fill="none" stroke="#A66E1F"/><path d="M10.5 14v5c0 2.6 11 2.6 11 0v-5" fill="#E8A82D"/><ellipse cx="16" cy="14" rx="5.5" ry="2.5" fill="#FFE17B"/><path d="M12 18c2 1 5 1 8 0" fill="none" stroke="#B57820"/>',
  population: '<path d="M2 20v-3a5 5 0 0 1 10 0v3Z" fill="#3F88CD"/><circle cx="7" cy="8" r="3.5" fill="#F0B880"/><path d="M12 20v-3a5 5 0 0 1 10 0v3Z" fill="#75B7DF"/><circle cx="17" cy="8" r="3.5" fill="#F7D3A6"/><path d="M8 22v-5a4 4 0 0 1 8 0v5Z" fill="#255E9C"/><circle cx="12" cy="10" r="3.5" fill="#FFD7AC"/>',
  food: '<path d="M3 20a2 2 0 0 1-2-2v-5C1 6 7 3 12 3s11 3 11 10v5a2 2 0 0 1-2 2Z" fill="#C77C34"/><path d="M2 14C2 7 7 4 12 4s10 3 10 10Z" fill="#EAAF59" stroke="none"/><path d="m7 7-2 5m8-6-2 5m8-3-2 5" stroke="#FFE0A0" stroke-width="2.4"/><path d="M3 17h18" stroke="#A55E2A" fill="none"/>',
  tax: '<path d="M4 2h13v20l-3-2-3 2-3-2-4 2Z" fill="#E7D5AD"/><path d="M7 6h7M7 10h5M7 14h4" stroke="#846649" fill="none"/><circle cx="17" cy="15" r="5" fill="#F2BD43"/><path d="M17 12v6m-1.5-1 3-4" stroke="#8C5B21" fill="none"/>',
  popularity: '<circle cx="12" cy="12" r="10" fill="#8ACB66"/><path d="M5 7a8 8 0 0 1 12-1" stroke="#BDE69D" fill="none" stroke-width="1.5"/><circle cx="8.5" cy="10" r="1.1" fill="#314C31" stroke="none"/><circle cx="15.5" cy="10" r="1.1" fill="#314C31" stroke="none"/><path d="M7 14c2 5 8 5 10 0" fill="#FCF6D8"/>',
  fear: '<path d="M2 4c4-2 8-2 12 0v8c0 5-6 8-6 8s-6-3-6-8Z" fill="#8EBE78"/><path d="M5 8h1m4 0h1M5 12c2 3 4 3 6 0" stroke="#34482B" fill="none"/><path d="M11 5c4 2 7 2 11 0v9c0 5-5.5 8-5.5 8S11 19 11 14Z" fill="#D86C58"/><path d="m14 10 1 1m3 0 1-1m-5 7c1-3 4-3 5 0" stroke="#622C28" fill="none"/>',
  wood: '<path d="m4 11 9-7c4-3 10 4 7 7l-9 9Z" fill="#A96F3B"/><path d="m8 10 9-6m-4 10 8-6" stroke="#DDA568" fill="none"/><ellipse cx="7.5" cy="16" rx="5.5" ry="6" transform="rotate(-35 7.5 16)" fill="#E6B77C"/><ellipse cx="7.5" cy="16" rx="2.5" ry="3" transform="rotate(-35 7.5 16)" fill="none" stroke="#AA783F"/>',
  stone: '<path d="m2 19 2-7 6-2 5 4-2 7H4Z" fill="#AAB4BD"/><path d="m10 11 2-7 6-1 4 5-3 6-5 1Z" fill="#CCD2D4"/><path d="m12 4 4 5 6-1m-6 1-2 6" fill="none" stroke="#7F8A94"/><path d="m13 21 1-8 5-1 4 5-2 4Z" fill="#8493A1"/><path d="m4 12 4 4 5-2M8 16l-1 5" stroke="#74838F" fill="none"/>',
  iron: '<path d="m2 7.5 12-5 8 7-12 5Z" fill="#A5A39D" stroke="#77736B"/><path d="m2 7.5 8 7v7l-8-7Z" fill="#655D53" stroke="#4A453F"/><path d="m10 14.5 12-5v7l-12 5Z" fill="#292B2A" stroke="#77736B"/><path d="m3.5 7.5 10-4 6.5 5.5" fill="none" stroke="#BBB8B0" stroke-width=".7"/>',
  army: '<path d="m3 2 4 1 12 12-4 4L3 7Z" fill="#ADC9DA"/><path d="m21 2-4 1L5 15l4 4L21 7Z" fill="#D7E6EA"/><path d="m13 19 6-6M5 13l6 6" stroke="#E8B74F" stroke-width="3"/><path d="m17 18 4 4M7 18l-4 4" stroke="#8E5A31" stroke-width="3"/>',
  bow: '<path d="M7 2c15 4 15 16 0 20 9-6 9-14 0-20Z" fill="#C18B4E"/><path d="m7 2 6 10-6 10" stroke="#E9D8A9" fill="none"/><path d="M2 12h19" stroke="#85C2D9" stroke-width="1.8"/><path d="m18 9 5 3-5 3Z" fill="#B7DEE8"/><path d="m2 9 3 3-3 3" stroke="#DA6862" fill="none"/>',
  horse: '<path d="m4 22 1-9-3-3 5-5 1-3 4 3c7 0 9 6 8 12v5Z" fill="#B97E46"/><path d="M12 5c7 0 9 6 8 12l-5-3 1-7Z" fill="#664B38"/><path d="M7 6 6 10l-3 1m1 6 11-3" fill="none" stroke="#624128"/><circle cx="10" cy="7.5" r="1" fill="#252A31" stroke="none"/><path d="m12 5 9-3" stroke="#87CBE4" stroke-width="1.5"/><path d="m19 1 4 1-3 3Z" fill="#C2E4EA"/>',
  camel: '<path d="M3 21v-7l3-2c0-6 7-7 8-1l3 1V7l3-3h2v4l-3 2v11h-3v-6H6v6Z" fill="#DDA958"/><path d="M7 10c1-4 4-4 6 0" fill="#F2CC80" stroke="none"/><path d="M4 3h16" stroke="#956537" stroke-width="1.6"/><path d="m18 1 5 2-5 2Z" fill="#B7D0DC"/><path d="M3 21h3m10 0h3" stroke="#765337" stroke-width="1.7"/>',
  knight: '<path d="M5 20V10a7 7 0 0 1 14 0v10l-7 3Z" fill="#9AAFBF"/><path d="M6 10a6 6 0 0 1 12 0Z" fill="#D2E0E5" stroke="none"/><path d="M5 11h14v4H5Z" fill="#34495D"/><path d="M12 10v12" stroke="#E7BF62" stroke-width="2"/><path d="M9 3V1h6v3" fill="#CF6253"/><path d="M8 18h1m6 0h1" stroke="#526A7E"/>',
  lost: '<path d="M2 3h14v10c0 5-7 9-7 9s-7-4-7-9Z" fill="#8799A8"/><path d="M4 5h10v8c0 3-5 6-5 6s-5-3-5-6Z" fill="#BCCAD2" stroke="none"/><path d="m9 3-2 6 5 3-5 5" stroke="#4E6476" fill="none" stroke-width="1.5"/>',
};

const plus = '<circle cx="18.5" cy="18.5" r="5" fill="#5CB77C" stroke="#28583E"/><path d="M18.5 15.5v6m-3-3h6" stroke="#F0FFF2" stroke-width="1.5"/>';
const total = (name) => `<g transform="translate(0 .5) scale(.78)">${art[name]}</g>${plus}`;
const arrow = (down) => `<path d="M19 ${down ? '13v9m-3-3 3 3 3-3' : '22v-9m-3 3 3-3 3 3'}" fill="none" stroke="${down ? '#E46C65' : '#75C78E'}" stroke-width="2.4"/>`;
const icons = [
  ['gold', 'Gold', art.gold],
  ['population', 'Population', art.population],
  ['army', 'Army', art.army],
  ['food-ration', 'Food', art.food],
  ['tax', 'Tax', art.tax],
  ['popularity', 'Popularity', art.popularity],
  ['fear-factor', 'Fear factor', art.fear],
  ['horse-archer', 'Horse archers', art.horse],
  ['camel-lancer', 'Camel lancers', art.camel],
  ['knight', 'Knights', art.knight],
  ['shooters', 'Shooters', art.bow],
  ['army-killed', 'Army killed', `<g transform="translate(-.5 .5) scale(.8)">${art.army}</g>${arrow(false)}`],
  ['army-lost', 'Army lost', `${art.lost}${arrow(true)}`],
  ['gold-acquired', 'Acquired gold', total('gold')],
  ['food-produced', 'Produced food', total('food')],
  ['weapons-produced', 'Produced weapons', total('army')],
  ['wood-produced', 'Produced wood', total('wood')],
  ['stone-produced', 'Produced stone', total('stone')],
  ['iron-produced', 'Produced iron', total('iron')],
];

const escape = (value) => value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('"', '&quot;');
function svg(label, body) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" stroke="#493E32" stroke-width=".8" stroke-linecap="round" stroke-linejoin="round"><title>${escape(label)}</title>${body}</svg>\n`;
}
function at(icon, x, y, size) {
  return svg(icon[1], icon[2]).replace('<svg ', `<svg x="${x}" y="${y}" `).replace('width="24" height="24"', `width="${size}" height="${size}"`);
}
await mkdir(output, { recursive: true });
for (const [id, label, body] of icons) await writeFile(join(output, `${id}.svg`), svg(label, body));
await writeFile(join(output, 'manifest.json'), JSON.stringify({
  status: 'approved',
  intendedUse: 'Floating overlay column headers only, with icon before text.',
  viewBox: '0 0 24 24',
  style: 'simple-colored-pictograms',
  slintNote: 'Preserve intrinsic colors: do not apply Image.colorize. Bind square size to the label font size.',
  icons: icons.map(([id, label]) => ({ id, label, file: `${id}.svg` })),
}, null, 2) + '\n');

const rows = Math.ceil(icons.length / 4);
const width = 1308;
const height = 244 + rows * 142;
const sheet = [
  `<rect width="${width}" height="${height}" fill="#121619"/>`,
  '<text x="28" y="46" fill="#F1F1E9" font-size="28" font-weight="600">SHC Stats · colored overlay icons</text>',
  '<text x="28" y="78" fill="#B6BFC1" font-size="15">Simple colored shapes · transparent SVGs · scale with the text</text>',
];
for (const [row, size] of [18, 26, 34].entries()) {
  const y = 106 + row * 40;
  sheet.push(`<text x="28" y="${y + size * .8}" fill="#77868A" font-size="12">${size}px</text>`);
  for (const [index, id] of ['gold', 'population', 'army', 'popularity'].entries()) {
    const icon = icons.find((i) => i[0] === id);
    const x = 100 + index * 288;
    sheet.push(at(icon, x, y, size), `<text x="${x + size + 9}" y="${y + size * .8}" fill="#F2F1E7" font-size="${size}">${icon[1]}</text>`);
  }
}
for (const [index, icon] of icons.entries()) {
  const x = 28 + (index % 4) * 316;
  const y = 234 + Math.floor(index / 4) * 142;
  sheet.push(`<rect x="${x}" y="${y}" width="304" height="128" rx="10" fill="#22292E" stroke="#364047"/>`);
  sheet.push(at(icon, x + 16, y + 16, 48));
  sheet.push(`<text x="${x + 78}" y="${y + 45}" fill="#F4F1E8" font-size="17" font-weight="600">${icon[1]}</text>`);
  sheet.push(`<rect x="${x + 234}" y="${y + 71}" width="56" height="44" rx="6" fill="#F1F0E8"/>`, at(icon, x + 246, y + 77, 32));
  for (const [i, size] of [16, 24, 32].entries()) {
    const ix = x + 18 + i * 66;
    sheet.push(at(icon, ix, y + 83 - (size - 16) / 2, size));
    sheet.push(`<text x="${ix + size + 5}" y="${y + 99}" fill="#9CA9AE" font-size="10">${size}</text>`);
  }
}
await writeFile(join(output, 'preview.svg'), `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" font-family="Segoe UI, Arial, sans-serif">${sheet.join('')}</svg>\n`);
const card = ([id, label, body]) => `<article><div class="large">${svg(label, body)}<b>${label}</b></div><div class="sizes">${[16,24,32,48].map((size) => `<span>${svg(label, body).replace('width="24" height="24"', `width="${size}" height="${size}"`)}<small>${size}px</small></span>`).join('')}<span class="paper">${svg(label, body).replace('width="24" height="24"', 'width="32" height="32"')}</span></div><a href="${id}.svg">${id}.svg</a></article>`;
await writeFile(join(output, 'preview.html'), `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Colored overlay icons — review</title><style>
*{box-sizing:border-box}body{margin:0;padding:30px;background:#121619;color:#f4f1e8;font:15px 'Segoe UI',sans-serif}h1{font-size:28px;margin:0 0 10px}p{color:#b6bfc1;line-height:1.5}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:14px;margin-top:28px}article{background:#22292e;border:1px solid #364047;border-radius:10px;padding:18px}.large{display:flex;gap:16px;align-items:center}.large svg{width:48px;height:48px}.large b{font-size:18px}.sizes{display:flex;gap:18px;align-items:center;margin:24px 0 14px}.sizes span{display:flex;align-items:center;flex-direction:column;gap:8px;min-width:28px}small,a{font-size:11px;color:#9ca9ae}.paper{padding:8px;background:#f1f0e8;border-radius:5px}a{text-decoration:none}a:hover{text-decoration:underline}.example{display:flex;gap:30px;align-items:center;flex-wrap:wrap;margin-top:20px}.example span{display:flex;gap:.4em;align-items:center;font-size:24px}.example svg{width:1em;height:1em}
</style><h1>Colored overlay icons</h1><p>19 colored SVG icons for the floating overlay headers. Simple pictograms, transparent backgrounds, no fonts or embedded pictures. Each icon appears before its label and scales with the overlay text.</p><div class="example">${['gold','population','army','popularity'].map((id) => { const icon=icons.find((i) => i[0]===id); return `<span>${svg(icon[1],icon[2])}${icon[1]}</span>`; }).join('')}</div><div class="grid">${icons.map(card).join('')}</div></html>\n`);
console.log(`Generated ${icons.length} colored SVG icons and previews in ${output}`);
