'use strict';
const $=(s,r=document)=>r.querySelector(s), $$=(s,r=document)=>[...r.querySelectorAll(s)];
const enc=new TextEncoder(), dec=new TextDecoder();
// ---------- Настройки приложения: тема, язык, вибрация ----------
const SETS='perfocut.settings.v1';
const S={theme:'dark',lang:'ru',vibro:true,lockRatio:false,ratio:0};
(()=>{let raw=null;try{raw=localStorage.getItem(SETS)}catch(e){}
  if(raw){try{Object.assign(S,JSON.parse(raw))}catch(e){}}
  else if(!/^ru|^uk|^be|^kk/i.test(navigator.language||'ru'))S.lang='en'})();
function saveS(){try{localStorage.setItem(SETS,JSON.stringify(S))}catch(e){}}
const T=(s,o)=>{const m=/^(\s*)([\s\S]*?)(\s*)$/.exec(s);let t=S.lang==='en'?(I18N[m[2]]??m[2]):m[2];if(o)for(const k in o)t=t.split('{'+k+'}').join(o[k]);return m[1]+t+m[3]};
const fmtN=(v,d=0)=>Number(v).toLocaleString(S.lang==='en'?'en-US':'ru-RU',{minimumFractionDigits:d,maximumFractionDigits:d});
const isExe=location.protocol==='http:'&&/^(127\.0\.0\.1|localhost)$/.test(location.hostname);
const AB=window.AndroidBridge||null;
let ex, DEF, P, stats=null, geom=null, img=null, mode='metal', view={z:1,x:0,y:0}, runT=0, C={};
const STORE='perfocut.params.v1';

// ---------- WASM ----------
function put(bytes){const n=bytes.length,p=ex.pc_alloc(n);new Uint8Array(ex.memory.buffer,p,n).set(bytes);return [p,n]}
function out(){const p=ex.pc_out_ptr(),n=ex.pc_out_len();return new Uint8Array(ex.memory.buffer,p,n).slice()}
function call(fn,bytes){const [p,n]=put(bytes);try{return fn(p,n)}finally{ex.pc_free(p,n)}}
function exportFmt(id){const rc=call(ex.pc_export,enc.encode(id));if(rc!==0)throw new Error('format '+id);return out()}
function zipBuild(files){ex.pc_zip_new();for(const [name,data] of files){const nb=enc.encode(name);const [np,nl]=put(nb);const [dp,dl]=put(data);ex.pc_zip_add(np,nl,dp,dl);ex.pc_free(np,nl);ex.pc_free(dp,dl)}ex.pc_zip_finish();return out()}

async function boot(){
  try{
    const b64=$('#wasm').textContent.trim();
    const bin=Uint8Array.from(atob(b64),c=>c.charCodeAt(0));
    const {instance}=await WebAssembly.instantiate(bin,{});
    ex=instance.exports;
  }catch(e){toast(T('Не удалось запустить ядро: ')+e.message,8000);return}
  ex.pc_defaults(); DEF=JSON.parse(dec.decode(out()));
  let saved={}; try{saved=JSON.parse(localStorage.getItem(STORE)||'{}')}catch(e){}
  P=Object.assign({},DEF,saved);P.lang=S.lang;if(S.lang==='en'&&P.name==='Панель')P.name='Panel';
  applyTheme(); applyLang();
  bindFormats(); renderFormats(); bindInputs(); syncInputs(); bindStage(); bindSettings(); bindModals(); bindKeys(); bindHaptics(); bindKeyboard(); resize();
  new ResizeObserver(resize).observe($('#wrap'));
  schedule();
}

// ---------- Inputs ----------
function bindInputs(){
  $$('[data-k]').forEach(el=>{
    const k=el.dataset.k;
    if(el.classList.contains('seg')){
      el.addEventListener('click',e=>{const b=e.target.closest('button');if(!b)return;
        P[k]=('bool' in el.dataset)?b.dataset.v==='true':b.dataset.v;changed()});
      return;
    }
    const ev=(el.type==='checkbox')?'change':'input';
    el.addEventListener(ev,()=>{
      if(el.type==='checkbox')P[k]=el.checked;
      else if('str' in el.dataset)P[k]=el.value;
      else{const v=parseFloat(String(el.value).replace(',','.'));if(!isFinite(v))return;P[k]=v*(parseFloat(el.dataset.scale)||1);
        if(k==='gc_decimals')P[k]=Math.max(1,Math.min(6,Math.round(v)));
        if(S.lockRatio&&S.ratio>0&&v>0){if(k==='sheet_w')P.sheet_h=r1(v/S.ratio);if(k==='sheet_h')P.sheet_w=r1(v*S.ratio)}}
      if(el.type==='range')rangeOut(el);
      changed(el);
    });
  });
  $('#gcPreset').addEventListener('click',e=>{const b=e.target.closest('button');if(!b||'custom' in b.dataset)return;P.gc_on=b.dataset.on;P.gc_off=b.dataset.off;changed()});
  $('#bFit').onclick=()=>{P.d_max=Math.max(0,+(P.pitch-P.bridge).toFixed(2));changed()};
  $('#bAspect').onclick=()=>{if(!img)return;P.sheet_h=Math.round(P.sheet_w*img.h/img.w);S.ratio=P.sheet_w/P.sheet_h;saveS();changed()};
  $('#bReset').onclick=()=>{const keep={name:P.name,order_no:P.order_no,client:P.client,phone:P.phone,email:P.email,comment:P.comment};P=Object.assign({},DEF,keep,{lang:S.lang});if(S.lang==='en'&&P.name==='Панель')P.name='Panel';S.ratio=P.sheet_w/P.sheet_h;saveS();changed();toast(T('Настройки сброшены'))};
  $('#lockRatio').onchange=e=>{S.lockRatio=e.target.checked;S.ratio=P.sheet_w/P.sheet_h;saveS()};
  $('#holesN').addEventListener('input',e=>{const n=Math.round(parseFloat(e.target.value));if(!(n>=1))return;
    const long=Math.max(P.sheet_w,P.sheet_h)-2*P.border;if(long<=0)return;P.pitch=Math.max(0.5,+(long/n).toFixed(2));changed(e.target)});
  $('#bTrace').onclick=()=>{Object.assign(P,{grid:'square',shape:'circle',size_mode:'prop',fixed:false,tone_cut:0.05,
    d_max:Math.max(0,+Math.min(P.pitch*1.3/1.5,P.pitch-P.bridge).toFixed(2))});changed();toast(T('Как в SVG tracing: квадратная сетка, круги, диаметр пропорционален тону'),4000)};
}
function rangeOut(el){const o=el.parentElement.querySelector('output');if(!o)return;const k=el.dataset.k;const v=P[k];
  o.textContent=k==='gamma'?v.toFixed(2):k==='tone_cut'?Math.round(v*100)+'%':(v>0?'+':'')+Math.round(v*100)}
function syncInputs(skip){
  $$('[data-k]').forEach(el=>{
    if(el===skip)return;const k=el.dataset.k,v=P[k];
    if(el.classList.contains('seg')){$$('button',el).forEach(b=>b.classList.toggle('on',String(v)===b.dataset.v));return}
    if(el.type==='checkbox')el.checked=!!v;
    else if('str' in el.dataset)el.value=v??'';
    else{const s=parseFloat(el.dataset.scale)||1;const sv=v/s;el.value=el.type==='range'?sv:+sv.toFixed(4);if(el.type==='range')rangeOut(el)}
  });
  $$('[data-show]').forEach(el=>{const [k,v]=el.dataset.show.split('=');el.hidden=String(P[k])!==v});
  const pre=$$('#gcPreset button');let m=false;pre.forEach(b=>{const on=!('custom' in b.dataset)&&b.dataset.on===P.gc_on&&b.dataset.off===P.gc_off;b.classList.toggle('on',on);m=m||on});
  pre[pre.length-1].classList.toggle('on',!m);
  $$('[data-cur]').forEach(e=>e.textContent=P.currency);
  $('#mSheet').textContent=`${fmtN(P.sheet_w)} × ${fmtN(P.sheet_h)}`;
  $('#mMetal').textContent=`${fmtN(P.thickness,1)} ${T('мм')}`;
  $('#lockRatio').checked=!!S.lockRatio;
  const hn=$('#holesN');if(hn!==skip){const long=Math.max(P.sheet_w,P.sheet_h)-2*P.border;hn.value=P.pitch>0&&long>0?Math.round(long/P.pitch):''}
}
const r1=v=>Math.round(v*10)/10;
function changed(src){syncInputs(src);try{localStorage.setItem(STORE,JSON.stringify(P))}catch(e){}schedule()}
function schedule(){clearTimeout(runT);runT=setTimeout(run,90)}

// ---------- Generation ----------
function run(){
  P.lang=S.lang;
  const rc=call(ex.pc_generate,enc.encode(JSON.stringify(P)));
  if(rc!==0){toast(dec.decode(out()),5000);return}
  stats=JSON.parse(dec.decode(out()));
  ex.pc_geometry();const g=out();geom=new Float32Array(g.buffer,0,g.byteLength>>2);
  renderStats();draw();
  const has=!!img&&stats.count>0;['bZip','bZip2','bSaveOrder','bSaveOrder2'].forEach(id=>$('#'+id).disabled=!(id.startsWith('bSave')?!!img:has));
}

function renderStats(){
  const s=stats,c=P.currency;
  if(!img){$('#sCost').textContent='—';['sCount','sPierce','sCut','sTime','sOpen','sMass'].forEach(i=>$('#'+i).textContent='—');$('#sCostRows').innerHTML='';$('#bSizes').hidden=true;
    $('#sWarn').innerHTML=`<div class="okline">${T('Загрузите картинку')}</div>`;$('#hud').textContent='';return}
  $('#sCost').innerHTML=`${fmtN(s.cost,0)} <small>${esc(c)}</small>`;
  const rows=[[T('Резка'),s.cost_cut],[T('Врезки'),s.cost_pierce]];if(s.cost_material>0)rows.push([T('Металл'),s.cost_material]);if(s.cost_fixed>0)rows.push([T('Подготовка'),s.cost_fixed]);
  $('#sCostRows').innerHTML=rows.map(r=>`<div class="kv"><span>${r[0]}</span><span class="num">${fmtN(r[1],0)} ${esc(c)}</span></div>`).join('');
  $('#sCount').textContent=fmtN(s.count);$('#sPierce').textContent=fmtN(s.pierces);
  $('#sCut').textContent=fmtN(s.cut_len_m,1)+' '+T('м');$('#sTime').textContent='≈ '+fmtTime(s.time_min);
  $('#sOpen').textContent=fmtN(s.open_area_pct,1)+' %';$('#sMass').textContent=fmtN(s.weight_kg,1)+' '+T('кг');
  const bs=s.by_size||[];$('#bSizes').hidden=!bs.length;
  if(bs.length){const mx=Math.max(...bs.map(b=>b.count));const list=bs.length>14?[...bs].sort((a,b)=>b.count-a.count).slice(0,14).sort((a,b)=>a.size-b.size):bs;
    $('#sSizes').innerHTML=list.map(b=>`<div class="sz"><span>Ø ${fmtN(b.size,b.size%1?1:0)}</span><i style="width:${Math.max(2,b.count/mx*100)}%"></i><span class="n">${fmtN(b.count)}</span></div>`).join('')+(bs.length>14?`<div class="sub">${T('Показаны 14 из {n}. Увеличьте шаг размеров',{n:bs.length})}</div>`:'')}
  const w=[...(s.warnings||[])];if(s.removed_edge>0)w.push(T('{n} отв. у края не поместились в поле и пропущены',{n:fmtN(s.removed_edge)}));
  $('#sWarn').innerHTML=w.length?`<ul class="warns">${w.map(t=>`<li>${esc(t)}</li>`).join('')}</ul>`:`<div class="okline">${esc(T(P.pattern==='lines'?'Ширина от {a} до {b} мм, перемычка не меньше {c} мм':'Размеры от {a} до {b} мм, перемычка не меньше {c} мм',{a:fmtN(s.size_min,1),b:fmtN(s.size_max,1),c:fmtN(P.bridge,1)}))}</div>`;
  $('#hud').textContent=T('{n} контуров · {p} %',{n:fmtN(s.count),p:fmtN(s.open_area_pct,1)});
}
function fmtTime(m){if(m<60)return fmtN(m,m<10?1:0)+' '+T('мин');const h=Math.floor(m/60);return `${h} ${T('ч')} ${Math.round(m-h*60)} ${T('мин')}`}
function esc(s){return String(s).replace(/[&<>"]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]))}

// ---------- Drawing ----------
const cv=$('#cv'),ctx=cv.getContext('2d');let dpr=1,W=0,H=0;
function resize(){const r=$('#wrap').getBoundingClientRect();dpr=Math.min(window.devicePixelRatio||1,2);W=r.width;H=r.height;cv.width=Math.max(1,Math.round(W*dpr));cv.height=Math.max(1,Math.round(H*dpr));draw()}
function base(){const pad=W<600?36:56;return Math.min((W-pad*2)/P.sheet_w,(H-pad*2)/P.sheet_h)}
function tf(){const s=base()*view.z;const ox=W/2-P.sheet_w*s/2+view.x,oy=H/2+P.sheet_h*s/2+view.y;return {s,ox,oy}}
function shapesPath(target,s,ox,oy){
  const g=geom;if(!g)return;let i=0;const n=g.length;
  while(i<n){const k=g[i];
    if(k===0){const x=ox+g[i+1]*s,y=oy-g[i+2]*s,r=g[i+3]*s/2;target.moveTo(x+r,y);target.arc(x,y,r,0,Math.PI*2);i+=4}
    else{const m=g[i+1];target.moveTo(ox+g[i+2]*s,oy-g[i+3]*s);for(let j=1;j<m;j++)target.lineTo(ox+g[i+2+j*2]*s,oy-g[i+3+j*2]*s);target.closePath();i+=2+m*2}
  }
}
function imgRect(){ // повторяет размещение картинки в ядре
  const iw=img.w,ih=img.h,w=P.sheet_w,h=P.sheet_h;let a,b;
  if(P.fit==='contain'){a=b=Math.min(w/iw,h/ih)}else if(P.fit==='stretch'){a=w/iw;b=h/ih}else{a=b=Math.max(w/iw,h/ih)}
  a*=P.img_scale;b*=P.img_scale;const dw=iw*a,dh=ih*b;return {left:w/2+P.img_dx-dw/2,top:h/2-P.img_dy-dh/2,dw,dh}
}
function draw(){
  if(!P)return;ctx.setTransform(dpr,0,0,dpr,0,0);ctx.clearRect(0,0,W,H);
  $('#empty').hidden=!!img;if(!img)return;
  const {s,ox,oy}=tf();const sw=P.sheet_w*s,sh=P.sheet_h*s,top=oy-sh;
  paintSheet(ctx,s,ox,oy,mode);
  // размерные линии
  ctx.save();ctx.strokeStyle=C.dim;ctx.fillStyle=C.t3;ctx.lineWidth=1;ctx.font='12px '+C.mono;
  const yb=oy+20,xl=ox-20;
  line(ox,oy+6,ox,yb+6);line(ox+sw,oy+6,ox+sw,yb+6);line(ox,yb,ox+sw,yb);tick(ox,yb);tick(ox+sw,yb);
  line(ox-6,oy,xl-6,oy);line(ox-6,top,xl-6,top);line(xl,top,xl,oy);tick(xl,oy);tick(xl,top);
  ctx.textAlign='center';ctx.textBaseline='middle';
  label(`${fmtN(P.sheet_w)} ${T('мм')}`,ox+sw/2,yb);
  ctx.save();ctx.translate(xl,top+sh/2);ctx.rotate(-Math.PI/2);label(`${fmtN(P.sheet_h)} ${T('мм')}`,0,0);ctx.restore();
  ctx.restore();
  function line(a,b,c,d){ctx.beginPath();ctx.moveTo(a+.5,b+.5);ctx.lineTo(c+.5,d+.5);ctx.stroke()}
  function tick(x,y){ctx.beginPath();ctx.moveTo(x-4+.5,y+4+.5);ctx.lineTo(x+4+.5,y-4+.5);ctx.stroke()}
  function label(t,x,y){const w=ctx.measureText(t).width+12;ctx.save();ctx.fillStyle=C.bg;ctx.fillRect(x-w/2,y-8,w,16);ctx.restore();ctx.fillText(t,x,y+.5)}
}
function paintSheet(c,s,ox,oy,m){
  const sw=P.sheet_w*s,sh=P.sheet_h*s,top=oy-sh;const sheetCol=$('#cSheet').value,backCol=$('#cBack').value;
  if(m==='metal'){
    const gr=c.createLinearGradient(ox,top,ox+sw,oy);gr.addColorStop(0,shade(sheetCol,12));gr.addColorStop(.55,sheetCol);gr.addColorStop(1,shade(sheetCol,-14));
    c.fillStyle=gr;c.fillRect(ox,top,sw,sh);
    c.beginPath();shapesPath(c,s,ox,oy);c.fillStyle=backCol;c.fill();
    c.strokeStyle='rgba(0,0,0,.35)';c.lineWidth=1;c.strokeRect(ox+.5,top+.5,sw-1,sh-1);
  }else if(m==='lines'){
    c.fillStyle=C.card;c.fillRect(ox,top,sw,sh);
    const b=P.border*s;c.save();c.setLineDash([4,4]);c.strokeStyle=C.frame;c.strokeRect(ox+b+.5,top+b+.5,sw-2*b-1,sh-2*b-1);c.restore();
    c.beginPath();shapesPath(c,s,ox,oy);c.strokeStyle=C.t1;c.lineWidth=Math.max(.5,Math.min(1,s*.15));c.stroke();
    c.strokeStyle='#e8833a';c.lineWidth=1;c.strokeRect(ox+.5,top+.5,sw-1,sh-1);
  }else{
    c.fillStyle='#fff';c.fillRect(ox,top,sw,sh);
    c.save();c.beginPath();c.rect(ox,top,sw,sh);c.clip();const r=imgRect();c.drawImage(img.canvas,ox+r.left*s,top+r.top*s,r.dw*s,r.dh*s);
    c.beginPath();shapesPath(c,s,ox,oy);c.strokeStyle='rgba(232,131,58,.85)';c.lineWidth=1;c.stroke();c.restore();
  }
}
function shade(hex,p){const n=parseInt(hex.slice(1),16);let r=n>>16,g=n>>8&255,b=n&255;const f=v=>Math.max(0,Math.min(255,Math.round(v+(p/100)*255)));return `rgb(${f(r)},${f(g)},${f(b)})`}

function bindStage(){
  $('#viewMode').addEventListener('click',e=>{const b=e.target.closest('button');if(!b)return;mode=b.dataset.v;$$('#viewMode button').forEach(x=>x.classList.toggle('on',x===b));draw()});
  $('#cSheet').oninput=draw;$('#cBack').oninput=draw;
  $('#bFitView').onclick=fitView;
  const pts=new Map();let last=null,pinch=null;
  cv.addEventListener('pointerdown',e=>{cv.setPointerCapture(e.pointerId);pts.set(e.pointerId,{x:e.clientX,y:e.clientY});cv.classList.add('drag');
    if(pts.size===2){const [a,b]=[...pts.values()];pinch={d:Math.hypot(a.x-b.x,a.y-b.y),z:view.z}}});
  cv.addEventListener('pointermove',e=>{if(!pts.has(e.pointerId))return;const p0=pts.get(e.pointerId);const p1={x:e.clientX,y:e.clientY};pts.set(e.pointerId,p1);
    if(pts.size===2&&pinch){const [a,b]=[...pts.values()];const d=Math.hypot(a.x-b.x,a.y-b.y);const r=cv.getBoundingClientRect();zoomAt((a.x+b.x)/2-r.left,(a.y+b.y)/2-r.top,pinch.z*d/pinch.d/view.z)}
    else if(pts.size===1){view.x+=p1.x-p0.x;view.y+=p1.y-p0.y;draw()}});
  const up=e=>{pts.delete(e.pointerId);if(pts.size<2)pinch=null;if(!pts.size)cv.classList.remove('drag')};
  cv.addEventListener('pointerup',up);cv.addEventListener('pointercancel',up);
  cv.addEventListener('wheel',e=>{e.preventDefault();const r=cv.getBoundingClientRect();zoomAt(e.clientX-r.left,e.clientY-r.top,Math.exp(-e.deltaY*0.0015))},{passive:false});
  cv.addEventListener('dblclick',fitView);
  $('#tabs').addEventListener('click',e=>{const b=e.target.closest('button');if(!b)return;$$('#tabs button').forEach(x=>x.classList.toggle('on',x===b));const t=b.dataset.t;
    $('#pSet').classList.toggle('active',t==='set');$('#pRight').classList.toggle('active',t!=='set');
    $$('#pRight [data-tab-part]').forEach(x=>x.classList.toggle('active',x.dataset.tabPart===t))});
  // файлы
  $('#bOpen').onclick=$('#bPick').onclick=$('#bReplace').onclick=()=>$('#file').click();
  $('#file').onchange=e=>{const f=e.target.files[0];if(f)openFile(f);e.target.value=''};
  $('#bSample').onclick=sample;
  let dc=0;
  window.addEventListener('dragenter',e=>{e.preventDefault();dc++;$('#app').classList.add('dropping')});
  window.addEventListener('dragleave',e=>{dc--;if(dc<=0){dc=0;$('#app').classList.remove('dropping')}});
  window.addEventListener('dragover',e=>e.preventDefault());
  window.addEventListener('drop',e=>{e.preventDefault();dc=0;$('#app').classList.remove('dropping');const f=e.dataTransfer.files[0];if(f)openFile(f)});
  $('#bZip').onclick=$('#bZip2').onclick=downloadZip;
  $('#bSaveOrder').onclick=$('#bSaveOrder2').onclick=saveOrder;
}
function zoomAt(mx,my,f){const nz=Math.max(.2,Math.min(80,view.z*f));f=nz/view.z;const cx=W/2+view.x,cy=H/2+view.y;view.x+=(mx-cx)*(1-f);view.y+=(my-cy)*(1-f);view.z=nz;draw()}
function fitView(){view={z:1,x:0,y:0};draw()}

// ---------- Files ----------
async function openFile(f){
  const name=f.name||'file';const bytes=new Uint8Array(await f.arrayBuffer());
  if(/\.(perfo|zip)$/i.test(name))return openOrder(bytes,name);
  await setImage(new Blob([bytes],{type:f.type||mimeOf(name)}),name,bytes);
}
function mimeOf(n){const e=(n.split('.').pop()||'').toLowerCase();return {jpg:'image/jpeg',jpeg:'image/jpeg',png:'image/png',webp:'image/webp',gif:'image/gif',bmp:'image/bmp'}[e]||'application/octet-stream'}
async function decodeImage(blob){
  try{return await createImageBitmap(blob)}catch(e){}
  return await new Promise((res,rej)=>{const u=URL.createObjectURL(blob);const i=new Image();i.onload=()=>res(i);i.onerror=()=>rej(new Error(T('формат не поддерживается')));i.src=u});
}
async function setImage(blob,name,bytes){
  let bmp;try{bmp=await decodeImage(blob)}catch(e){toast(T('Не удалось открыть картинку: ')+e.message,5000);return}
  const w0=bmp.width,h0=bmp.height,k=Math.min(1,1600/Math.max(w0,h0));const w=Math.max(1,Math.round(w0*k)),h=Math.max(1,Math.round(h0*k));
  const c=document.createElement('canvas');c.width=w;c.height=h;const x=c.getContext('2d',{willReadFrequently:true});x.fillStyle='#fff';x.fillRect(0,0,w,h);x.drawImage(bmp,0,0,w,h);
  const data=x.getImageData(0,0,w,h).data;const [p,n]=put(data);ex.pc_set_image(p,w,h);ex.pc_free(p,n);
  img={canvas:c,w,h,w0,h0,name,bytes,type:blob.type||mimeOf(name)};
  $('#chip').hidden=false;$('#chipName').textContent=name;$('#chipSize').textContent=`${w0} × ${h0} px`;
  if(S.lockRatio)S.ratio=P.sheet_w/P.sheet_h;$('#thumb').src=c.toDataURL('image/jpeg',.7);
  $('#bAspect').disabled=false;fitView();schedule();
}
async function sample(){
  const c=document.createElement('canvas');c.width=1200;c.height=720;const x=c.getContext('2d');
  const g=x.createLinearGradient(0,0,1200,0);g.addColorStop(0,'#f4f4f4');g.addColorStop(1,'#3a3a3a');x.fillStyle=g;x.fillRect(0,0,1200,720);
  const r=x.createRadialGradient(380,360,20,380,360,300);r.addColorStop(0,'#000');r.addColorStop(.7,'#222');r.addColorStop(1,'rgba(40,40,40,0)');x.fillStyle=r;x.beginPath();x.arc(380,360,300,0,7);x.fill();
  x.strokeStyle='#fff';x.lineWidth=46;x.beginPath();for(let i=0;i<=1200;i+=8){const y=560+70*Math.sin(i/120);i?x.lineTo(i,y):x.moveTo(i,y)}x.stroke();
  const blob=await new Promise(res=>c.toBlob(res,'image/png'));const bytes=new Uint8Array(await blob.arrayBuffer());
  await setImage(blob,T('пример')+'.png',bytes);
}
async function openOrder(bytes,name){
  const rc=call(ex.pc_zip_list,bytes);if(rc!==0){toast(T('Файл заказа повреждён или сжат другой программой'),5000);return}
  const list=JSON.parse(dec.decode(out()));const get=n=>{const e=list.find(e=>e.name===n||e.name.endsWith('/'+n));return e&&e.method===0?bytes.slice(e.offset,e.offset+e.len):null};
  const st=get('settings.json');if(!st){toast(T('В архиве нет settings.json'),5000);return}
  let obj;try{obj=JSON.parse(dec.decode(st))}catch(e){toast(T('settings.json не читается'),5000);return}
  P=Object.assign({},DEF,obj.params||obj,{lang:S.lang});if(S.lockRatio)S.ratio=P.sheet_w/P.sheet_h;changed();
  const src=list.find(e=>/^source\./.test(e.name)&&e.method===0);
  if(src){const b=bytes.slice(src.offset,src.offset+src.len);await setImage(new Blob([b],{type:mimeOf(src.name)}),src.name.replace(/^source/,(obj.source_name||'source').replace(/\.[^.]+$/,'')),b)}
  toast(T('Заказ открыт: ')+name);
}

// ---------- Export ----------
const FORMATS=[
  {id:'dxf',ext:'dxf',t:'DXF',n:'CypCut, AutoCAD · R12, окружности',on:true},
  {id:'nc',ext:'nc',t:'G-code .nc',n:'NCStudio, Unicut',on:true},
  {id:'cnc',ext:'cnc',t:'G-code .cnc',n:'Тот же код с расширением .cnc'},
  {id:'svg',ext:'svg',t:'SVG',n:'Inkscape, LightBurn, веб',on:true},
  {id:'pdf',ext:'pdf',t:'PDF',n:'Вектор для согласования и печати',on:true},
  {id:'eps',ext:'eps',t:'EPS',n:'CorelDRAW, Illustrator'},
  {id:'plt',ext:'plt',t:'PLT',n:'HPGL, плоттеры'},
  {id:'png',ext:'png',t:'PNG',n:'Вид для клиента',on:true},
  {id:'csv',ext:'csv',t:'CSV',n:'Сводка для Excel'},
  {id:'report',ext:'txt',t:'Паспорт заказа',n:'Текст с расчётом',on:true},
];
function renderFormats(){
  let sel={};try{sel=JSON.parse(localStorage.getItem('perfocut.fmts')||'null')||{}}catch(e){}
  $('#fmtList').innerHTML=FORMATS.map(f=>`<div class="fmt"><input type="checkbox" id="f_${f.id}" ${(f.id in sel?sel[f.id]:f.on)?'checked':''}><label for="f_${f.id}"><b>${T(f.t)}</b><span>${T(f.n)}</span></label><button class="btn ghost icon" data-dl="${f.id}" title="${T('Скачать')} ${T(f.t)}" aria-label="${T('Скачать')} ${T(f.t)}"><svg class="i" viewBox="0 0 24 24"><path d="M12 15V3"/><path d="M7 10l5 5 5-5"/><path d="M5 21h14"/></svg></button></div>`).join('');
}
function bindFormats(){
  $('#fmtList').addEventListener('change',()=>{const s={};FORMATS.forEach(f=>s[f.id]=$('#f_'+f.id).checked);try{localStorage.setItem('perfocut.fmts',JSON.stringify(s))}catch(e){}});
  $('#fmtList').addEventListener('click',async e=>{const b=e.target.closest('[data-dl]');if(!b)return;if(!ready())return;const f=FORMATS.find(x=>x.id===b.dataset.dl);
    try{save(`${baseName()}.${f.ext}`,await bytesOf(f),mimeFor(f.ext))}catch(err){toast(T('Ошибка экспорта: ')+err.message,5000)}});
}
function ready(){if(!img){toast(T('Сначала загрузите картинку'));return false}if(!stats||!stats.count){toast(T('Узор пустой: проверьте настройки'));return false}return true}
function baseName(){const s=v=>String(v||'').trim().replace(/[\\/:*?"<>|]+/g,'_').replace(/\s+/g,'_');return [s(P.name)||'perfocut',s(P.order_no)].filter(Boolean).join('_')}
function mimeFor(e){return {dxf:'application/dxf',svg:'image/svg+xml',pdf:'application/pdf',png:'image/png',csv:'text/csv',txt:'text/plain',zip:'application/zip',perfo:'application/octet-stream',eps:'application/postscript'}[e]||'application/octet-stream'}
async function bytesOf(f){if(f.id==='png')return await renderPng();return exportFmt(f.id)}
async function renderPng(maxSide=4000){
  const k=Math.min(6,maxSide/Math.max(P.sheet_w,P.sheet_h));const w=Math.round(P.sheet_w*k),h=Math.round(P.sheet_h*k);
  const c=document.createElement('canvas');c.width=w;c.height=h;const x=c.getContext('2d');paintSheet(x,k,0,h,'metal');
  const blob=await new Promise(res=>c.toBlob(res,'image/png'));return new Uint8Array(await blob.arrayBuffer());
}
async function downloadZip(){
  if(!ready())return;const sel=FORMATS.filter(f=>$('#f_'+f.id).checked);if(!sel.length){toast(T('Отметьте хотя бы один формат'));return}
  toast(T('Собираю архив…'),1500);await new Promise(r=>setTimeout(r,30));
  try{const files=[];const b=baseName();for(const f of sel)files.push([`${b}.${f.ext}`,await bytesOf(f)]);save(`${b}.zip`,zipBuild(files),'application/zip')}catch(e){toast(T('Ошибка экспорта: ')+e.message,5000)}
}
async function saveOrder(){
  if(!img){toast(T('Сначала загрузите картинку'));return}
  try{
    const ext=(img.name.split('.').pop()||'png').toLowerCase().replace(/[^a-z0-9]/g,'')||'png';
    const settings=JSON.parse(dec.decode(exportFmt('settings')));settings.source_name=img.name;settings.saved=new Date().toISOString();
    const files=[['settings.json',enc.encode(JSON.stringify(settings,null,2))],[`source.${ext}`,img.bytes]];
    if(stats&&stats.count){files.push(['preview.png',await renderPng(2400)],['report.txt',exportFmt('report')],['stats.json',exportFmt('stats')],['cut.dxf',exportFmt('dxf')])}
    save(`${baseName()}.perfo`,zipBuild(files),mimeFor('perfo'));
  }catch(e){toast(T('Не удалось сохранить заказ: ')+e.message,5000)}
}
function save(name,bytes,mime){
  if(window.AndroidBridge&&AndroidBridge.saveFile){
    let s='';const ch=0x8000;for(let i=0;i<bytes.length;i+=ch)s+=String.fromCharCode.apply(null,bytes.subarray(i,i+ch));
    const r=AndroidBridge.saveFile(name,mime,btoa(s));
    if(!r||r[0]==='!')toast(T('Не удалось сохранить: ')+(r?r.slice(1):name),5000);else toast(T('Сохранено: ')+r,4000);return;
  }
  const a=document.createElement('a');a.href=URL.createObjectURL(new Blob([bytes],{type:mime}));a.download=name;document.body.appendChild(a);a.click();
  setTimeout(()=>{URL.revokeObjectURL(a.href);a.remove()},2000);toast(T('Скачано: ')+name);
}
let tT=0;function toast(t,ms=2500){const e=$('#toast');e.textContent=t;e.classList.add('show');clearTimeout(tT);tT=setTimeout(()=>e.classList.remove('show'),ms)}
// ---------- Тема ----------
const mqDark=matchMedia('(prefers-color-scheme: dark)');
function systemDark(){try{if(AB&&AB.isSystemDark)return !!AB.isSystemDark()}catch(e){}return mqDark.matches}
function applyTheme(){
  const t=S.theme==='system'?(systemDark()?'dark':'light'):S.theme;
  document.documentElement.dataset.theme=t;
  const cs=getComputedStyle(document.documentElement),v=n=>cs.getPropertyValue(n).trim();
  C={dim:v('--dim'),t1:v('--t1'),t3:v('--t3'),bg:v('--bg'),card:v('--card'),frame:v('--frame'),mono:v('--mono')};
  const bg0=v('--bg0');$('meta[name=theme-color]').content=bg0;
  try{AB&&AB.setTheme&&AB.setTheme(bg0,t==='light')}catch(e){}
  if(P)draw();
}
mqDark.addEventListener&&mqDark.addEventListener('change',()=>{if(S.theme==='system')applyTheme()});

// ---------- Язык ----------
const NOTR='script,style,textarea,#fmtList,#sSizes,#sWarn,#sCostRows,#sCost,.fig b,#hud,#chipName,#chipSize,#toast,#mSheet,#mMetal';
function applyLang(){
  document.documentElement.lang=S.lang;
  document.title=T('PerfoCut — перфорация для лазерной резки');
  const tw=document.createTreeWalker(document.body,NodeFilter.SHOW_TEXT);
  for(let n=tw.nextNode();n;n=tw.nextNode()){
    if(n.__ru===undefined){const t=n.nodeValue.trim();const pe=n.parentElement;
      n.__ru=(t&&/[А-Яа-яЁё]/.test(t)&&!(pe&&pe.closest(NOTR)))?t:null;
      if(n.__ru){n.__pre=n.nodeValue.match(/^\s*/)[0];n.__post=n.nodeValue.match(/\s*$/)[0]}}
    if(n.__ru)n.nodeValue=n.__pre+T(n.__ru)+n.__post;
  }
  $$('[title],[placeholder],[aria-label]').forEach(el=>{if(el.closest('#fmtList'))return;el.__ruA=el.__ruA||{};
    for(const a of ['title','placeholder','aria-label']){if(!el.hasAttribute(a))continue;if(!(a in el.__ruA))el.__ruA[a]=el.getAttribute(a);el.setAttribute(a,T(el.__ruA[a]))}});
}
function setLang(l){S.lang=l;saveS();P.lang=l;if(P.name==='Панель'||P.name==='Panel')P.name=l==='en'?'Panel':'Панель';applyLang();renderFormats();syncInputs();syncSettings();renderStats();draw();schedule()}

// ---------- Окно настроек ----------
function bindSettings(){
  $('#bSettings').onclick=()=>{syncSettings();openModal('mSettings')};
  $('#bInfo').onclick=()=>openModal('mInfo');
  $('#setTheme').addEventListener('click',e=>{const b=e.target.closest('button');if(!b)return;S.theme=b.dataset.v;saveS();applyTheme();syncSettings()});
  $('#setLang').addEventListener('click',e=>{const b=e.target.closest('button');if(!b||b.dataset.v===S.lang)return;setLang(b.dataset.v)});
  $('#setVibro').onchange=e=>{S.vibro=e.target.checked;saveS();if(S.vibro)haptic('click',true)};
}
function syncSettings(){$$('#setTheme button').forEach(b=>b.classList.toggle('on',b.dataset.v===S.theme));$$('#setLang button').forEach(b=>b.classList.toggle('on',b.dataset.v===S.lang));$('#setVibro').checked=!!S.vibro}

// ---------- Всплывающие окна: затемнение, закрытие по фону ----------
const modals=[];
function openModal(id){const m=$('#'+id);if(modals.includes(m))return;if(isText(document.activeElement))document.activeElement.blur();
  m.__ret=document.activeElement;m.hidden=false;modals.push(m);requestAnimationFrame(()=>m.classList.add('show'))}
function closeModal(){const m=modals.pop();if(!m)return false;m.classList.remove('show');setTimeout(()=>{if(!m.classList.contains('show'))m.hidden=true},200);
  try{m.__ret&&m.__ret.focus&&m.__ret.focus({preventScroll:true})}catch(e){}return true}
function bindModals(){$$('.modal').forEach(m=>{
  m.addEventListener('pointerdown',e=>{m.__down=e.target===m});
  m.addEventListener('click',e=>{const top=modals[modals.length-1]===m;if(!top)return;
    if((e.target===m&&m.__down)||e.target.closest('[data-close]'))closeModal();m.__down=false})})}

// ---------- «Назад» и Esc: клавиатура → окно → свернуть ----------
function isText(el){return !!el&&(el.tagName==='TEXTAREA'||(el.tagName==='INPUT'&&!/^(checkbox|radio|range|color|button|file|submit)$/.test(el.type)))}
function back(){const a=document.activeElement;
  if(isText(a)){a.blur();if(!modals.length)return false}
  if(modals.length){closeModal();return true}
  return false}
function bindKeys(){document.addEventListener('keydown',e=>{if(e.key!=='Escape'&&e.key!=='Esc')return;e.preventDefault();
  const a=document.activeElement;
  if(isText(a)){a.blur();return}
  if(modals.length){closeModal();return}
  if(isExe)fetch('/minimize',{cache:'no-store'}).catch(()=>{})})}

// ---------- Вибрация ----------
let lastH=0;
function haptic(kind='click',force){if(!S.vibro&&!force)return;const now=performance.now();if(now-lastH<(kind==='tick'?28:60))return;lastH=now;
  try{if(AB&&AB.vibrate){AB.vibrate(kind);return}}catch(e){}
  try{if(navigator.vibrate&&matchMedia('(pointer:coarse)').matches)navigator.vibrate(kind==='tick'?4:10)}catch(e){}}
function bindHaptics(){
  document.addEventListener('click',e=>{if(e.target.closest('button,summary,a,label,input[type=checkbox],input[type=color]'))haptic('click')},true);
  document.addEventListener('input',e=>{if(e.target.type==='range')haptic('tick')},true);
  document.addEventListener('change',e=>{if(e.target.type==='number')haptic('tick')},true);
}

// ---------- Клавиатура на телефоне: поле по центру ----------
function bindKeyboard(){
  const mq=matchMedia('(max-width:900px)');let maxH=innerHeight,lastW=innerWidth;
  const vh=()=>window.visualViewport?visualViewport.height:innerHeight;
  const center=()=>{const el=document.activeElement;if(!mq.matches||!isText(el))return;
    const top=Math.max(0,$('#stage').getBoundingClientRect().bottom),h=vh(),r=el.getBoundingClientRect();
    const target=top+(h-top)/2-r.height/2;if(Math.abs(r.top-target)>8)window.scrollBy({top:r.top-target,behavior:'smooth'})};
  const update=()=>{if(innerWidth!==lastW){lastW=innerWidth;maxH=0}maxH=Math.max(maxH,innerHeight);
    const open=isText(document.activeElement)&&vh()<maxH*0.8;
    if(document.body.classList.contains('kb')!==open){document.body.classList.toggle('kb',open);setTimeout(resize,220)}
    if(open)setTimeout(center,240)};
  window.addEventListener('resize',update);window.visualViewport&&visualViewport.addEventListener('resize',update);
  document.addEventListener('focusin',e=>{if(isText(e.target)&&mq.matches){setTimeout(update,60);setTimeout(center,380)}});
  document.addEventListener('focusout',()=>setTimeout(update,120));
}

window.PerfoCut={openFile,back};
boot();
