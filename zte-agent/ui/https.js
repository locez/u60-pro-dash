'use strict';
(() => {
  const tr = globalThis.OpenU60I18n?.t || ((s,...v)=>s.replace(/\{(\d+)\}/g,(match,i)=>Number(i)<v.length?String(v[i]):match));
  const UI=OpenU60UI, API=OpenU60Console, {el,button}=UI, route='settings/https';
  let data=null, epoch=0;
  const panel=UI.panel(route,'https-settings',tr('HTTPS & install'),tr('Secure local access with the router’s certificate authority. Each device trusts it once.'));
  const facts=el('dl','settings-facts'), nodes={};
  for(const [key,label] of [['state','HTTPS'],['expires',tr('Certificate valid until')],['clock',tr('Router clock')],['authority',tr('Certificate authority')],['scope',tr('Can vouch for')],['fingerprint',tr('SHA-256 fingerprint')],['sha1',tr('SHA-1 fingerprint for Windows')]]){
    const row=el('div'), value=el('dd',key==='fingerprint'||key==='sha1'?'mono https-fingerprint':'','—');value.id=`https-${key}`;row.append(el('dt','',label),value);facts.append(row);nodes[key]=value;
  }
  const addresses=el('div','form-actions'), actions=el('div','form-actions diagnostic-actions'), feedback=el('p','action-status'), caption=el('p','reading-caption',tr('Waiting for HTTPS status.'));feedback.setAttribute('role','status');
  async function change(kind){
    const current=epoch, snapshot=data;
    if(!snapshot||API.locked)return;
    const date=Number.isFinite(snapshot.clock_utc)?new Date(snapshot.clock_utc*1000).toLocaleString(globalThis.OpenU60I18n?.locale): tr('not reported');
    const details=kind==='reset'?tr('Remove the router’s HTTPS certificates and turn off its HTTPS listener. Remove the old authority from your devices afterward. If you are using HTTPS now, reopen the router’s HTTP address after this change.'):tr("{0} the router’s certificate authority. The router clock is {1}{2} Devices must trust the router’s authority before opening its HTTPS address.", kind==='on'?tr('Enable HTTPS using'):tr('Renew the certificate from'), date, snapshot.clock_verified?' (verified).':tr(' (not independently verified).'));
    if(!await UI.confirm(kind==='reset'?tr('Reset HTTPS?'):kind==='renew'?tr('Renew certificate?'):tr('Turn on HTTPS?'),details,{label:kind==='reset'?tr('Reset HTTPS'):kind==='renew'?tr('Renew certificate'):tr('Turn on HTTPS'),destructive:kind==='reset',typed:kind==='reset'}))return;
    if(current!==epoch||API.locked||OpenU60Navigation.current.key!==route)return;
    await UI.mutate(feedback,async active=>{const result=await UI.request(`/api/https/${kind}`,UI.json('POST',{confirm:true}));if(active()&&current===epoch){draw(result);UI.say(feedback,kind==='reset'?tr('HTTPS was reset. Open the router’s HTTP address to continue.'):kind==='renew'?tr('Certificate renewed.'):tr('HTTPS enabled. Install the certificate on this device before opening an HTTPS address.'));}});
  }
  const enable=button(tr('Turn on HTTPS'),()=>change('on'),'primary'), renew=button(tr('Renew certificate'),()=>change('renew')), reset=button(tr('Reset HTTPS'),()=>change('reset'),'danger');
  for(const node of [enable,renew,reset])node.disabled=true;
  actions.append(enable,renew,reset);panel.append(facts,addresses,actions,feedback,caption);
  const install=UI.panel(route,'https-install',tr('Install on your devices'),tr('Install only this router’s public certificate, then open one of its HTTPS addresses. Private keys never leave the router.'));
  const downloads=el('div','form-actions');
  for(const [label,href]of[[tr('Profile for iPhone & iPad'),'/ui/https/u60-pro-ca.mobileconfig'],[tr('Certificate for Android, Mac & Windows'),'/ui/https/u60-pro-ca.crt']]){const link=el('a','button',label);link.href=href;link.setAttribute('download','');downloads.append(link);}downloads.hidden=true;
  const trust=el('p','panel-note',tr('Before trusting the certificate, compare the SHA-256 fingerprint above with the one shown on your device. For a trusted comparison, use this page over HTTPS on a device that already trusts the router, or compare with sha256sum /data/zte-agent-tls/ca.crt over SSH. Plain HTTP can be altered by someone on the network.'));
  const steps=el('dl','settings-facts');
  for(const [name,text]of[[tr('iPhone & iPad'),tr('Download the profile. Open Settings → Profile Downloaded → Install. Then Settings → General → About → Certificate Trust Settings: enable full trust for the router’s authority. In Safari, use Share → Add to Home Screen.')],[tr('Android'),tr('Download the certificate. In Settings, search for Install a CA certificate, confirm with your screen lock and select the file. Open an HTTPS address above, then use your browser’s Install or Add to Home screen option.')],[tr('Mac'),tr('Download and open the certificate in Keychain Access. Open its Trust section and select Always Trust. In Safari, open an HTTPS address above, then File → Add to Dock.')],[tr('Windows'),tr('Open the certificate → Install Certificate → Current User → Place all certificates in Trusted Root Certification Authorities. Compare Windows’ SHA-1 Thumbprint with the value above. Open an HTTPS address in Chrome or Edge and choose Install.')]]){const row=el('div');row.append(el('dt','',name),el('dd','',text));steps.append(row);}
  install.append(downloads,trust,steps,el('p','panel-note',tr('Devices using private DNS, a VPN or browser DNS-over-HTTPS may not resolve the router’s local name; use its listed LAN address. Each origin keeps its own console sign-in.')));
  function draw(value){
    data=value;nodes.state.textContent=value.enabled?(value.listening?tr('On'):tr('On · not listening')):tr('Off');
    nodes.expires.textContent=value.not_after?new Date(value.not_after*1000).toLocaleDateString(globalThis.OpenU60I18n?.locale)+(value.renew_due?tr(' · renewal due'):''):tr('Not reported');
    nodes.clock.textContent=Number.isFinite(value.clock_utc)?new Date(value.clock_utc*1000).toLocaleString(globalThis.OpenU60I18n?.locale)+(value.clock_verified?tr(' · verified'):tr(' · not independently verified')):tr('Not reported');
    nodes.authority.textContent=value.ca_name||value.kept_ca||tr('Not created');nodes.scope.textContent=(value.ca_covers||[]).join(', ')||tr('Not reported');nodes.fingerprint.textContent=value.ca_sha256||tr('Not reported');nodes.sha1.textContent=value.ca_sha1||tr('Not reported');
    addresses.replaceChildren();for(const url of value.urls||[]){try{const parsed=new URL(url);if(parsed.protocol!=='https:'||parsed.username||parsed.password)continue;const link=el('a','button',url);link.href=parsed.href;link.rel='noreferrer';addresses.append(link);}catch{}}
    enable.hidden=!!value.enabled&&!!value.listening;enable.textContent=value.enabled?tr('Try HTTPS again'):tr('Turn on HTTPS');renew.hidden=!value.enabled;for(const node of [enable,renew,reset])node.disabled=false;downloads.hidden=!value.enabled;
    caption.textContent=(API.connected?tr('Live HTTPS status'):tr('Last received HTTPS status'))+(value.error?` · ${value.error}`:'');caption.dataset.stale=String(!API.connected);
  }
  function clear(){epoch++;data=null;for(const value of Object.values(nodes))value.textContent='—';addresses.replaceChildren();downloads.hidden=true;for(const node of[enable,renew,reset])node.disabled=true;feedback.textContent='';caption.textContent=tr('Waiting for HTTPS status.');}
  window.addEventListener('dashboard:readings',event=>{if(OpenU60Navigation.current.key!==route)return;const reading=event.detail.readings.https_status;if(reading?.ok)draw(reading.data);else if(reading){caption.textContent=tr('HTTPS status unavailable.');caption.dataset.stale='true';}});
  window.addEventListener('dashboard:locked',clear);window.addEventListener('dashboard:navigate',()=>{if(OpenU60Navigation.current.key!==route)clear();});
  window.addEventListener('dashboard:connection',()=>{if(data){caption.dataset.stale=String(!API.connected);if(!API.connected)caption.textContent=tr('Last received HTTPS status · connection interrupted.');}});
})();
