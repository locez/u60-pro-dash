'use strict';
(() => {
  const tr = globalThis.OpenU60I18n?.t || ((s,...v)=>s.replace(/\{(\d+)\}/g,(match,i)=>Number(i)<v.length?String(v[i]):match));
  const UI = OpenU60UI, API = OpenU60Console, R = OpenU60Routing;
  const {el,button,field,onoff,choices,say,request,json,confirm,mutate} = UI;
  const lanKeys = ['lan_ipaddr','lan_netmask','dhcp_enable','dhcp_start','dhcp_end','dhcp_lease_time'];
  const samples = new Map();
  const enabled = value => ['1','true','on'].includes(String(value).toLowerCase());
  const state = value => value === undefined || value === null || value === '' ? tr('Not reported') : enabled(value) ? tr('Enabled') : ['0','false','off'].includes(String(value).toLowerCase()) ? tr('Disabled') : String(value);
  const readLAN = async()=>R.normalizeLAN(await request('/api/router/lan'));
  function setting(endpoint,keys) {
    return async()=>{
      const data = await request(endpoint);
      if (!data || !keys.every(key=>Object.hasOwn(data,key))) throw new Error(tr('The router returned incomplete settings'));
      return R.normalizeSwitches(data);
    };
  }
  function summary(route,id,title,description,definitions) {
    const section = UI.panel(route,id,title,description), list = el('dl','routing-stats'), nodes = {};
    for (const [key,label,numeric] of definitions) {
      const pair = el('div'), value = el('dd',numeric ? 'mono' : '', '—');
      pair.append(el('dt','',label),value); list.append(pair); nodes[key] = {value,numeric};
    }
    const caption = el('p','reading-caption',tr('Waiting for live router settings.')); section.append(list,caption);
    return {section,nodes,caption};
  }
  function values(summary,data) { for (const [key,{value,numeric}] of Object.entries(summary.nodes)) {
    const text = data[key] ?? '—'; if (numeric) OpenU60Numbers.set(value,text); else value.textContent=text;
  } }
  function stamp(summary,names) {
    const available = names.map(name=>samples.get(name));
    const received = available.filter(Boolean).map(sample=>sample.at);
    const unavailable = available.some(sample=>!sample || sample.unavailable);
    summary.caption.textContent = received.length ? `${API.connected && !unavailable ? tr('Live settings') : tr('Last received settings')} · ${new Date(Math.min(...received)).toLocaleTimeString(globalThis.OpenU60I18n?.locale)}${unavailable ? tr(' · some readings unavailable') : ''}` : tr('Waiting for live router settings.');
    summary.caption.dataset.stale = String(!API.connected || unavailable);
  }
  const lanSummary = summary('network/lan','lan-overview',tr('Your local network'),tr('Local addresses stay separate from your mobile internet connection.'),[['address',tr('Router address')],['dhcp',tr('DHCP server')],['pool',tr('Addresses in pool'),true],['leases',tr('Reported leases'),true]]);
  const lanForm = UI.makeForm({route:'network/lan',id:'lan-config',title:tr('LAN address & DHCP'),description:tr('Choose the local gateway and address pool. Pool addresses must belong to the same subnet and exclude the router.'),endpoint:'/api/router/lan',read:readLAN,source:'lan_config',liveData:data=>{try{return R.normalizeLAN(data);}catch{return null;}},guardKeys:lanKeys,
    fields:[field('lan_ipaddr',tr('Router IPv4 address'),{required:true,placeholder:'192.168.0.1'}),field('lan_netmask',tr('Subnet mask'),{required:true,placeholder:'255.255.255.0'}),onoff('dhcp_enable',tr('DHCP server')),field('dhcp_start',tr('First pool address'),{help:tr('A full IPv4 address, for example 192.168.0.100.')}),field('dhcp_end',tr('Last pool address')),field('dhcp_lease_time',tr('Lease duration'),{help:tr('Seconds or a duration such as 12h or 1d. Existing units are preserved.')})],
    validate:(_,all)=>R.lanBody(all),body:(_,all)=>R.lanBody(all),
    warning:(_,all,baseline)=>tr("Changing LAN or DHCP settings can disconnect devices. {0}{1}Reconnect to the local network after saving.", all.lan_ipaddr!==baseline.lan_ipaddr ? tr("The console’s new local address will be {0}. ", new URL(R.reconnectURL(location.origin,all.lan_ipaddr)).host) : '', all.dhcp_enable==='0' ? tr('With DHCP off, devices need a manually assigned address or another DHCP server. ') : ''),
    onWriting:body=>{ reconnect.href=R.reconnectURL(location.origin,body.lan_ipaddr); reconnect.textContent=tr("Open console at {0}", new URL(reconnect.href).host); reconnectNote.hidden=false; },
    verify:(data,body)=>lanKeys.every(key=>key==='dhcp_lease_time' ? R.lease(data[key])===R.lease(body[key]) : String(data[key])===String(body[key]))
  });
  const reconnectNote = el('div','routing-reconnect'); reconnectNote.hidden=true;
  const reconnect = el('a','button'); reconnectNote.append(el('p','',tr('If the console disconnects, reconnect to the router’s network and open this address. A lost response does not confirm the change.')),reconnect); lanForm.section.append(reconnectNote);
  const leases = UI.panel('network/lan','lan-leases',tr('DHCP leases'),tr('Reported leases may include devices that recently disconnected. Use a stable device address for DMZ and port-forward destinations.'));
  const leaseWrap = el('div','table-scroll diagnostic-table'), leaseTable = el('table'), leaseHead = el('thead'), leaseHeader = el('tr'), leaseBody = el('tbody'); leaseTable.setAttribute('aria-label',tr('DHCP leases'));
  for (const text of ['DEVICE','ADDRESS',tr('MAC ADDRESS')]) { const th=el('th','',text); th.scope='col'; leaseHeader.append(th); }
  leaseHead.append(leaseHeader); leaseTable.append(leaseHead,leaseBody); leaseWrap.append(leaseTable); leases.append(leaseWrap);
  function drawLeases() {
    const data = samples.get('clients')?.data;
    const entries = Array.isArray(data?.dhcp_leases) ? data.dhcp_leases : null;
    if (!entries?.length) { const row=el('tr'), cell=el('td','empty',entries ? tr('No DHCP leases reported.') : tr('Lease readings unavailable.')); cell.colSpan=3; row.append(cell); leaseBody.replaceChildren(row); return; }
    const existing = new Map([...leaseBody.children].map(row=>[row.dataset.key,row])), ordered=[];
    for (const [index,entry] of entries.entries()) {
      const key=JSON.stringify([entry.macaddr,entry.ipaddr,index]); let row=existing.get(key);
      if (!row) {row=el('tr');row.dataset.key=key;for(const label of [tr('Device'),tr('Address'),tr('MAC address')]){const cell=el('td',label===tr('Device') ? '' : 'mono');cell.dataset.label=label;row.append(cell);}}
      [entry.hostname,entry.ipaddr,entry.macaddr].forEach((text,index)=>{row.children[index].textContent=text || '—';}); ordered.push(row);
    }
    const wanted=new Set(ordered); for(const row of [...leaseBody.children])if(!wanted.has(row))row.remove();
    ordered.forEach((row,index)=>{if(leaseBody.children[index]!==row)leaseBody.insertBefore(row,leaseBody.children[index] || null);});
  }
  const firewallSummary = summary('network/firewall','firewall-overview',tr('Inbound access'),tr('Review which services can accept connections from outside your local network.'),[['firewall',tr('Firewall')],['nat','NAT'],['upnp',tr('UPnP')],['dmz',tr('DMZ host')]]);
  function simple({id,title,description,key,endpoint='/api/router/firewall',writeEndpoint,source='firewall_config',warning,values}) {
    return UI.makeForm({route:'network/firewall',id,title,description,endpoint,writeEndpoint,source,read:setting(endpoint,[key]),liveData:R.normalizeSwitches,guardKeys:[key],fields:[values ? choices(key,title,values) : onoff(key,title)],validate:(_,all)=>values ? undefined : R.switchValue(all[key],title),warning});
  }
  simple({id:'firewall-switch',title:tr('Firewall protection'),description:tr('Enable or disable the router’s inbound firewall.'),key:'firewall_switch',writeEndpoint:'/api/router/firewall/switch',warning:tr('Changing firewall protection can affect incoming connections. Disabling it removes the router’s inbound filtering. Apply this change?')});
  simple({id:'firewall-level',title:tr('Protection level'),description:tr('Use the filtering level provided by your router’s firmware.'),key:'firewall_level',writeEndpoint:'/api/router/firewall/level',values:[['low',tr('Low')],['medium',tr('Medium')],['high',tr('High')]],warning:tr('A different firewall level may interrupt existing connections or block services. Apply this level?')});
  simple({id:'nat-config',title:tr('Network address translation'),description:tr('NAT allows local devices to share the router’s internet address.'),key:'nat_switch',writeEndpoint:'/api/router/firewall/nat',warning:tr('Changing NAT may interrupt internet access for local devices. Keep a local connection to the router.')});
  simple({id:'upnp-config',title:tr('UPnP'),description:tr('Let applications on local devices request incoming port mappings automatically.'),key:'upnp_switch',endpoint:'/api/router/firewall/upnp',source:'upnp_config',warning:tr('UPnP lets applications request inbound access automatically. Apply this change for devices on your local network?')});
  UI.makeForm({route:'network/firewall',id:'dmz-config',title:tr('DMZ host'),description:tr('Send otherwise unmatched incoming connections to one local device. Use a device with its own firewall and a stable address.'),endpoint:'/api/router/firewall',writeEndpoint:'/api/router/firewall/dmz',read:setting('/api/router/firewall',['dmz_enabled','dmz_ip']),liveData:R.normalizeSwitches,source:'firewall_config',guardKeys:['dmz_enabled','dmz_ip'],fields:[onoff('dmz_enabled','DMZ'),field('dmz_ip',tr('Destination IPv4 address'),{placeholder:'192.168.0.10'})],
    validate:async(_,all)=>{R.switchValue(all.dmz_enabled,'DMZ');if(all.dmz_ip.trim())R.ipv4(all.dmz_ip,tr('DMZ address'));if(all.dmz_enabled==='1')R.host(all.dmz_ip,await readLAN(),tr('DMZ address'));},
    body:(_,all)=>({dmz_enabled:all.dmz_enabled,dmz_ip:all.dmz_ip.trim()}),preflight:async body=>{if(body.dmz_enabled==='1')R.host(body.dmz_ip,await readLAN(),tr('DMZ address'));},
    warning:(_,all)=>all.dmz_enabled==='1' ? tr("DMZ exposes {0} to otherwise unmatched incoming traffic. Confirm this device has its own firewall before enabling DMZ.", all.dmz_ip) : tr('Disable the DMZ host? Incoming connections to it may stop.')
  });
  const portsSummary = summary('network/ports','ports-overview',tr('Explicit incoming connections'),tr('A rule maps one external port to one local device. Mobile carriers may block incoming traffic or use carrier-grade NAT.'),[['state',tr('Port forwarding')],['rules',tr('Reported rules'),true],['enabled',tr('Enabled rules'),true],['upnp',tr('UPnP')]]);
  UI.makeForm({route:'network/ports',id:'ports-switch',title:tr('Port forwarding service'),description:tr('The global switch controls whether configured port-forward rules are active.'),endpoint:'/api/router/firewall',writeEndpoint:'/api/router/firewall/port-forward/switch',read:setting('/api/router/firewall',['port_forward_switch']),liveData:R.normalizeSwitches,source:'firewall_config',guardKeys:['port_forward_switch'],fields:[onoff('port_forward_switch',tr('Port forwarding'))],validate:(_,all)=>R.switchValue(all.port_forward_switch,tr('port forwarding')),warning:tr('Changing the global switch affects all configured port-forward rules. Apply this change?')});
  const ports = UI.panel('network/ports','port-rules',tr('Forwarding rules'),tr('Add a mapping or remove a reported rule. External and internal ports must be between 1 and 65535.')), toolbar = el('div','form-actions rule-toolbar');
  const reload = button(tr('Read current rules'),()=>loadRules()); toolbar.append(reload); ports.append(toolbar);
  const ruleList = el('div','rule-list'), portCaption = el('p','reading-caption',tr('Waiting for live rule readings.')); ports.append(ruleList,portCaption);
  const form = el('form','settings-form forward-editor'); form.autocomplete='off';
  const fields = el('fieldset','form-grid'); fields.disabled=true; fields.append(el('legend','form-group-heading',tr('New port-forward rule')));
  const inputs = {};
  for (const [key,label,options] of [['name',tr('Rule name')],['protocol',tr('Protocol'),[['tcp','TCP'],['udp','UDP'],['tcp+udp',tr('TCP + UDP')]]],['wan_port',tr('External port')],['lan_ip',tr('Local device address')],['lan_port',tr('Internal port')],['enabled',tr('Rule status'),[['1',tr('Enabled')],['0',tr('Disabled')]]]]) {
    const wrapper=el('div','form-field'), labelNode=el('label','',label), input=el(options ? 'select' : 'input');input.id=`port-rule-${key}`;input.name=key;labelNode.htmlFor=input.id;
    if(options)for(const [value,text]of options)input.add(new Option(text,value));else{input.type='text';input.required=true;input.autocomplete='off';if(key.endsWith('_port')){input.inputMode='numeric';input.maxLength=5;}if(key==='name')input.maxLength=64;}
    wrapper.append(labelNode,input);fields.append(wrapper);inputs[key]=input;
  }
  const actions=el('div','form-actions'),add=button(tr('Add rule'),null,'primary'),discard=button(tr('Discard draft'),resetDraft);add.type='submit';actions.append(add,discard);form.append(fields,actions);ports.append(form);const feedback=UI.status(ports);
  let rawRules=null, ruleEntries=[], attempted=false, loading=false, dirty=false, healthy=false;
  const draft=()=>Object.fromEntries(Object.entries(inputs).map(([key,input])=>[key,input.value]));
  function resetDraft(){form.reset();dirty=false;ports.dataset.dirty='false';updateActions();}
  function updateActions(){fields.disabled=!rawRules || !healthy || !API.connected;add.disabled=!dirty || !rawRules || !healthy || !API.connected;discard.disabled=!dirty;reload.disabled=loading || !API.connected;for(const card of ruleList.children)if(card.deleteButton)card.deleteButton.disabled=!API.connected || !healthy || !card.rule.removable;}
  form.addEventListener('input',()=>{dirty=!!(inputs.name.value || inputs.wan_port.value || inputs.lan_ip.value || inputs.lan_port.value);ports.dataset.dirty=String(dirty);updateActions();if(dirty)say(feedback,tr('Unsaved rule · live updates keep your draft.'));});
  form.addEventListener('change',()=>form.dispatchEvent(new Event('input')));
  function applyRules(data){ruleEntries=R.rules(data);rawRules=data.rule_list;healthy=true;drawRules();updateActions();renderSummaries();}
  function drawRules(){
    if(!ruleEntries.length){ruleList.replaceChildren(el('p','empty',tr('No port-forward rules configured.')));return;}
    const existing=new Map([...ruleList.children].filter(node=>node.dataset.ruleId).map(node=>[node.dataset.ruleId,node])),ordered=[];
    for(const [index,rule]of ruleEntries.entries()){
      const key=JSON.stringify([rule.id,index]);let card=existing.get(key);
      if(!card){card=el('article','rule-card');card.dataset.ruleId=key;const copy=el('div'),top=el('div','title-count'),title=el('h3'),tag=el('span','tag'),mapping=el('p','mono'),buttons=el('div','form-actions rule-actions'),remove=button(tr('Delete'),()=>deleteRule(card.rule),'danger');top.append(title,tag);copy.append(top,mapping);buttons.append(remove);card.append(copy,buttons);Object.assign(card,{heading:title,tag,mapping,deleteButton:remove});}
      card.rule=rule;card.heading.textContent=rule.name || tr('Unnamed rule');card.tag.textContent=state(rule.enabled);card.mapping.textContent=`${String(rule.protocol || tr('Not reported')).toUpperCase()} · ${rule.wan_port || '—'} → ${rule.lan_ip || '—'}:${rule.lan_port || '—'}`;card.deleteButton.setAttribute('aria-label',tr("Delete {0}", rule.name || tr('unnamed rule')));ordered.push(card);
    }
    const wanted=new Set(ordered);for(const card of [...ruleList.children])if(!wanted.has(card))card.remove();ordered.forEach((card,index)=>{if(ruleList.children[index]!==card)ruleList.insertBefore(card,ruleList.children[index] || null);});
  }
  async function loadRules(){
    if(loading || API.locked || !API.connected || UI.saving)return;loading=true;attempted=true;updateActions();const epoch=UI.epoch;
    try{const data=await request('/api/router/firewall/port-forward');if(!UI.current(epoch))return;applyRules(data);say(feedback,tr('Current rules loaded.'));}
    catch(error){if(UI.current(epoch)){healthy=false;updateActions();say(feedback,tr("Rules unavailable. {0}. Read current rules to try again.", error.message),true);}}
    finally{loading=false;updateActions();}
  }
  async function verifiedRules(check,current){
    for(const wait of [0,700,1400]){
      if(wait)await new Promise(resolve=>setTimeout(resolve,wait));if(!current())return false;
      const data=await request('/api/router/firewall/port-forward');if(!current())return false;applyRules(data);if(check(ruleEntries))return true;
    }
    return false;
  }
  form.addEventListener('submit',async event=>{
    event.preventDefault();if(!dirty || !healthy || !form.reportValidity() || UI.saving)return;const epoch=UI.epoch;
    try{
      const all=draft(),body=R.portBody(all,await readLAN(),ruleEntries);if(!UI.current(epoch))return;
      if(!await confirm(tr('Add port-forward rule'),tr("Forward incoming {0} port {1} to {2}:{3}? The rule will be {4}.", body.protocol.toUpperCase(), body.wan_port, body.lan_ip, body.lan_port, body.enabled==='1' ? 'enabled' : 'disabled'),{label:tr('Add rule')}))return;
      await mutate(feedback,async current=>{
        const [latest,lan]=await Promise.all([request('/api/router/firewall/port-forward'),readLAN()]);if(!current())return;
        R.portBody(all,lan,R.rules(latest));say(feedback,tr('Adding rule…'));healthy=false;updateActions();
        await request('/api/router/firewall/port-forward',json('POST',body));if(!current())return;
        const matched=await verifiedRules(entries=>entries.some(rule=>['name','protocol','wan_port','lan_ip','lan_port','enabled'].every(key=>String(rule[key])===String(body[key]))),current);if(!current())return;
        if(matched){resetDraft();say(feedback,tr('Rule added and confirmed by the router.'));}else{healthy=false;say(feedback,tr('Request accepted, but the rule is not confirmed. Your draft is kept. Read current rules before retrying.'),true);}
      });
    }catch(error){if(UI.current(epoch))say(feedback,error.message,true);}finally{updateActions();}
  });
  async function deleteRule(rule){
    if(!rawRules || !healthy || !rule.removable || UI.saving)return;
    const expected=JSON.parse(JSON.stringify(rawRules));
    if(!await confirm(tr('Delete port-forward rule'),tr("Remove {0} ({1} port {2})? Incoming connections using it will stop.", rule.name || tr('this rule'), rule.protocol || tr('protocol unreported'), rule.wan_port || 'unreported'),{label:tr('Delete rule'),destructive:true}))return;
    await mutate(feedback,async current=>{
      const latest=await request('/api/router/firewall/port-forward');if(!current())return;R.rules(latest);
      if(R.canonical(latest.rule_list)!==R.canonical(expected)){applyRules(latest);say(feedback,tr('The rule list changed. Review the current rule before deleting it.'),true);return;}
      healthy=false;updateActions();say(feedback,tr('Removing rule…'));
      await request('/api/router/firewall/port-forward',json('POST',{action:'delete',id:rule.id,expected_rules:expected}));if(!current())return;
      const matched=await verifiedRules(entries=>entries.length===expected.length-1 && R.canonical(entries.map(entry=>{const {id,...raw}=entry.raw;return raw;}))===R.canonical(expected.filter((_,index)=>ruleEntriesId(expected[index],index)!==rule.id).map(entry=>{const {id,...raw}=entry;return raw;})),current);if(!current())return;
      if(matched)say(feedback,tr('Rule removed and confirmed by the router.'));else{healthy=false;say(feedback,tr('Removal is not confirmed. Read current rules before another change.'),true);}
    });updateActions();
  }
  const ruleEntriesId=(rule,index)=>String(rule.id ?? index);
  UI.track({route:'network/ports',get loaded(){return attempted;},get dirty(){return dirty;},load:loadRules,clear(){rawRules=null;ruleEntries=[];attempted=false;loading=false;healthy=false;resetDraft();ruleList.replaceChildren();say(feedback,tr('Unlock the console to load rules.'));updateActions();}});
  UI.makeForm({route:'network/qos',id:'qos-config',title:tr('Traffic prioritization'),description:tr('Use the router’s built-in traffic policy. Individual device priorities and custom bandwidth limits are unavailable in this console.'),endpoint:'/api/router/qos',read:setting('/api/router/qos',['qos_switch']),liveData:R.normalizeSwitches,source:'qos_config',guardKeys:['qos_switch'],fields:[onoff('qos_switch',tr('Quality of service'))],validate:(_,all)=>R.switchValue(all.qos_switch,tr('quality of service')),warning:tr('Changing traffic prioritization may affect active transfers. Apply the router’s QoS setting?')});
  UI.makeForm({route:'network/ttl',id:'ttl-config',title:tr('Outbound packet lifetime'),description:tr('Rewrite outbound packets on the cellular software path. Qualcomm hardware acceleration may bypass these rules on some interfaces or established flows.'),endpoint:'/api/router/ttl',
    fields:[choices('enabled',tr('TTL rewriting'),[['true',tr('On')],['false',tr('Off')]],{boolean:true}),field('ipv4_ttl',tr('IPv4 TTL'),{type:'number',min:1,max:255,help:tr('The value placed on IPv4 packets leaving the cellular interface.')}),field('ipv6_hop_limit',tr('IPv6 hop limit'),{type:'number',min:1,max:255,help:tr('The value placed on IPv6 packets leaving the cellular interface.')})],
    validate:(_,all)=>{for(const [key,label] of [['ipv4_ttl',tr('IPv4 TTL')],['ipv6_hop_limit',tr('IPv6 hop limit')]]){const value=Number(all[key]);if(!Number.isInteger(value)||value<1||value>255)throw new Error(tr("{0} must be a whole number from 1 to 255.", label));}},
    warning:(_,all)=>all.enabled==='true' ? tr("Enable outbound packet rewriting with IPv4 TTL {0} and IPv6 hop limit {1}? This applies immediately; it does not reconnect mobile data, restart the firewall or disable hardware acceleration.", all.ipv4_ttl, all.ipv6_hop_limit) : tr('Disable the agent’s packet TTL and hop-limit rules?')
  });
  const qosSummary=summary('network/qos','qos-overview',tr('Router traffic policy'),tr('Live status of the firmware’s built-in policy.'),[['state',tr('Quality of service')]]);
  function renderSummaries(){
    let lan=samples.get('lan_config')?.data;try{lan=R.normalizeLAN(lan);}catch{lan=null;}
    const clientData=samples.get('clients')?.data,firewall=samples.get('firewall_config')?.data,upnp=samples.get('upnp_config')?.data,qos=samples.get('qos_config')?.data;
    let pool='—';if(lan?.dhcp_enable==='0')pool=0;else if(lan)try{const start=R.host(lan.dhcp_start,lan),end=R.host(lan.dhcp_end,lan);if(end>=start)pool=end-start+1;}catch{/* Invalid ranges stay unreported. */}
    values(lanSummary,{address:lan?.lan_ipaddr,dhcp:state(lan?.dhcp_enable),pool,leases:Array.isArray(clientData?.dhcp_leases) ? clientData.dhcp_leases.length : '—'});stamp(lanSummary,['lan_config','clients']);
    values(firewallSummary,{firewall:state(firewall?.firewall_switch),nat:state(firewall?.nat_switch),upnp:state(upnp?.upnp_switch),dmz:firewall?.dmz_enabled===undefined ? tr('Not reported') : enabled(firewall.dmz_enabled) ? firewall.dmz_ip || tr('Address unreported') : tr('Disabled')});stamp(firewallSummary,['firewall_config','upnp_config']);
    values(portsSummary,{state:state(firewall?.port_forward_switch),rules:rawRules ? ruleEntries.length : '—',enabled:rawRules ? ruleEntries.filter(rule=>enabled(rule.enabled)).length : '—',upnp:state(upnp?.upnp_switch)});stamp(portsSummary,['firewall_config','port_rules','upnp_config']);portCaption.textContent=portsSummary.caption.textContent;portCaption.dataset.stale=portsSummary.caption.dataset.stale;
    values(qosSummary,{state:state(qos?.qos_switch)});stamp(qosSummary,['qos_config']);
  }
  window.addEventListener('dashboard:readings',event=>{
    if(event.detail.snapshot)for(const name of ['lan_config','firewall_config','upnp_config','port_rules','qos_config'])if(samples.has(name))samples.get(name).unavailable=true;
    for(const name of ['lan_config','firewall_config','upnp_config','port_rules','qos_config','clients']){
      const reading=event.detail.readings[name];if(!reading)continue;
      const previous=samples.get(name);let usable=!!reading.ok && !!reading.data;
      if(usable)try{if(name==='lan_config')R.normalizeLAN(reading.data);if(name==='port_rules')R.rules(reading.data);if(name==='clients' && !Array.isArray(reading.data.dhcp_leases))usable=false;}catch{usable=false;}
      if(usable)samples.set(name,{data:reading.data,at:Date.now(),unavailable:false});else if(previous)previous.unavailable=true;
      if(name==='port_rules'){
        if(usable)try{if(!UI.saving)applyRules(reading.data);}catch{healthy=false;if(previous)previous.unavailable=true;}
        else healthy=false;
      }
    }
    renderSummaries();drawLeases();updateActions();
  });
  window.addEventListener('dashboard:mutation-complete',updateActions);
  window.addEventListener('dashboard:connection',()=>{renderSummaries();updateActions();});
  window.addEventListener('dashboard:locked',()=>{samples.clear();reconnectNote.hidden=true;reconnect.removeAttribute('href');reconnect.textContent='';drawLeases();renderSummaries();});
  renderSummaries();drawLeases();
})();
