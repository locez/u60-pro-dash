'use strict';
(() => {
  const UI=OpenU60UI, API=OpenU60Console, {el,button}=UI;
  const active=route=>OpenU60Navigation.current.key===route;
  function input(form,id,label,{type='text',values,help='',multiline=false,value='',required=false}={}) {
    const wrap=el('label','field'), name=el('span','field-label',label);
    const node=el(values?'select':multiline?'textarea':'input');node.id=id;node.name=id;node.autocomplete='off';
    if(values)for(const [key,text]of values){const option=el('option','',text);option.value=key;node.append(option);}
    else if(!multiline)node.type=type;
    if(multiline){node.rows=7;node.spellcheck=false;}
    node.value=value;node.required=required;wrap.append(name,node);if(help)wrap.append(el('small','field-help',help));form.append(wrap);return node;
  }
  function form(section,id,title,description) {
    const node=el('form','settings-form service-form');node.id=id;node.autocomplete='off';
    node.append(el('h3','form-section-title',title));if(description)node.append(el('p','panel-note',description));
    node.addEventListener('input',()=>node.dataset.dirty='true');node.addEventListener('change',()=>node.dataset.dirty='true');
    section.append(node);return node;
  }
  function submit(node,label,run) {
    const actions=el('div','form-actions'), apply=el('button','button primary',label);apply.type='submit';actions.append(apply);node.append(actions);
    node.addEventListener('submit',event=>{event.preventDefault();if(node.reportValidity())run();});return apply;
  }
  function service(key,title,description,{endpoint=key,route=`network/${key}`}={}) {
    const panel=UI.panel(route,`${key}-service`,title,description);
    const hero=el('div','service-summary'), badge=el('span','service-indicator'), state=el('strong','', 'Waiting for status'), caption=el('p','reading-caption','Connect to read service status.');
    badge.setAttribute('aria-hidden','true');hero.append(badge,state);panel.append(hero,caption);
    const facts=el('dl','settings-facts'), actions=el('div','form-actions diagnostic-actions'), content=el('div');panel.append(facts,actions,content);
    const feedback=UI.status(panel), controls=[];
    let snapshot=null, generation=0, loading=false, loaded=false;
    const view={route,panel,content,feedback,actions,get data(){return snapshot;},onDraw:()=>{},onClear:()=>{}};
    view.fact=(label,value)=>{const row=el('div');row.append(el('dt','',label),el('dd','',value===undefined||value===null||value===''?'Not reported':String(value)));facts.append(row);};
    view.guard=(node,predicate=()=>true)=>{controls.push([node,predicate]);node.disabled=true;return node;};
    view.sync=()=>{for(const[node,predicate]of controls)node.disabled=!snapshot||API.locked||!API.connected||UI.saving||!predicate(snapshot);view.onSync?.();};
    view.draw=data=>{snapshot=data;loaded=true;facts.replaceChildren();view.onDraw(data);view.sync();};
    view.heading=(text,running)=>{state.textContent=text;badge.dataset.running=String(running);};
    view.clear=()=>{generation++;loading=false;loaded=false;snapshot=null;facts.replaceChildren();state.textContent='Waiting for status';badge.dataset.running='false';caption.textContent='Connect to read service status.';UI.say(feedback,'');for(const f of panel.querySelectorAll('form')){f.reset();delete f.dataset.dirty;}view.onClear();view.sync();};
    view.load=async()=>{
      if(loading||!active(route)||API.locked||!API.connected)return;
      const epoch=generation;loading=true;
      try{const data=await UI.request(`/api/${endpoint}/status`,{timeoutMs:20000});if(epoch!==generation||API.locked||!active(route))return;view.draw(data);caption.textContent='Read from the router just now.';caption.dataset.stale='false';await view.afterRead?.(data,()=>epoch===generation&&!API.locked&&active(route));}
      catch(error){if(epoch===generation&&!API.locked){caption.textContent=`Status unavailable. ${error.message}`;caption.dataset.stale='true';}}
      finally{if(epoch===generation)loading=false;}
    };
    view.write=async(action,body,explanation,{method='POST',node,after,label='Apply change',destructive=false}={})=>{
      const epoch=generation;if(!snapshot||API.locked||!API.connected||!active(route))return;
      if(!await UI.confirm(title,explanation,{label,destructive}))return;
      if(epoch!==generation||API.locked||!API.connected||!active(route))return;
      await UI.mutate(feedback,async live=>{
        const result=await UI.request(`/api/${endpoint}/${action}`,{...UI.json(method,body),timeoutMs:150000});
        if(!live()||epoch!==generation||!active(route))return;
        if(node){node.reset();delete node.dataset.dirty;}
        after?.(result);UI.say(feedback,'Request accepted. Reading current state…');
        // A prior status read must not overwrite the result of this mutation.
        generation++;loading=false;await view.load();
        if(live()&&active(route))UI.say(feedback,'Current state refreshed.');
      });view.sync();
    };
    actions.append(button('Refresh status',()=>view.load()));
    window.addEventListener('dashboard:navigate',()=>{if(!active(route))view.clear();});
    window.addEventListener('dashboard:connection',()=>{view.sync();if(!API.connected){caption.textContent='Connection interrupted. Refresh after reconnecting.';caption.dataset.stale='true';}});
    window.addEventListener('dashboard:mutation-complete',view.sync);
    view.track=()=>UI.track({route,load:view.load,refresh:view.load,clear:view.clear,get loaded(){return loaded;},get dirty(){return !!panel.querySelector('form[data-dirty=true]');}});
    // A single in-flight read, only while this authenticated page is visible.
    setInterval(()=>{if(!document.hidden&&document.hasFocus()&&!UI.saving&&active(route))view.load();},10000);
    return view;
  }

  globalThis.OpenU60Services={input,form,submit,service};

  const ts=service('tailscale','Tailscale','Join your private network and manage remote access and subnet routes.');
  const tsReady=d=>d.installed&&!d.updating;
  for(const[action,label,explanation,predicate]of[
    ['enable','Start Tailscale','Start Tailscale using the saved network identity.',d=>tsReady(d)&&!d.enabled],
    ['disable','Stop Tailscale','Disconnect this router from Tailscale. Remote administration and subnet access through it will stop.',d=>tsReady(d)&&d.enabled],
    ['logout','Sign out of Tailscale','Remove this router’s network identity. Remote access stops and another authorization is required to reconnect.',d=>tsReady(d)&&d.enrolled]
  ])ts.actions.append(ts.guard(button(label,()=>ts.write(action,{},explanation,{label,destructive:action==='logout'}),action==='enable'?'primary':''),predicate));
  const authorize=el('div','form-actions diagnostic-actions'),loginLink=el('a','button primary','Open authorization page');loginLink.target='_blank';loginLink.rel='noopener noreferrer';loginLink.hidden=true;
  function authURL(value){loginLink.hidden=true;loginLink.removeAttribute('href');try{const u=new URL(value);if(u.protocol==='https:'&&!u.username&&!u.password&&(u.hostname==='login.tailscale.com'||u.hostname.endsWith('.tailscale.com'))){loginLink.href=u.href;loginLink.hidden=false;}}catch{}}
  authorize.append(ts.guard(button('Authorize this router',()=>ts.write('login-url',{},'Request an authorization link, then sign in to Tailscale in the new tab.',{after:r=>authURL(r.login_url),label:'Get authorization link'})),d=>tsReady(d)&&d.backend_state!=='Running'),loginLink);ts.content.append(authorize);
  const tf=form(ts.content,'tailscale-policy','Network settings','Subnet routes you advertise must also be approved in the Tailscale admin console.');
  const hostname=input(tf,'tailscale-hostname','Device name',{required:true}),routes=input(tf,'tailscale-routes','Advertised IPv4 routes',{help:'Comma-separated CIDRs. Leave empty to advertise none.'}),accept=input(tf,'tailscale-accept','Accept remote subnet routes',{values:[['true','On'],['false','Off']],help:'Applies while connected in kernel gateway mode. Remote routes still need approval and access rules.'}),ssh=input(tf,'tailscale-ssh','Tailscale SSH',{values:[['false','Off'],['true','On']]}),wake=input(tf,'tailscale-wake','Keep network awake',{values:[['true','On'],['false','Off']]});hostname.pattern='[a-zA-Z0-9-]{1,63}';
  ts.guard(submit(tf,'Save network settings',()=>ts.write('config',{hostname:hostname.value,advertise_routes:routes.value.split(',').map(s=>s.trim()).filter(Boolean),accept_routes:accept.value==='true',tailscale_ssh:ssh.value==='true',hold_wakelock:wake.value==='true'},'Apply these Tailscale preferences. Changing subnet routes or SSH can affect remote access.',{method:'PUT',node:tf})),d=>!d.updating);
  const keyForm=form(ts.content,'tailscale-enroll','Enroll with an auth key','An alternative to browser authorization. The key is submitted once and is cleared from this form afterward.');
  const key=input(keyForm,'tailscale-auth-key','Tailscale auth key',{type:'password',required:true});
  ts.guard(submit(keyForm,'Enroll router',()=>ts.write('setup',{auth_key:key.value},'Join the network associated with this auth key.',{node:keyForm,label:'Enroll router'})),d=>tsReady(d)&&d.backend_state!=='Running');
  const update=form(ts.content,'tailscale-update','Install or update Tailscale','Download the official ARM64 package after verifying its SHA-256. Updating can interrupt remote access.');
  const tv=input(update,'tailscale-version','Version',{value:'1.104.4',required:true}),th=input(update,'tailscale-sha256','Package SHA-256',{required:true});tv.defaultValue='1.104.4';th.pattern='[a-fA-F0-9]{64}';tv.pattern='[0-9]+\\.[0-9]+\\.[0-9]+';
  ts.guard(submit(update,'Install version',()=>ts.write('update',{version:tv.value,sha256:th.value},'Install this Tailscale version. Check the service status here until the update completes.',{node:update,label:'Install version'})),d=>!d.updating);
  ts.onDraw=d=>{
    ts.fact('Remote web port',d.management_port);
    ts.heading(d.updating?'Updating Tailscale':d.backend_state==='Running'?'Connected':d.enabled?(d.backend_state||'Starting'):'Stopped',d.backend_state==='Running'&&!d.updating);
    ts.fact('Installed version',d.client_version||d.pinned_version||(d.installed?'Installed':'Not installed'));ts.fact('Addresses',(d.tailscale_ips||[]).join(' · '));ts.fact('Network name',d.dns_name);ts.fact('Routing mode',d.gateway_mode?'Kernel gateway':'Device access only');ts.fact('Remote route preference',d.accept_routes_effective?'Enabled while running':'Not active');ts.fact('Health',(d.health||[]).join(' · ')||'No reported warning');ts.fact('Latest update',typeof d.last_update==='string'?d.last_update:(d.last_update?JSON.stringify(d.last_update):'No update recorded'));
    if(!tf.dataset.dirty){hostname.value=d.hostname||'';routes.value=(d.advertise_routes||[]).join(', ');accept.value=String(d.accept_routes!==false);ssh.value=String(!!d.tailscale_ssh);wake.value=String(!!d.hold_wakelock);}
    authorize.hidden=d.backend_state==='Running';keyForm.hidden=d.backend_state==='Running';
    if(d.login_url)authURL(d.login_url);if(d.backend_state==='Running')authURL(null);
  };
  ts.onClear=()=>{key.value='';authURL(null);};ts.track();
})();
