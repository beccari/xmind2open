'use strict';
var DATA = JSON.parse(document.getElementById('d').textContent);

var BOX_H = 40, GAP_X = 14, GAP_Y = 46, PAD = 24;
var CHAR_W = 6.4, MAX_LINE = 24, MAX_W = 210, MIN_W = 60;
var DEPTH_COLORS = ['#232326', '#b45f06', '#cc0000', '#a4508b', '#1c4587',
                    '#38761d', '#674ea7', '#9e7b0b', '#57314a'];

var cur = 0, scale = 1, MAXD = 0, LAST_W = 0;
var sv = document.getElementById('sv');
var wrapEl = document.getElementById('wrap');
var infoEl = document.getElementById('info');
var NODE_MAP = {};

function esc(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;')
                  .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function wrapText(t) {
  var words = String(t).split(/\s+/), lines = [], line = '';
  for (var i = 0; i < words.length; i++) {
    var w = words[i];
    if (line && (line + ' ' + w).length > MAX_LINE) { lines.push(line); line = w; }
    else line = line ? line + ' ' + w : w;
  }
  if (line) lines.push(line);
  if (lines.length > 2) {
    var rest = lines.slice(1).join(' ');
    lines.length = 2;
    lines[1] = rest.slice(0, MAX_LINE - 1) + '\u2026';
  }
  return lines;
}

function boxOf(t) {
  var lines = wrapText(t.t || ''), w = MIN_W;
  for (var i = 0; i < lines.length; i++)
    w = Math.max(w, Math.ceil(lines[i].length * CHAR_W + 20));
  return { lines: lines, w: Math.min(w, MAX_W) };
}

function countDesc(t) {
  var n = 0;
  for (var i = 0; i < t.ch.length; i++) n += 1 + countDesc(t.ch[i]);
  return n;
}

function layout(t, depth) {
  NODE_MAP[t.i] = t;
  t._d = depth;
  if (depth > MAXD) MAXD = depth;
  t._collapsible = t.k === 'topic' && t.ch && t.ch.length > 0;
  if (t._collapsible && collapsed.has(t.i)) t._vk = [];
  else t._vk = t.ch || [];
  var kw = 0;
  for (var i = 0; i < t._vk.length; i++) {
    kw += layout(t._vk[i], depth + 1);
    if (i) kw += GAP_X;
  }
  var b = boxOf(t);
  t._lines = b.lines;
  t._bw = b.w;
  t._w = Math.max(b.w, kw);
  t._y = depth * (BOX_H + GAP_Y);
  return t._w;
}

function place(t, xLeft) {
  t._cx = xLeft + t._w / 2;
  var bx = xLeft;
  for (var i = 0; i < t._vk.length; i++) { place(t._vk[i], bx); bx += t._vk[i]._w + GAP_X; }
}

var collapsed = new Set();

function styleOf(t) {
  if (t.k === 'summary') return { f: '#fff2cc', s: '#bf8f00', txt: '#5f4402', dash: '', edge: '#bf8f00', dashed: false };
  if (t.k === 'group')   return { f: '#f1f1f1', s: '#999999', txt: '#555555', dash: ' stroke-dasharray="5 4"', edge: '#bbbbbb', dashed: true };
  var c = DEPTH_COLORS[Math.min(t._d, DEPTH_COLORS.length - 1)];
  return { f: c, s: c, txt: '#ffffff', dash: '', edge: '#8a8a8a', dashed: false };
}

function drawEdges(t, out) {
  for (var i = 0; i < t._vk.length; i++) {
    var c = t._vk[i], st = styleOf(c);
    var x0 = t._cx, y0 = t._y + BOX_H / 2, x1 = c._cx, y1 = c._y - BOX_H / 2;
    var ym = Math.round((y0 + y1) / 2);
    var dash = st.dashed ? ' stroke-dasharray="5 4"' : '';
    out.push('<path d="M ' + x0 + ' ' + y0 + ' V ' + ym + ' H ' + x1 + ' V ' + y1 +
             '" fill="none" stroke="' + st.edge + '" stroke-width="1.3"' + dash + '/>');
    drawEdges(c, out);
  }
}

function drawNodes(t, out) {
  var st = styleOf(t);
  var x = Math.round(t._cx - t._bw / 2), y = Math.round(t._y - BOX_H / 2);
  var w = Math.round(t._bw), H = BOX_H;
  out.push('<rect x="' + x + '" y="' + y + '" width="' + w + '" height="' + H +
           '" rx="8" ry="8" fill="' + st.f + '" stroke="' + st.s +
           '" stroke-width="1.4"' + st.dash + ' data-i="' + t.i + '">');
  var tip = t.t || '';
  if (t.c) tip += '\n\ncovers: ' + t.c;
  if (t.m && t.m.length) tip += '\n\nmarkers: ' + t.m.join(', ');
  if (t.n) tip += '\n\n' + t.n;
  out.push('<title>' + esc(tip) + '</title></rect>');
  var ty = Math.round(y + H / 2 - (t._lines.length - 1) * 7.2 + 4.2);
  for (var i = 0; i < t._lines.length; i++)
    out.push('<text x="' + t._cx + '" y="' + (ty + i * 14.4) +
             '" text-anchor="middle" font-size="12" fill="' + st.txt +
             '" pointer-events="none">' + esc(t._lines[i]) + '</text>');
  if (t._collapsible && t._vk.length === 0) {
    var n = countDesc(t), bx = t._cx + w / 2 + 16;
    out.push('<g data-tog="' + t.i + '" style="cursor:pointer">' +
             '<circle cx="' + bx + '" cy="' + t._y + '" r="11" fill="#ffffff" stroke="#888"/>' +
             '<text x="' + bx + '" y="' + (t._y + 3.5) + '" text-anchor="middle" font-size="10" fill="#555">' +
             (n > 999 ? '999+' : n) + '</text></g>');
  }
  for (var j = 0; j < t._vk.length; j++) drawNodes(t._vk[j], out);
}

function render() {
  for (var k in NODE_MAP) delete NODE_MAP[k];
  MAXD = 0;
  var root = DATA.sheets[cur].root;
  layout(root, 0);
  place(root, 0);
  LAST_W = Math.round(root._w + PAD * 2);
  var H = Math.round((MAXD + 1) * (BOX_H + GAP_Y) - GAP_Y + PAD * 2);
  var parts = [];
  drawEdges(root, parts);
  drawNodes(root, parts);
  sv.innerHTML = parts.join('');
  sv.setAttribute('viewBox', '0 0 ' + LAST_W + ' ' + H);
  sv.setAttribute('width', Math.max(1, Math.round(LAST_W * scale)));
  sv.setAttribute('height', Math.max(1, Math.round(H * scale)));
}

function toggle(id) {
  if (collapsed.has(id)) collapsed.delete(id);
  else collapsed.add(id);
  render();
}

function setScale(s) {
  scale = Math.max(0.08, Math.min(4, s));
  render();
}

function fit() {
  render();
  scale = Math.min(1, (wrapEl.clientWidth - 40) / LAST_W);
  render();
}

function showInfo(t) {
  infoEl.style.display = 'block';
  infoEl.innerHTML = '<a class="x" onclick="document.getElementById(&quot;info&quot;).style.display=&quot;none&quot;">\u00d7</a>' +
    '<b>' + esc(t.t) + '</b>' +
    (t.k === 'summary' ? '<div class="kind">XMind summary topic</div>' : '') +
    (t.k === 'group' ? '<div class="kind">XMind boundary</div>' : '') +
    (t.c ? '<div><small>covers:</small> ' + esc(t.c) + '</div>' : '') +
    (t.n ? '<div class="note">' + esc(t.n).replace(/\n/g, '<br>') + '</div>' : '');
}

sv.addEventListener('click', function (e) {
  var tgl = e.target.closest ? e.target.closest('[data-tog]') : null;
  if (tgl) { toggle(+tgl.getAttribute('data-tog')); return; }
  var r = e.target.closest ? e.target.closest('[data-i]') : null;
  if (!r) return;
  var t = NODE_MAP[+r.getAttribute('data-i')];
  if (!t) return;
  if (t._collapsible) toggle(t.i);
  else showInfo(t);
});

var tabs = document.getElementById('tabs');
for (var i = 0; i < DATA.sheets.length; i++) {
  (function (i) {
    var b = document.createElement('button');
    b.textContent = DATA.sheets[i].t;
    b.onclick = function () { cur = i; collapsed.clear(); fit(); mark(); };
    tabs.appendChild(b);
  })(i);
}
function mark() {
  for (var i = 0; i < tabs.children.length; i++)
    tabs.children[i].classList.toggle('on', i === cur);
}
document.getElementById('zin').onclick = function () { setScale(scale * 1.25); };
document.getElementById('zout').onclick = function () { setScale(scale / 1.25); };
document.getElementById('fit').onclick = fit;
document.getElementById('exp').onclick = function () { collapsed.clear(); render(); };
document.getElementById('col').onclick = function () {
  (function rec(t) {
    if (t._collapsible) collapsed.add(t.i);
    for (var i = 0; i < t.ch.length; i++) rec(t.ch[i]);
  })(DATA.sheets[cur].root);
  render();
};
mark();
fit();
