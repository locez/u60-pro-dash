'use strict';
(() => {
  const $=id=>document.getElementById(id);
  const q=selector=>[...document.querySelectorAll(selector)];
  const scenarios={
    normal:{description:'蜂窝网络正常，热点设备可以访问远端网络。',online:true,reachable:true,battery:82},
    'peer-offline':{description:'U60 仍在线，但子网路由节点离线。',online:true,reachable:false,battery:82},
    'no-internet':{description:'蜂窝连接断开，局域网热点仍保持开启。',online:false,reachable:false,battery:82},
    'low-battery':{description:'网络连接正常，设备电量降至 12%。',online:true,reachable:true,battery:12},
  };
  const baseline=[36,47,39,65,62,88,72,90,107,84,91,98,74,112,106,128];
  const rates=[128.6,134.2,121.8,148.3,137.4,126.2,119.6,132.8];
  const appearance=window.U60Appearance;
  const state={page:'network',scenario:'normal',...appearance.read(),brightness:80,motion:!matchMedia('(prefers-reduced-motion: reduce)').matches,asleep:false,ts:true,session:true,history:[...baseline],tick:0,probeTimer:null,autoSleep:0,lastInput:Date.now()};
  let toastTimer;
  const text=(id,value)=>{$(id).textContent=String(value);};
  const spec=()=>scenarios[state.scenario];
  function toast(value){clearTimeout(toastTimer);text('screen-toast',value);$('screen-toast').hidden=false;toastTimer=setTimeout(()=>{$('screen-toast').hidden=true;},2600);}
  function hideToast(){clearTimeout(toastTimer);$('screen-toast').hidden=true;}
  function nav(root){
    const selected=root==='device-settings'?'device':root==='tailnet'?'network-menu':root==='signal'?'network':root;
    q('.page-picker [data-page]').forEach(b=>b.setAttribute('aria-pressed',String(b.dataset.page===selected)));
    q('.screen-nav [data-page]').forEach(b=>{if(b.dataset.page===selected)b.setAttribute('aria-current','page');else b.removeAttribute('aria-current');});
  }
  function reveal(id){q('.screen-page').forEach(p=>p.hidden=p.id!=='page-'+id);$('page-'+id).scrollTop=0;}
  function showStatic(id){features.cancelDialog();hideToast();if(state.asleep)power(false);state.page=id;reveal(id);nav(id);updateReadings();}
  const features=window.createFeatureUI({
    showCatalog(root){hideToast();if(state.asleep)power(false);state.page='catalog';reveal('catalog');nav(root);},showStatic,toast,
    scenario:setScenario,appearance:setAppearance,brightness:setBrightness,
    getScreenPreferences:()=>({theme:state.theme==='light'?'浅色':'深色',accent:state.accent,brightness:state.brightness,sleep:state.autoSleep?state.autoSleep+' 秒':'从不'}),
    asleep:()=>state.asleep,connected:()=>state.session,
    tailscale(on){state.ts=on;updateReadings();},
    session(on){state.session=on;updateReadings();},
    autoSleep(seconds){state.autoSleep=seconds;state.lastInput=Date.now();},
    reset:reset,
  });
  function openPage(id){features.cancelDialog();if(id==='network-menu'||id==='apps'||id==='device-settings')features.root(id);else showStatic(id);state.lastInput=Date.now();}
  function setScenario(id){if(!scenarios[id])return;state.scenario=id;state.ts=true;state.session=true;state.tick=0;state.history=[...baseline];clearTimeout(state.probeTimer);state.probeTimer=null;$('probe-target').disabled=false;text('probe-label','检测目标连通性');text('probe-result','尚未检测');$('probe-result').className='probe-result';q('[data-scenario]').forEach(b=>b.setAttribute('aria-pressed',String(b.dataset.scenario===id)));text('scenario-description',spec().description);updateReadings();}
  function setAppearance(patch,persist=true){
    const changed={};
    if(['light','dark'].includes(patch.theme)){state.theme=patch.theme;changed.theme=state.theme==='light'?'浅色':'深色';}
    const accent=appearance.normalize(patch.accent);
    if(accent){state.accent=accent;changed.accent=accent;}
    appearance.apply($('device-screen'),state);
    q('button[data-theme]').forEach(b=>b.setAttribute('aria-pressed',String(b.dataset.theme===state.theme)));
    q('button[data-accent]').forEach(b=>b.setAttribute('aria-pressed',String(b.dataset.accent===state.accent)));
    $('screen-theme-toggle').firstChild.textContent=state.theme==='dark'?'深色 ':'浅色 ';
    $('accent-picker').value=state.accent;$('accent-hex').value=state.accent.toUpperCase();
    $('accent-hex').setCustomValidity('');$('accent-hex').removeAttribute('aria-invalid');
    const saved=!persist||appearance.save(state);
    text('accent-help',saved?'即时生效 · 自动记住配色':'已生效 · 浏览器未允许保存，刷新后不会记住此次修改');
    features.syncPreferences(changed);
    return saved;
  }
  function setTheme(theme){setAppearance({theme});}
  function setAccent(accent){setAppearance({accent});}
  function setBrightness(value){state.brightness=Math.max(20,Math.min(100,Number(value)));$('brightness').value=state.brightness;text('brightness-value',state.brightness+'%');$('screen-dimmer').style.opacity=state.asleep?0:(100-state.brightness)/250;features.syncPreferences({brightness:state.brightness});}
  function power(asleep=!state.asleep){features.cancelDialog();state.asleep=asleep;$('awake-ui').hidden=asleep;$('awake-ui').inert=asleep;$('device-screen').classList.toggle('is-asleep',asleep);$('physical-power').setAttribute('aria-label','模拟电源键：'+(asleep?'打开屏幕':'关闭屏幕'));$('stage-power').querySelector('span').textContent=asleep?'亮屏':'熄屏';text('interaction-hint',asleep?'屏幕已关闭，点击侧边电源键或按空格唤醒':'点击底部切页，也可在屏幕上左右滑动');setBrightness(state.brightness);state.lastInput=Date.now();if(!asleep)updateReadings();}
  function paintChart(){const online=spec().online&&state.session;$('chart-empty').hidden=online;const values=state.history;const max=Math.max(180,...values);const points=values.map((v,i)=>[i*288/(values.length-1),45-v/max*40]);const line=points.map(([x,y],i)=>(i?'L':'M')+x.toFixed(1)+' '+y.toFixed(1)).join(' ');$('chart-line').setAttribute('d',line);$('chart-area').setAttribute('d',line+' L288 48 L0 48 Z');}
  function badge(id,label,kind){text(id,label);$(id).className='badge '+kind;}
  function updateReadings(){
    const s=spec(),online=s.online&&state.session,tsOnline=online&&state.ts,reachable=tsOnline&&s.reachable,low=s.battery<20;
    text('battery-percent',s.battery+'%');text('device-battery',s.battery);$('battery-fill').style.width=s.battery+'%';$('device-battery-fill').style.width=s.battery+'%';
    $('battery-fill').style.background=low?'var(--screen-warn)':'';$('device-battery-fill').style.background=low?'var(--screen-warn)':'';$('device-battery').style.color=low?'var(--screen-warn)':'';
    text('battery-message',low?'电量较低，建议连接充电器。':'电量充足，继续出发。');
    text('status-network',online?'5G':'—');badge('network-badge',!state.session?'会话过期':online?'已连接':'无服务',online?'good':'warning');
    text('radio-label',online?'5G SA · n78':'等待网络');text('download-rate',online?rates[state.tick%rates.length].toFixed(1):'—');text('upload-rate',online?(16.4+(state.tick%4)*.8).toFixed(1):'—');
    text('signal-rsrp',online?'−86':'—');text('signal-quality',online?'良好':'暂无');text('detail-rsrp',online?'−86':'—');text('detail-sinr',online?'21.4':'—');text('detail-rsrq',online?'−9':'—');
    text('signal-caption',online?'5G SA · n78':'未连接蜂窝网络');text('signal-summary',online?'当前信号良好':'暂时没有信号读数');text('signal-help',online?'调整设备位置，可观察信号变化。实际速率还受网络负载影响。':'等待蜂窝网络恢复后，再查看信号质量。');$('signal-indicator').className='status-dot '+(online?'good':'warning');
    text('tailnet-shortcut-title',tsOnline?'Tailscale 已连接':state.ts?'Tailscale 等待网络':'Tailscale 已停止');text('tailnet-shortcut-sub','子网路由与连通性检测');
    badge('tailnet-badge',reachable?'可访问':'未连通',reachable?'good':'warning');
    text('tailnet-message',reachable?'通过 Tailscale 访问目标网络。':tsOnline?'子网路由节点暂时离线。':'Tailscale 连接暂时中断。');text('tailnet-explanation',reachable?'热点里的设备可直接访问远端网络。':tsOnline?'请检查发布子网路由的设备是否在线。':online?'启用 Tailscale 后再检查网络连接。':'等待 U60 恢复网络连接。');
    text('tailscale-state',tsOnline?'已连接':state.ts?'等待网络':'已停止');$('tailscale-state').className=tsOnline?'tone-good':'tone-warning';text('tailnet-peer-state',reachable?'在线':tsOnline?'离线':'未知');
    $('route-link-a').classList.toggle('disconnected',!tsOnline);$('route-link-b').classList.toggle('disconnected',!reachable);$('tailnet-map-node').style.color=reachable?'':'var(--screen-warn)';paintChart();
  }
  function reset(){features.reset();state.motion=!matchMedia('(prefers-reduced-motion: reduce)').matches;state.autoSleep=0;state.lastInput=Date.now();setScenario('normal');setBrightness(80);$('motion-toggle').setAttribute('aria-checked',String(state.motion));power(false);showStatic('network');$('feature-search').value='';renderFeatureSearch();}
  function renderFeatureSearch(){const query=$('feature-search').value.trim().toLowerCase();const matches=features.features.filter(f=>(f.title+' '+f.description+' '+window.U60Catalog.groups.find(g=>g.id===f.group).title+' '+f.id).toLowerCase().includes(query));const target=$('feature-results');target.replaceChildren();for(const f of matches){const b=document.createElement('button');b.type='button';const title=document.createElement('span');title.textContent=f.title;const group=document.createElement('small');group.textContent=window.U60Catalog.groups.find(g=>g.id===f.group).title;b.append(title,group);b.addEventListener('click',()=>features.feature(f.id));target.append(b);}if(!matches.length){const p=document.createElement('p');p.textContent='没有匹配的功能';target.append(p);}text('feature-count',matches.length+' / '+features.features.length);}
  document.addEventListener('click',e=>{
    const p=e.target.closest('[data-page]');if(p){openPage(p.dataset.page);return;}
    const group=e.target.closest('[data-group]');if(group){features.group(group.dataset.group);return;}
    const root=e.target.closest('[data-root]');if(root){features.root(root.dataset.root);return;}
    const open=e.target.closest('[data-open]');if(open){showStatic(open.dataset.open);return;}
    const scenario=e.target.closest('[data-scenario]');if(scenario){setScenario(scenario.dataset.scenario);return;}
    const theme=e.target.closest('button[data-theme]');if(theme){setTheme(theme.dataset.theme);return;}
    const accent=e.target.closest('button[data-accent]');if(accent)setAccent(accent.dataset.accent);
  });
  for(const preset of appearance.presets){
    const b=document.createElement('button');b.type='button';b.dataset.accent=preset.color;
    b.setAttribute('aria-label',preset.name+'强调色');b.setAttribute('aria-pressed','false');
    b.style.setProperty('--swatch',preset.color);
    const dot=document.createElement('i');dot.setAttribute('aria-hidden','true');
    const label=document.createElement('span');label.textContent=preset.name;b.append(dot,label);$('accent-presets').append(b);
  }
  $('accent-picker').addEventListener('input',e=>setAccent(e.target.value));
  $('accent-hex').addEventListener('input',e=>{
    const raw=e.target.value,accent=appearance.normalize(raw);
    if(accent){const caret=e.target.selectionStart;setAccent(accent);e.target.value=raw;e.target.setSelectionRange(caret,caret);return;}
    e.target.setCustomValidity('请输入 3 位或 6 位 HEX 色值');e.target.setAttribute('aria-invalid','true');
    text('accent-help','请输入 HEX 色值，例如 #2563EB 或 #F80');
  });
  $('accent-hex').addEventListener('change',e=>{const accent=appearance.normalize(e.target.value);if(accent)setAccent(accent);});
  $('accent-hex').addEventListener('keydown',e=>{if(e.key==='Escape'){setAccent(state.accent);e.target.blur();}});
  $('reset-accent').addEventListener('click',()=>setAccent(appearance.defaults.accent));
  document.addEventListener('pointerdown',()=>{state.lastInput=Date.now();},true);
  $('motion-toggle').addEventListener('click',()=>{state.motion=!state.motion;$('motion-toggle').setAttribute('aria-checked',String(state.motion));});
  $('reset-preview').addEventListener('click',reset);
  $('feature-search').addEventListener('input',renderFeatureSearch);
  $('brightness').addEventListener('input',e=>setBrightness(e.target.value));
  $('screen-theme-toggle').addEventListener('click',()=>setTheme(state.theme==='dark'?'light':'dark'));
  $('physical-power').addEventListener('click',()=>power());$('stage-power').addEventListener('click',()=>power());$('screen-sleep').addEventListener('click',()=>power(true));
  $('probe-target').addEventListener('click',()=>{clearTimeout(state.probeTimer);$('probe-target').disabled=true;text('probe-label','正在检测…');text('probe-result','检测 192.168.1.1');state.probeTimer=setTimeout(()=>{const reachable=spec().online&&spec().reachable&&state.ts&&state.session;const label=reachable?'192.168.1.1 · 46 ms · 可访问':'192.168.1.1 · 暂不可达';text('probe-result',label);$('probe-result').className='probe-result '+(reachable?'tone-good':'tone-warning');$('probe-target').disabled=false;text('probe-label','重新检测');state.probeTimer=null;},700);});
  const primaryPages=['network','network-menu','apps','device'];
  function shiftPage(delta){const current=document.querySelector('.screen-nav [aria-current="page"]')?.dataset.page||'network';openPage(primaryPages[(primaryPages.indexOf(current)+delta+primaryPages.length)%primaryPages.length]);}
  let pointer=null;
  $('device-screen').addEventListener('pointerdown',e=>{if(state.asleep||e.target.closest('button,input,select,textarea')||!$('screen-dialog').hidden)return;pointer={x:e.clientX,y:e.clientY,id:e.pointerId};});
  $('device-screen').addEventListener('pointerup',e=>{if(!pointer||pointer.id!==e.pointerId)return;const dx=e.clientX-pointer.x,dy=e.clientY-pointer.y;pointer=null;if(Math.abs(dx)>50&&Math.abs(dy)<35)shiftPage(dx<0?1:-1);});
  $('device-screen').addEventListener('pointercancel',()=>{pointer=null;});
  document.addEventListener('keydown',e=>{state.lastInput=Date.now();if(e.target.closest('input,textarea,select,button')||!$('screen-dialog').hidden)return;if(e.key===' '){e.preventDefault();power();}else if(e.key==='ArrowRight'){e.preventDefault();shiftPage(1);}else if(e.key==='ArrowLeft'){e.preventDefault();shiftPage(-1);}else if(e.key==='Escape'&&state.page==='catalog'){features.back();}});
  setInterval(()=>{if(document.hidden||state.asleep)return;if(state.autoSleep&&Date.now()-state.lastInput>state.autoSleep*1000){power(true);return;}if(!state.motion||!state.session)return;state.tick++;if(spec().online){state.history.shift();state.history.push(rates[state.tick%rates.length]);}updateReadings();},2000);
  $('motion-toggle').setAttribute('aria-checked',String(state.motion));renderFeatureSearch();setAppearance({theme:state.theme,accent:state.accent},false);setScenario('normal');setBrightness(80);
  function openFragment(){const fragment=location.hash.slice(1);const requested=fragment==='tailscale-home'?'tailscale-network':fragment;if(features.features.some(f=>f.id===requested))features.feature(requested);}
  window.addEventListener('hashchange',openFragment);openFragment();
  // Inspection hook for local browser validation; all state remains synthetic.
  window.U60Preview={features,openPage,setScenario,setTheme,setAccent,reset,state};
})();
