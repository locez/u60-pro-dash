'use strict';
(() => {
  const tr = globalThis.OpenU60I18n?.t || ((s,...v)=>s.replace(/\{(\d+)\}/g,(match,i)=>Number(i)<v.length?String(v[i]):match));
  const UI=OpenU60UI, API=OpenU60Console, {el,button}=UI;
  const {input,form,submit,service}=OpenU60Services, route='network/proxy';
  const active=()=>OpenU60Navigation.current.key===route;
  const directory=UI.panel(route,'proxy-directory',tr('Proxy services'),tr('Each service has its own configuration, installation and start/stop controls. Services can run together when their ports and traffic rules do not conflict.'));
  const cards=el('div','proxy-cards');directory.append(cards);
  let catalogEpoch=0, catalogLoaded=false;
  async function loadDirectory(){
    const epoch=++catalogEpoch;
    try {
      const data=await UI.request('/api/proxy/services');
      if(epoch!==catalogEpoch||API.locked||!active())return;
      catalogLoaded=true;cards.replaceChildren();
      for(const item of data.services||[]){
        const card=el('article','proxy-card'),name=el('h3','',item.name||item.id),state=el('span','service-tag',item.running?tr('Running'):item.installed?tr('Stopped'):tr('Not installed'));
        const title=el('div','proxy-card-title');title.append(name,state);card.append(title,el('p','panel-note',item.local_endpoint||tr('Independent proxy service')));
        if(item.id==='mihomo')card.append(button(tr('Manage mihomo'),()=>{proxy.panel.hidden=false;proxy.panel.scrollIntoView({behavior:'smooth',block:'start'});proxy.panel.querySelector('h2').focus({preventScroll:true});},'primary'));
        cards.append(card);
      }
      if(!cards.children.length)cards.append(el('p','panel-note',tr('No supported proxy services were reported. Update the router backend.')));
    }catch{if(epoch===catalogEpoch&&!API.locked)cards.replaceChildren(el('p','panel-note',tr('Service list unavailable. Refresh to try again.')));}
  }
  directory.append(button(tr('Refresh services'),loadDirectory));
  UI.track({route,load:loadDirectory,refresh:loadDirectory,clear:()=>{catalogEpoch++;catalogLoaded=false;cards.replaceChildren();},get loaded(){return catalogLoaded;}});
  window.addEventListener('dashboard:navigate',()=>{if(!active()){catalogEpoch++;catalogLoaded=false;cards.replaceChildren();}});

  const proxy=service('proxy','mihomo',tr('Manage this service’s YAML configuration, routing mode and proxy groups.'),{endpoint:'proxy/mihomo',route});
  directory.after(proxy.panel);proxy.panel.querySelector('h2').tabIndex=-1;
  const nav=el('div','proxy-sections');nav.setAttribute('role','tablist');nav.setAttribute('aria-label',tr('mihomo management'));proxy.content.append(nav);
  const panes=new Map(),tabs=[];
  function pane(id,label){
    const node=el('div','proxy-pane');node.id=`proxy-pane-${id}`;node.setAttribute('role','tabpanel');
    const tab=button(label,()=>show(id));tab.id=`proxy-tab-${id}`;tab.setAttribute('role','tab');tab.setAttribute('aria-controls',node.id);node.setAttribute('aria-labelledby',tab.id);node.tabIndex=0;tabs.push([id,tab]);nav.append(tab);proxy.content.append(node);panes.set(id,node);return node;
  }
  function show(id){for(const[key,node]of panes)node.hidden=key!==id;for(const[key,tab]of tabs){tab.setAttribute('aria-selected',String(key===id));tab.tabIndex=key===id?0:-1;}}
  nav.addEventListener('keydown',event=>{if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;event.preventDefault();let n=tabs.findIndex(([,tab])=>tab===document.activeElement);n=event.key==='Home'?0:event.key==='End'?tabs.length-1:(n+(event.key==='ArrowRight'?1:-1)+tabs.length)%tabs.length;show(tabs[n][0]);tabs[n][1].focus();});
  const overview=pane('overview',tr('Overview')),configuration=pane('configuration',tr('Configuration')),groupPane=pane('groups',tr('Proxy groups')),installation=pane('installation',tr('Installation'));show('overview');
  for(const[action,label,explanation,predicate]of[
    ['enable',tr('Start mihomo'),tr('Start only mihomo with its saved configuration. Other proxy services are not stopped.'),d=>d.installed&&d.configured&&!d.running],
    ['disable',tr('Stop mihomo'),tr('Stop only mihomo and remove the traffic rules it owns. Other services keep running.'),d=>d.running||d.enabled],
    ['restart',tr('Restart mihomo'),tr('Restart only mihomo. Its existing proxy connections will close.'),d=>d.running]
  ])proxy.actions.append(proxy.guard(button(label,()=>proxy.write(action,{},explanation,{label,after:loadDirectory}),action==='enable'?'primary':''),predicate));

  const pf=form(overview,'proxy-policy',tr('Runtime settings'),tr('Stop this service before changing these settings. They override the corresponding fields in the imported YAML.'));
  const pm=input(pf,'proxy-mode',tr('Mode'),{values:[['rule',tr('Rule')],['global',tr('Global')],['direct',tr('Direct')]]}),pc=input(pf,'proxy-capture',tr('Traffic scope'),{values:[['local',tr('Local HTTP / SOCKS clients')],['hotspot',tr('Wi-Fi hotspot traffic')]],help:tr('Tailscale and existing local subnet routes keep priority. USB management is excluded.')});
  const port=input(pf,'proxy-port',tr('Mixed HTTP / SOCKS port'),{type:'number',required:true,help:tr('Loopback only. Choose a free port for this service.')});port.min=1024;port.max=65535;
  const ipv6=input(pf,'proxy-ipv6',tr('IPv6'),{values:[['true',tr('Enabled')],['false',tr('Disabled')]]}),concurrent=input(pf,'proxy-concurrent',tr('Concurrent TCP connections'),{values:[['false',tr('Disabled')],['true',tr('Enabled')]]});
  proxy.guard(submit(pf,tr('Save runtime settings'),()=>proxy.write('settings',{mode:pm.value,capture:pc.value,mixed_port:Number(port.value),ipv6:ipv6.value==='true',tcp_concurrent:concurrent.value==='true'},tr('Save settings for this service’s next start.'),{method:'PUT',node:pf})),d=>!d.running);

  const imported=form(configuration,'proxy-import',tr('mihomo configuration'),tr('Edit native YAML or JSON: proxies, proxy-groups, providers, rules, DNS and sniffing. Device-owned listeners, controller credentials and TUN routing are applied by the router.'));
  const upload=input(imported,'proxy-file',tr('Configuration file'),{type:'file'});upload.accept='.yaml,.yml,.json';
  const config=input(imported,'proxy-config',tr('YAML / JSON'),{multiline:true,required:true,help:tr('Up to 512 KiB. May contain credentials. Drafts are cleared when you leave this page or lock the console.')});config.maxLength=512*1024;config.rows=16;
  let fileEpoch=0;
  upload.addEventListener('change',async()=>{const epoch=++fileEpoch,file=upload.files[0];if(!file)return;if(file.size>512*1024){UI.say(proxy.feedback,tr('Configuration exceeds 512 KiB.'),true);upload.value='';return;}try{const value=await file.text();if(epoch===fileEpoch&&active()&&!API.locked){config.value=value;imported.dataset.dirty='true';}}catch{if(epoch===fileEpoch)UI.say(proxy.feedback,tr('Could not read this file.'),true);}});
  const read=button(tr('Load saved configuration'),async()=>{
    const epoch=++fileEpoch;
    if(!await UI.confirm(tr('Load saved configuration'),tr('Load the saved configuration into this editor. It may contain credentials and will replace the current draft.'),{label:tr('Load configuration')}))return;
    if(epoch!==fileEpoch||!active()||API.locked)return;
    try{const result=await UI.request('/api/proxy/mihomo/config');if(epoch!==fileEpoch||!active()||API.locked)return;config.value=result.config;imported.dataset.dirty='true';}catch(error){if(epoch===fileEpoch&&!API.locked)UI.say(proxy.feedback,error.message,true);}
  });proxy.guard(read,d=>d.configured);imported.append(read);
  proxy.guard(submit(imported,tr('Save configuration'),()=>{if(new TextEncoder().encode(config.value).length>512*1024){UI.say(proxy.feedback,tr('Configuration exceeds 512 KiB.'),true);return;}proxy.write('config',{config:config.value},tr('Validate with mihomo, then replace only this service’s saved configuration. The service stays stopped.'),{node:imported});}),d=>d.installed&&!d.running);
  const subscription=form(configuration,'proxy-subscription',tr('Subscription / configuration URL'),tr('Download once over HTTPS. The URL is cleared after import; provider URLs inside the configuration remain in the private configuration.'));
  const subURL=input(subscription,'proxy-source-url',tr('HTTPS URL'),{type:'password',required:true});
  proxy.guard(submit(subscription,tr('Fetch configuration'),()=>proxy.write('subscription',{url:subURL.value},tr('Fetch and validate a mihomo configuration for this service.'),{node:subscription})),d=>d.installed&&!d.running);
  configuration.append(el('p','panel-note',tr('Managed fields: runtime mode, mixed port, IPv6, concurrent TCP, listener addresses, controller authentication, provider cache paths, TUN and local-domain DNS routing. Other supported native options are retained.')));

  groupPane.append(el('h3','form-section-title',tr('Proxy groups')),el('p','panel-note',tr('Manual groups let you select a node. URL-test, fallback and load-balance groups follow their own configuration.')));
  const groups=el('div','service-groups'),groupDrafts=new Map();groupPane.append(groups);
  proxy.onSync=()=>{for(const node of groups.querySelectorAll('button,select'))node.disabled=node.dataset.selectable!=='true'||!proxy.data?.running||API.locked||!API.connected||UI.saving;};
  proxy.afterRead=async(d,current)=>{
    if(!d.running){groupDrafts.clear();groups.replaceChildren(el('p','panel-note',tr('Start mihomo to read its proxy groups.')));return;}
    try{const result=await UI.request('/api/proxy/mihomo/groups',{timeoutMs:20000});if(!current())return;for(const name of groupDrafts.keys())if(!result.some(g=>g.name===name))groupDrafts.delete(name);groups.replaceChildren();
      for(const g of result){const row=el('div','service-group'),label=el('label','field'),name=el('span','field-label',g.name),select=el('select');
        for(const c of g.choices||[]){const option=el('option','',c);option.value=c;select.append(option);}const draft=groupDrafts.get(g.name);if(draft!==undefined&&!(g.choices||[]).includes(draft))groupDrafts.delete(g.name);select.value=groupDrafts.get(g.name)||g.selected||'';select.addEventListener('change',()=>groupDrafts.set(g.name,select.value));select.setAttribute('aria-label',g.name);select.disabled=!g.selectable;label.append(name,select);
        const apply=button(tr('Use node'),()=>proxy.write('select',{group:g.name,choice:select.value},tr("Change the selected node for {0}.", g.name),{after:()=>groupDrafts.delete(g.name)}));apply.disabled=!g.selectable||!API.connected||UI.saving;
        select.dataset.selectable=apply.dataset.selectable=String(!!g.selectable);
        row.append(label,el('span','reading-caption',g.selectable?tr('Select · manual'):tr("{0} · managed by configuration", g.kind||tr('Automatic'))),apply);groups.append(row);
      }if(!result.length)groups.append(el('p','panel-note',tr('No proxy groups in this configuration.')));
    }catch{if(current())groups.replaceChildren(el('p','panel-note',tr('Proxy groups unavailable. Refresh after startup.')));}
  };
  const installer=form(installation,'proxy-install',tr('Install / update mihomo'),tr('Static Linux ARM64 executable, optionally gzip compressed. Verify the SHA-256 from the release publisher.'));
  const binaryURL=input(installer,'proxy-binary-url',tr('Download URL'),{type:'url',required:true}),version=input(installer,'proxy-version',tr('Version'),{required:true}),hash=input(installer,'proxy-sha256',tr('Download SHA-256'),{required:true,help:tr('Checksum of the downloaded file, before decompression.')});hash.pattern='[a-fA-F0-9]{64}';version.pattern='[a-zA-Z0-9._-]{1,32}';
  proxy.guard(submit(installer,tr('Install mihomo'),()=>proxy.write('install',{url:binaryURL.value,version:version.value,sha256:hash.value},tr('Validate and install this mihomo executable. The saved configuration is retained.'),{node:installer,label:tr('Install mihomo'),after:loadDirectory})),d=>!d.running);
  proxy.onDraw=d=>{
    proxy.heading(d.running?(d.capturing?tr('Running · hotspot'):tr('Running · local proxy')):d.enabled?tr('Stopped · attention required'):tr('Stopped'),d.running);
    proxy.fact(tr('Version'),d.installed?(d.version||tr('Installed')):tr('Not installed'));proxy.fact(tr('Configuration'),d.configured?tr('Saved'):tr('Not imported'));proxy.fact(tr('Mode'),({rule:tr('Rule'),global:tr('Global'),direct:tr('Direct')})[d.mode]||d.mode);proxy.fact(tr('Local HTTP / SOCKS'),d.local_endpoint);if(d.last_error)proxy.fact(tr('Last error'),d.last_error);
    if(!pf.dataset.dirty){pm.value=d.mode;pc.value=d.capture;port.value=d.mixed_port||7890;ipv6.value=String(d.ipv6!==false);concurrent.value=String(!!d.tcp_concurrent);}
  };
  proxy.onClear=()=>{fileEpoch++;config.value='';subURL.value='';upload.value='';groups.replaceChildren();groupDrafts.clear();show('overview');};proxy.track();
})();
