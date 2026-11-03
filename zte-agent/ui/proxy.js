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
  const panes=new Map(),tabs=[];let selectedPane='overview',readConfiguration=async()=>{};
  function pane(id,label){
    const node=el('div','proxy-pane');node.id=`proxy-pane-${id}`;node.setAttribute('role','tabpanel');
    const tab=button(label,()=>show(id));tab.id=`proxy-tab-${id}`;tab.setAttribute('role','tab');tab.setAttribute('aria-controls',node.id);node.setAttribute('aria-labelledby',tab.id);node.tabIndex=0;tabs.push([id,tab]);nav.append(tab);proxy.content.append(node);panes.set(id,node);return node;
  }
  function show(id){selectedPane=id;if(id==='configuration')readConfiguration(false);for(const[key,node]of panes)node.hidden=key!==id;for(const[key,tab]of tabs){tab.setAttribute('aria-selected',String(key===id));tab.tabIndex=key===id?0:-1;}}
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

  const startHint=el('div','proxy-start-hint');proxy.actions.after(startHint);
  const imported=form(configuration,'proxy-import',tr('Current configuration'),tr('Read and edit the saved YAML. Changes are validated before saving; a running service applies them with reload.'));
  const editorActions=el('div','form-actions proxy-config-actions');imported.append(editorActions);
  const editorFeedback=UI.status(imported);editorFeedback.id='proxy-config-feedback';
  const read=button(tr('Load saved configuration'),()=>readConfiguration(true));
  const validate=button(tr('Validate configuration'),()=>applyConfiguration('validate'));
  const save=button(tr('Save configuration'),()=>applyConfiguration('config'),'primary');save.id='proxy-save-config';
  const reload=button(tr('Reload saved configuration'),()=>applyConfiguration('reload'));reload.id='proxy-reload-config';
  editorActions.append(read,validate,save,reload);
  const importOptions=el('details','proxy-import-options'),importTitle=el('summary','',tr('Import a file or subscription'));
  const importFields=el('div','proxy-import-fields');importOptions.append(importTitle,importFields);imported.append(importOptions);
  const upload=input(importFields,'proxy-file',tr('Configuration file'),{type:'file'});upload.accept='.yaml,.yml,.json';
  const applyFile=button(tr('Import and apply file'),()=>applyConfiguration('config'));applyFile.id='proxy-import-file';importFields.append(applyFile);
  const subURL=input(importFields,'proxy-source-url',tr('Subscription / configuration URL'),{type:'url',help:tr('HTTPS URL returning native YAML or JSON, including direct configuration links. The address is cleared after a confirmed import.')});subURL.inputMode='url';subURL.spellcheck=false;subURL.autocapitalize='none';
  const fetchSubscription=button(tr('Import and apply subscription'),()=>applyConfiguration('subscription'));fetchSubscription.id='proxy-import-subscription';importFields.append(fetchSubscription);
  const config=input(imported,'proxy-config',tr('YAML / JSON'),{multiline:true,help:tr('Up to 512 KiB. Edits remain a draft until saved. Leaving this page or locking the console clears the editor.')});config.maxLength=512*1024;config.rows=12;
  imported.addEventListener('submit',event=>{event.preventDefault();});
  let fileEpoch=0,configRead=0,fileRead=0,editSequence=0,editorLoading=false,fileLoading=false,editorLoaded=false,editorDirty=false,editorRevision=null;
  const editorCurrent=epoch=>epoch===fileEpoch&&active()&&!API.locked;
  function syncEditor(){
    const ready=active()&&!API.locked&&API.connected&&!UI.saving, installed=!!proxy.data?.installed;
    read.disabled=!ready||editorLoading||fileLoading;
    validate.disabled=!ready||!installed||!editorLoaded||editorLoading||fileLoading||!config.value.trim();
    save.disabled=validate.disabled||!editorDirty;
    applyFile.disabled=save.disabled||!upload.files.length;
    reload.disabled=!ready||!installed||!proxy.data?.running||!proxy.data?.configured||!editorLoaded||editorLoading||fileLoading;
    fetchSubscription.disabled=!ready||!installed||!editorLoaded||editorLoading||fileLoading||!subURL.value.trim();
    save.textContent=proxy.data?.running?tr('Save and reload'):tr('Save configuration');
    imported.dataset.dirty=String(editorDirty||!!subURL.value||!!upload.files.length);
  }
  function savedConfiguration(value){
    if(!value||typeof value.config!=='string'||typeof value.configured!=='boolean'||(value.configured&&(!value.config.trim()||!/^[a-f0-9]{64}$/.test(value.revision||'')))||(!value.configured&&value.revision!==null))throw new Error(tr('The router did not return a verified saved configuration.'));
    return value;
  }
  function populateConfiguration(value){config.value=value.config;editorRevision=value.revision;editorLoaded=true;editorDirty=false;upload.value='';}
  readConfiguration=async force=>{
    if(!active()||API.locked||!API.connected||editorLoading||UI.saving)return;
    const epoch=fileEpoch;
    if(force&&editorDirty&&!await UI.confirm(tr('Discard unsaved edits?'),tr('Read the current router settings and replace this draft.'),{label:tr('Discard and read')}))return;
    if(!editorCurrent(epoch))return;
    const readId=++configRead,draftAtStart=editSequence;editorLoading=true;syncEditor();
    try{
      const value=savedConfiguration(await UI.request('/api/proxy/mihomo/config'));
      if(!editorCurrent(epoch)||readId!==configRead)return;
      if(!editorDirty||(force&&draftAtStart===editSequence))populateConfiguration(value);
      else if(!editorLoaded){editorLoaded=true;editorRevision=value.revision;}
      importOptions.open=!value.configured;
      UI.say(editorFeedback,editorDirty?tr('Unsaved draft. Save to apply these changes.'):value.configured?tr('Current saved configuration loaded.'):tr('No saved configuration. Import a subscription or save a configuration to enable startup.'));
    }catch(error){if(editorCurrent(epoch)&&readId===configRead)UI.say(editorFeedback,error.message,true);}
    finally{if(editorCurrent(epoch)){editorLoading=false;syncEditor();}}
  };
  config.addEventListener('input',()=>{editSequence++;fileRead++;fileLoading=false;editorDirty=true;upload.value='';UI.say(editorFeedback,tr('Unsaved draft. Save to apply these changes.'));syncEditor();});
  subURL.addEventListener('input',syncEditor);
  upload.addEventListener('change',async()=>{
    const epoch=fileEpoch,readId=++fileRead,file=upload.files[0];if(!file)return;
    if(file.size>512*1024){UI.say(editorFeedback,tr('Configuration exceeds 512 KiB.'),true);upload.value='';return;}
    editSequence++;editorDirty=true;fileLoading=true;syncEditor();
    try{const value=await file.text();if(editorCurrent(epoch)&&readId===fileRead){config.value=value;UI.say(editorFeedback,tr('File loaded as a draft. Save to apply it.'));}}
    catch{if(editorCurrent(epoch))UI.say(editorFeedback,tr('Could not read this file.'),true);}
    finally{if(editorCurrent(epoch)&&readId===fileRead){fileLoading=false;syncEditor();}}
  });
  async function applyConfiguration(action){
    if(!editorLoaded||!active()||API.locked||!API.connected||UI.saving)return;
    const epoch=fileEpoch,body={revision:editorRevision};
    if(action==='subscription')body.url=subURL.value.trim();
    else if(action!=='reload')body.config=config.value;
    if(body.config!==undefined&&new TextEncoder().encode(body.config).length>512*1024){UI.say(editorFeedback,tr('Configuration exceeds 512 KiB.'),true);return;}
    if(action!=='validate'){
      const message=action==='reload'?tr('Reload the saved configuration without replacing the draft in this editor.'):action==='subscription'?tr('Fetch, validate and save this subscription. A running service will reload it.'):tr('Validate and save this draft. A running service will reload it; a stopped service stays stopped.');
      if(!await UI.confirm('mihomo',message,{label:action==='reload'?tr('Reload'):tr('Apply configuration')}))return;
    }
    if(!editorCurrent(epoch))return;
    configRead++;
    await UI.mutate(editorFeedback,async live=>{
      try{
        const result=await UI.request(`/api/proxy/mihomo/${action}`,{...UI.json('POST',body),timeoutMs:240000});
        if(!live()||!editorCurrent(epoch))return;
        if(action==='validate'){
          if(result.valid!==true)throw new Error(tr('The router did not confirm configuration validation.'));
          UI.say(editorFeedback,tr('Configuration is valid. No changes have been applied.'));return;
        }
        const saved=savedConfiguration(await UI.request('/api/proxy/mihomo/config'));
        if(!live()||!editorCurrent(epoch))return;
        if(result.configured!==true||!saved.configured||saved.revision!==result.config_revision)throw new Error(tr('The saved configuration could not be confirmed. Your draft is retained.'));
        if(action!=='reload'||!editorDirty)populateConfiguration(saved);
        if(action==='subscription')subURL.value='';
        proxy.draw(result);await proxy.reload();await loadDirectory();
        if(live()&&editorCurrent(epoch))UI.say(editorFeedback,action==='reload'&&editorDirty?tr('Saved configuration reloaded. Your editor draft is still unsaved.'):result.running?tr('Configuration saved and reloaded.'):tr('Configuration saved. You can now start the service.'));
      }catch(error){if(live()&&editorCurrent(epoch))UI.say(editorFeedback,error.status?error.message:tr('Could not confirm the result. The draft is retained; read the current configuration before retrying.')+' '+error.message,true);}
    });syncEditor();
  }
  configuration.append(el('p','panel-note',tr('Managed fields: runtime mode, mixed port, IPv6, concurrent TCP, listener addresses, controller authentication, provider cache paths, TUN and local-domain DNS routing. Other supported native options are retained.')));

  groupPane.append(el('h3','form-section-title',tr('Proxy groups')),el('p','panel-note',tr('Manual groups let you select a node. URL-test, fallback and load-balance groups follow their own configuration.')));
  const groups=el('div','service-groups'),groupDrafts=new Map();groupPane.append(groups);
  proxy.onSync=()=>{syncEditor();for(const node of groups.querySelectorAll('button,select'))node.disabled=node.dataset.selectable!=='true'||!proxy.data?.running||API.locked||!API.connected||UI.saving;};
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
  const installer=OpenU60Installer(proxy,installation,{service:'mihomo',title:'mihomo',onInstalled:loadDirectory});
  proxy.onDraw=d=>{
    installer.drawInstalled(d);
    startHint.replaceChildren();
    if(!d.installed||!d.configured){startHint.append(el('span','',d.installed?tr('Import and save a configuration before starting this service.'):tr('Install this service before importing its configuration.')),button(d.installed?tr('Open configuration'):tr('Installation'),()=>show(d.installed?'configuration':'installation')));}
    if(selectedPane==='configuration'&&!editorLoading&&(!editorLoaded||(!editorDirty&&d.config_revision!==undefined&&d.config_revision!==editorRevision)))readConfiguration(false);
    proxy.heading(d.running?(d.capturing?tr('Running · hotspot'):tr('Running · local proxy')):d.enabled?tr('Stopped · attention required'):tr('Stopped'),d.running);
    proxy.fact(tr('Version'),d.installed?(d.version||tr('Installed')):tr('Not installed'));proxy.fact(tr('Configuration'),d.configured?tr('Saved'):tr('Not imported'));proxy.fact(tr('Mode'),({rule:tr('Rule'),global:tr('Global'),direct:tr('Direct')})[d.mode]||d.mode);proxy.fact(tr('Local HTTP / SOCKS'),d.local_endpoint);if(d.last_error)proxy.fact(tr('Last error'),d.last_error);
    if(!pf.dataset.dirty){pm.value=d.mode;pc.value=d.capture;port.value=d.mixed_port||7890;ipv6.value=String(d.ipv6!==false);concurrent.value=String(!!d.tcp_concurrent);}
  };
  proxy.onClear=()=>{installer.clear();fileEpoch++;configRead++;fileRead++;editorLoading=false;fileLoading=false;editorLoaded=false;editorDirty=false;editorRevision=null;config.value='';subURL.value='';upload.value='';importOptions.open=false;UI.say(editorFeedback,'');startHint.replaceChildren();groups.replaceChildren();groupDrafts.clear();show('overview');};proxy.track();
})();
