'use strict';
(() => {
  const tr = globalThis.OpenU60I18n?.t || ((s,...v)=>s.replace(/\{(\d+)\}/g,(match,i)=>Number(i)<v.length?String(v[i]):match));
  const UI=OpenU60UI,API=OpenU60Console,M=OpenU60Clients,{el}=UI;
  const host=document.querySelector('#devices');
  const detail=el('div','client-detail-view');detail.dataset.views='network/clients';detail.dataset.viewHidden=String(OpenU60Navigation.current.key!=='network/clients');
  const intro=el('p','section-subtitle',tr('Per-device measurements from your router. Wi-Fi traffic includes local transfers as well as internet use.'));
  const overview=el('dl','client-overview'),counts={};
  for(const [key,label]of [['connected',tr('Confirmed connected')],['known',tr('Devices reported')]]){const pair=el('div'),value=el('dd','mono','—');pair.append(el('dt','',label),value);counts[key]=value;overview.append(pair);}
  const tools=el('div','client-view-tools'),label=el('label','',tr('Show devices')),filter=el('select');filter.id='client-status-filter';label.htmlFor=filter.id;
  for(const [value,text]of [['all',tr('All reported devices')],['connected',tr('Confirmed connected')],['known',tr('Not confirmed connected')]])filter.add(new Option(text,value));tools.append(label,filter);
  const caption=el('p','reading-caption',tr('Waiting for detailed device readings.')),routerCaps=el('p','client-router-limits'),list=el('div','client-card-list');list.id='client-detail-list';list.setAttribute('aria-label',tr('Device connection details'));
  detail.append(overview,tools,caption,list,routerCaps,intro);host.querySelector('.table-scroll').after(detail);
  let base=null,details=null,receivedAt=null,available=false,stream='connecting',expiryTimer;
  const search=document.querySelector('#device-search'),cards=new Map();
  const live=()=>API.connected && stream==='live' && available && receivedAt!==null && Date.now()-receivedAt<10000 && !document.hidden;
  function expire(){clearTimeout(expiryTimer);if(receivedAt!==null && !document.hidden)expiryTimer=setTimeout(render,Math.max(1,10001-(Date.now()-receivedAt)));}
  function metric(group,key,label,nodes){const pair=el('div'),value=el('dd','mono',tr('Not reported'));value.dataset.metric=key;pair.append(el('dt','',label),value);group.append(pair);nodes[key]=value;}
  function card(key){
    const node=el('article','client-card');node.dataset.mac=key;const top=el('div','client-card-heading'),copy=el('div'),name=el('h3'),kind=el('p','client-connection'),badge=el('span','tag');copy.append(name,kind);top.append(copy,badge);
    const identity=el('dl','client-identity'),identityNodes={};for(const [key,label]of [['mac',tr('MAC address')],['ipv4',tr('IPv4 address')],['ipv6',tr('IPv6 address')],['address_source',tr('Address assignment')]]){const pair=el('div'),value=el('dd','mono');pair.append(el('dt','',label),value);identity.append(pair);identityNodes[key]=value;}
    const traffic=el('dl','client-speeds'),numbers={};metric(traffic,'download',tr('Current download'),numbers);metric(traffic,'upload',tr('Current upload'),numbers);
    const more=el('dl','client-measurements');for(const [key,label]of [['downloadLimit',tr('Configured download limit')],['uploadLimit',tr('Configured upload limit')],['downloadPeak',tr('Observed download peak')],['uploadPeak',tr('Observed upload peak')]])metric(more,key,label,numbers);
    const extra=el('details','client-radio-details'),extraFields=el('dl','client-measurements');extra.append(el('summary','',tr('Link & transfer details')),extraFields);
    for(const [key,label]of [['downloadTotal',tr('Downloaded over this Wi-Fi association')],['uploadTotal',tr('Uploaded over this Wi-Fi association')],['downloadLink',tr('Wi-Fi download link rate')],['uploadLink',tr('Wi-Fi upload link rate')],['signal',tr('Wi-Fi signal')],['duration',tr('Connection duration')]])metric(extraFields,key,label,numbers);
    const note=el('p','client-sample-note'),limitsNote=el('p','field-help client-limit-note',tr('Configured per-device limits are shown only when reported. “Not reported” does not mean unlimited.'));
    node.append(top,traffic,identity,more,extra,note,limitsNote);return {node,name,kind,badge,identityNodes,numbers,note};
  }
  function duration(value){const n=M.number(value);if(n===null)return tr('Not reported');if(n<60)return tr("{0} s", Math.floor(n));if(n<3600)return tr("{0} min", Math.floor(n/60));return tr("{0} h {1} min", Math.floor(n/3600), Math.floor(n%3600/60));}
  function update(view,item){
    const connected=live() && item.connected===true && !item.connection_stale,current=connected && !item.telemetry_stale;
    view.name.textContent=item.name;view.name.title=item.name;view.kind.textContent=`${M.connection(item)}${item.interface ? ` · ${item.interface}` : ''}`;
    view.badge.textContent=item.connected===true ? connected ? tr('Connected') : tr('Last reported connected') : item.connected===false ? tr('Not connected') : tr('Connection unconfirmed');view.badge.dataset.connected=String(connected);
    for(const [key,node]of Object.entries(view.identityNodes))node.textContent=['ipv4','ipv6'].includes(key) ? item[key].join(', ') || tr('Not reported') : String(item[key] || tr('Not reported'));
    const traffic=item.traffic || {},peaks=item.peaks || {},limits=item.limits || {},link=item.link || {};
    const signal=Number(link.signal_dbm),signalText=link.signal_dbm!==null && link.signal_dbm!==undefined && Number.isFinite(signal) && signal<=0 && signal>=-127 ? tr("{0} dBm", signal.toFixed(0)) : tr('Not reported');
    const value={download:M.rate(current ? traffic.download_bps : null,'—'),upload:M.rate(current ? traffic.upload_bps : null,'—'),downloadLimit:M.rate(limits.download_bps),uploadLimit:M.rate(limits.upload_bps),downloadPeak:M.rate(peaks.download_bps),uploadPeak:M.rate(peaks.upload_bps),downloadTotal:M.bytes(traffic.download_bytes),uploadTotal:M.bytes(traffic.upload_bytes),downloadLink:link.download_mbps===undefined || M.number(link.download_mbps)===null ? tr('Not reported') : tr("{0} Mbps", Number(link.download_mbps).toFixed(1)),uploadLink:link.upload_mbps===undefined || M.number(link.upload_mbps)===null ? tr('Not reported') : tr("{0} Mbps", Number(link.upload_mbps).toFixed(1)),signal:signalText,duration:duration(item.connected_seconds)};
    for(const [key,text]of Object.entries(value))OpenU60Numbers.set(view.numbers[key],text);
    const since=M.number(peaks.since),period=since!==null ? tr(" Peaks are the highest sampled rates since {0}, while this page is monitored.", new Date(since).toLocaleTimeString(globalThis.OpenU60I18n?.locale)) : tr(' Peaks need at least two counter readings.');
    view.note.textContent=tr("{0}{1}{2} Wi-Fi link rates describe the radio connection.", current ? '' : tr('Showing last reported details. '), traffic.scope==='wifi_link' ? tr('Traffic measures this device’s Wi-Fi transfers, including local traffic.') : tr('The router has not reported per-device traffic counters for this connection.'), period);
  }
  function render(){
    const items=M.merge(base,details),query=search.value;
    OpenU60Numbers.set(counts.known,base || details ? items.length : '—');
    OpenU60Numbers.set(counts.connected,details && available ? items.filter(item=>item.connected===true && !item.connection_stale).length : '—');
    overview.querySelector('dt').textContent=live() || !details ? tr('Confirmed connected') : tr('Last confirmed connected');
    caption.textContent=receivedAt ? `${live() ? tr('Live device readings') : tr('Last received device readings')} · ${new Date(receivedAt).toLocaleTimeString(globalThis.OpenU60I18n?.locale)}${available ? details?.inventory_complete ? '' : tr(' · some connection sources unavailable') : tr(' · detailed source unavailable')}` : tr('Waiting for detailed device readings. Names and addresses remain available from host hints and DHCP.');caption.dataset.stale=String(!live());
    const caps=details?.router_limits;routerCaps.textContent=caps && (M.number(caps.upload_bps)!==null || M.number(caps.download_bps)!==null) ? tr("Router-wide configured limits: download {0} · upload {1}. These apply to the router, not to each device.", M.rate(caps.download_bps), M.rate(caps.upload_bps)) : tr('Router-wide speed limits are not reported. Configured device limits and observed peaks appear separately below.');
    const found=items.filter(item=>M.matches(item,query) && (filter.value==='all' || (filter.value==='connected' ? item.connected===true && !item.connection_stale : item.connected!==true || item.connection_stale)));
    const wanted=new Set(found.map(item=>item.mac));for(const [key,view]of cards)if(!wanted.has(key)){view.node.remove();cards.delete(key);}
    const empty=list.querySelector('.empty');if(empty)empty.remove();
    if(!found.length)list.append(el('p','empty',query ? tr('No devices match your search.') : filter.value==='connected' ? tr('No devices currently confirmed connected.') : tr('No devices reported.')));
    for(const [index,item]of found.entries()){let view=cards.get(item.mac);if(!view){view=card(item.mac);cards.set(item.mac,view);}update(view,item);if(list.children[index]!==view.node)list.insertBefore(view.node,list.children[index] || null);}
    if(OpenU60Navigation.current.key==='network/clients')OpenU60Numbers.set(document.querySelector('#device-count'),base || details ? items.length : '—');
  }
  search.addEventListener('input',render);filter.addEventListener('change',render);
  window.addEventListener('dashboard:readings',event=>{
    if(event.detail.snapshot)available=false;
    const hosts=event.detail.readings.clients;if(hosts?.ok)base=hosts.data;
    const reading=event.detail.readings.client_details;
    if(reading){if(reading.ok && Array.isArray(reading.data?.devices)){
      const next={...reading.data,devices:reading.data.devices.map(device=>({...device}))};
      for(const previous of details?.devices || []){
        const item=next.devices.find(item=>M.mac(item.mac)===M.mac(previous.mac));
        if(!item && !(previous.connection_type==='wifi' ? next.sources?.wireless : next.sources?.wired))next.devices.push({...previous,connection_stale:true,telemetry_stale:true});
        else if(item && next.sources?.stations?.[item.interface]===false)Object.assign(item,{traffic:previous.traffic,peaks:previous.peaks,link:previous.link,telemetry_stale:true});
      }
      details=next;receivedAt=Date.now();available=true;expire();
    }else available=false;}
    render();
  });
  window.addEventListener('dashboard:connection',event=>{stream=event.detail.state;render();});
  window.addEventListener('dashboard:navigate',render);
  document.addEventListener('visibilitychange',()=>{clearTimeout(expiryTimer);render();if(!document.hidden && receivedAt!==null && Date.now()-receivedAt<10000)expire();});
  window.addEventListener('pagehide',()=>clearTimeout(expiryTimer));
  window.addEventListener('dashboard:locked',()=>{clearTimeout(expiryTimer);base=null;details=null;receivedAt=null;available=false;filter.value='all';cards.clear();list.replaceChildren();render();});
  render();
})();
