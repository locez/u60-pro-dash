'use strict';
(() => {
  const tr = globalThis.OpenU60I18n?.t || ((s,...v)=>s.replace(/\{(\d+)\}/g,(match,i)=>Number(i)<v.length?String(v[i]):match));
  const UI = OpenU60UI, API = OpenU60Console, A = OpenU60Advanced;
  const {el,button,say,json,request,confirm,mutate} = UI;
  const samples = new Map(); let history = [], processQuery = '', processSort = 'cpu', latestSweep, stopRequested = false;
  let latestSpeed, startingSpeed = 0, loadingServers = false, serversLoaded = false, terminalDirty = false;
  const sweepJob = new A.Job('sweep',180000);
  const num = value => value !== null && value !== undefined && value !== '' && Number.isFinite(Number(value)) ? Number(value) : null;
  const number = (value,unit = '',digits = 1) => num(value) === null ? '—' : `${Number(value).toFixed(digits)}${unit}`;
  const numeric = (node,value) => OpenU60Numbers.set(node,value);
  const bool = value => value === true || value === 'true' || value === 1 || value === '1';
  function table(headers,label) {
    const wrap = el('div','table-scroll diagnostic-table'), node = el('table'), head = el('thead'), row = el('tr'), body = el('tbody'); node.setAttribute('aria-label',label);
    for (const text of headers) { const cell = el('th','',text); cell.scope = 'col'; row.append(cell); }
    head.append(row); node.append(head,body); wrap.append(node); return {wrap,body};
  }
  function rows(body,data,columns,empty,options = {}) {
    if (!data.length) { const row = el('tr'), cell = el('td','empty',empty); cell.colSpan = columns.length; row.append(cell); body.replaceChildren(row); return; }
    const existing = new Map([...body.children].filter(row=>row.dataset.rowKey).map(row=>[row.dataset.rowKey,row])), ordered = [];
    for (const [index,item] of data.entries()) {
      const key = String(options.key ? options.key(item) : index);
      let row = existing.get(key);
      if (!row) { row = el('tr'); row.dataset.rowKey = key; for (const column of columns) row.append(el('td')); }
      columns.forEach((column,index)=>{
        const value = column(item), text = value === undefined || value === null || value === '' ? '—' : String(value);
        if (options.animate?.includes(index)) numeric(row.children[index],text); else row.children[index].textContent = text;
      });
      ordered.push(row);
    }
    const wanted = new Set(ordered);
    for (const row of [...body.children]) if (!wanted.has(row)) row.remove();
    ordered.forEach((row,index)=>{ if (body.children[index] !== row) body.insertBefore(row,body.children[index] || null); });
  }
  function stats(section,definitions) {
    const list = el('dl','diagnostic-stats'), nodes = {};
    for (const [key,label] of definitions) { const pair = el('div'); nodes[key] = el('dd','mono','—'); pair.append(el('dt','',label),nodes[key]); list.append(pair); }
    section.append(list); return nodes;
  }
  function caption(section) { const node = el('p','reading-caption',tr('Waiting for live readings.')); section.append(node); return node; }
  function stamp(node,name,unavailable = false) {
    const sample = samples.get(name);
    unavailable = unavailable || !!sample?.unavailable;
    const fresh = sample && Date.now()-sample.receivedAt < 12000;
    node.textContent = sample ? `${API.connected && fresh && !unavailable ? tr('Live') : tr('Last received')} · ${new Date(sample.receivedAt).toLocaleTimeString(globalThis.OpenU60I18n?.locale)}${unavailable ? tr(' · some readings unavailable') : ''}` : tr('Waiting for live readings.');
    node.dataset.stale = String(!API.connected || !fresh || unavailable);
  }
  const health = UI.panel('tools/diagnostics','diagnostic-health',tr('System health'),tr('Resource use and sensor readings from the agent. Missing measurements stay unreported.'));
  const healthStats = stats(health,[['cpu',tr('CPU use')],['memory',tr('Memory used')],['available',tr('Available memory')],['processes',tr('Processes')]]);
  const cores = el('p','field-help'), sensors = el('div','sensor-list'); health.append(cores,sensors); const healthCaption = caption(health);
  const processes = UI.panel('tools/diagnostics','diagnostic-processes',tr('Running processes'),tr('Inspect resource use without changing or stopping processes.'));
  const processActions = el('div','diagnostic-actions'), search = el('input'), sort = el('select'); search.type = 'search'; search.placeholder = tr('Find a process or PID'); search.id = 'process-search'; search.setAttribute('aria-label',tr('Find a process or PID')); sort.id = 'process-sort'; sort.setAttribute('aria-label',tr('Sort processes'));
  for (const [key,label] of [['cpu',tr('Highest CPU use')],['memory',tr('Largest memory use')],['name',tr('Process name')]]) sort.add(new Option(label,key));
  processActions.append(search,sort); processes.append(processActions);
  const processTable = table(['PROCESS','PID','CPU','MEMORY','STATE'],tr('Running processes')); processes.append(processTable.wrap);
  search.addEventListener('input',()=>{ processQuery = search.value.toLowerCase().trim(); drawProcesses(); }); sort.addEventListener('change',()=>{ processSort = sort.value; drawProcesses(); });
  function drawProcesses() {
    const data = samples.get('resources')?.data.processes;
    const found = (data?.data?.processes || []).filter(item=>`${item.name} ${item.pid}`.toLowerCase().includes(processQuery)).slice();
    found.sort((a,b)=>processSort === 'name' ? String(a.name).localeCompare(String(b.name)) : processSort === 'memory' ? Number(b.rss_kb)-Number(a.rss_kb) : Number(b.cpu_pct)-Number(a.cpu_pct));
    rows(processTable.body,found,[item=>item.name,item=>item.pid,item=>number(item.cpu_pct,' %'),item=>number(num(item.rss_kb) === null ? null : item.rss_kb/1024,tr(' MiB')),item=>item.state],processQuery ? tr('No processes match.') : tr('No process readings available.'),{key:item=>JSON.stringify([item.pid,item.name]),animate:[2,3]});
  }
  const interfaces = UI.panel('tools/diagnostics','diagnostic-interfaces',tr('Network interfaces & report'),tr('Inspect reported LAN and IPv6 WAN details, or save a local diagnostics report. Reports contain network information; review before sharing.'));
  const details = el('details','diagnostic-details'), summary = el('summary','',tr('Reported interface details')), interfaceOutput = el('pre','',tr('Waiting for interface readings.')); details.append(summary,interfaceOutput); interfaces.append(details);
  const reportActions = el('div','form-actions diagnostic-actions'), includeLabel = el('label','message-select'), includeHistory = el('input'); includeHistory.type = 'checkbox'; includeHistory.id = 'report-terminal-history'; includeLabel.append(includeHistory,el('span','',tr('Include AT terminal history')));
  const exportReport = button(tr('Save diagnostics report'),downloadReport); reportActions.append(exportReport,includeLabel); interfaces.append(reportActions); const reportFeedback = UI.status(interfaces);
  function downloadReport() {
    const report = {created_at:new Date().toISOString(),agent:location.origin,readings:Object.fromEntries(samples),...(includeHistory.checked ? {terminal_history:history} : {})};
    const blob = new Blob([JSON.stringify(report,null,2)],{type:'application/json'}), url = URL.createObjectURL(blob), link = el('a');
    link.href = url; link.download = `openu60-diagnostics-${new Date().toISOString().replace(/[:.]/g,'-')}.json`; document.body.append(link); link.click(); link.remove(); setTimeout(()=>URL.revokeObjectURL(url),5000);
    say(reportFeedback,tr('Diagnostics saved to this device. No report was uploaded.'));
  }
  function renderResources(data) {
    const cpu = data.cpu?.ok ? data.cpu.data : null, memory = data.memory?.ok ? data.memory.data : null;
    if (cpu) {
      numeric(healthStats.cpu,number(cpu.overall,' %'));
      if (!Array.isArray(cpu.cores)) cores.textContent = tr('Per-core usage unavailable.');
      else {
        for (const node of [...cores.childNodes]) if (node.nodeType === Node.TEXT_NODE) node.remove();
        while (cores.children.length > cpu.cores.length) cores.lastElementChild.remove();
        cpu.cores.forEach((value,index)=>{
          let item = cores.children[index];
          if (!item) { item = el('span','core-reading'); item.append(document.createTextNode(tr("Core {0}: ", index+1)),el('span')); cores.append(item); }
          numeric(item.querySelector('span'),number(value,' %'));
        });
      }
    }
    if (memory) { numeric(healthStats.memory,number(memory.usage_pct,' %')); numeric(healthStats.available,number(num(memory.available_kb) === null ? null : memory.available_kb/1024,tr(' MiB'))); }
    if (data.processes?.ok) numeric(healthStats.processes,String(data.processes.data?.total_count ?? '—'));
    if (data.thermal?.ok) {
      const reported = data.thermal.data?.sensors || [], existing = new Map([...sensors.querySelectorAll('.sensor-card')].map(card=>[card.dataset.sensorKey,card])), cards = [];
      for (const [index,sensor] of reported.entries()) {
        const key = JSON.stringify([sensor.id || sensor.name,index]); let card = existing.get(key);
        if (!card) { card = el('div','sensor-card'); card.dataset.sensorKey = key; card.append(el('span'),el('strong','mono')); }
        card.querySelector('span').textContent = sensor.name || sensor.id || tr('Sensor'); numeric(card.querySelector('strong'),number(sensor.temperature_c,' °C')); cards.push(card);
      }
      if (!cards.length) sensors.replaceChildren(el('p','carrier-empty',tr('No temperature sensors reported.')));
      else {
        const wanted = new Set(cards); for (const child of [...sensors.children]) if (!wanted.has(child)) child.remove();
        cards.forEach((card,index)=>{ if (sensors.children[index] !== card) sensors.insertBefore(card,sensors.children[index] || null); });
      }
    }
    interfaceOutput.textContent = JSON.stringify({lan:data.lan,ipv6_wan:data.wan6},null,2);
    drawProcesses(); stamp(healthCaption,'resources',[data.cpu,data.memory,data.thermal,data.processes].some(reading=>!reading?.ok));
  }

  const neighborPanel = UI.panel('tools/radio','diagnostic-neighbors',tr('Neighboring cells'),tr('Reported neighbors are candidates around the serving connection. They are not additional aggregated carriers or distinct physical sites.'));
  const neighborActions = el('div','form-actions diagnostic-actions'), scanNeighbors = button(tr('Scan neighboring cells'),async()=>{
    if (!await confirm(tr('Scan neighboring cells?'),tr('Ask the modem for neighboring LTE and NR cells. The scan can briefly affect mobile data.'),{label:tr('Scan neighbors')})) return;
    await mutate(neighborFeedback,async current=>{ await request('/api/cell/neighbors/scan',json('POST',{})); if (current()) say(neighborFeedback,tr('Neighbor scan requested. Live lists will update when the modem reports results; the modem can retain cached entries.')); });
  },'primary'); neighborActions.append(scanNeighbors); neighborPanel.append(neighborActions);
  const neighborTable = table(['RADIO','BAND','PCI','CHANNEL','RSRP'],tr('Neighboring LTE and NR cells')); neighborPanel.append(neighborTable.wrap); const neighborCaption = caption(neighborPanel), neighborFeedback = UI.status(neighborPanel);
  function renderNeighbors(data) {
    const found = [];
    for (const [rat,label] of [['lte','LTE'],['nr','NR']]) if (data[rat]?.data) for (const row of data[rat].data?.cell_list || []) found.push({...row,rat:label});
    rows(neighborTable.body,found,[item=>item.rat,item=>item.band,item=>item.pci,item=>item.earfcn,item=>num(item.rsrp) === null ? null : tr("{0} dBm", item.rsrp)],tr('No neighboring cells reported.'),{key:item=>JSON.stringify([item.rat,item.band,item.earfcn,item.pci]),animate:[4]});
    stamp(neighborCaption,'neighbors',!data.lte?.ok || !data.nr?.ok);
  }
  const sweepPanel = UI.panel('tools/radio','diagnostic-sweep',tr('Signal quality sweep'),tr('Run the modem’s quality scan and inspect reported results for each radio and channel.'));
  const sweepActions = el('div','form-actions diagnostic-actions'), startSweep = button(tr('Start signal sweep'),async()=>{
    if (!await confirm(tr('Start a signal quality sweep?'),tr('The modem will scan radio quality. Mobile data may be interrupted. The sweep continues if you leave this page.'),{label:tr('Start sweep')})) return;
    await mutate(sweepFeedback,async current=>{ await request('/api/cell/signal-detect/start',json('POST',{})); if (!current()) return; sweepJob.start(); stopRequested = false; renderSweep(latestSweep,true); });
  },'primary'), stopSweep = button(tr('Stop sweep'),async()=>{
    if (!await confirm(tr('Stop the signal sweep?'),tr('Ask the modem to stop its current quality sweep. Existing results stay available.'),{label:tr('Stop sweep')})) return;
    const wasRunning = bool(latestSweep?.progress?.data?.running);
    await mutate(sweepFeedback,async current=>{ await request('/api/cell/signal-detect/stop',json('POST',{})); if (current()) { stopRequested = true; say(sweepFeedback,tr('Stop requested. Waiting for the modem to report it has stopped.')); if (wasRunning) renderSweep(latestSweep,false); } });
  });
  sweepActions.append(startSweep,stopSweep); sweepPanel.append(sweepActions);
  const sweepProgress = el('progress','task-progress'); sweepProgress.max = 100; sweepProgress.setAttribute('aria-label',tr('Signal quality sweep progress')); sweepPanel.append(sweepProgress);
  const sweepTable = table(['RADIO','BAND','CHANNEL','PCI','RSRP','RSRQ','SINR'],tr('Signal sweep results')); sweepPanel.append(sweepTable.wrap); const sweepCaption = caption(sweepPanel), sweepFeedback = UI.status(sweepPanel);
  function renderSweep(data,fresh = true) {
    const progress = data?.progress?.ok ? data.progress.data : null, value = num(progress?.progress);
    const phase = fresh ? sweepJob.observe(value) : sweepJob.tick();
    const running = bool(progress?.running) || sweepJob.pending;
    startSweep.disabled = !API.connected || running; stopSweep.disabled = !API.connected || !running;
    if (value === null || phase === 'waiting') sweepProgress.removeAttribute('value'); else sweepProgress.value = Math.min(100,Math.max(0,value));
    if (sweepJob.started !== null) {
      if (phase === 'complete') say(sweepFeedback,tr('The modem reports the sweep is complete. Results below are the latest recorder output.'));
      else if (phase === 'timeout') say(sweepFeedback,tr('The sweep has not completed within three minutes. Check the modem state; it may still be scanning.'),true);
      else if (!stopRequested) say(sweepFeedback,tr("Signal sweep in progress{0}", value !== null && phase !== 'waiting' ? ` · ${value}%` : '…'));
    }
    if (stopRequested && progress && !bool(progress.running)) { stopRequested = false; sweepJob.clear(); say(sweepFeedback,tr('The modem reports the sweep is stopped.')); }
    if (data?.results?.ok) rows(sweepTable.body,data.results.data?.record_list || [],[item=>item.type,item=>item.band,item=>item.earfcn,item=>item.pci,item=>item.rsrp,item=>item.rsrq,item=>item.sinr],tr('No quality records reported.'),{key:item=>JSON.stringify([item.type,item.band,item.earfcn,item.pci]),animate:[4,5,6]});
    stamp(sweepCaption,'sweep',!data?.progress?.ok || !data?.results?.ok);
  }

  const speedPanel = UI.panel('tools/speedtest','diagnostic-speedtest',tr('Router internet speed'),tr('Measure the router-to-internet link. This consumes mobile data and can affect other connected devices.'));
  const serverLabel = el('label','',tr('Test server')), serverSelect = el('select'); serverSelect.id = 'speedtest-server'; serverLabel.htmlFor = serverSelect.id; serverSelect.disabled = true; serverSelect.add(new Option(tr('Load available test servers'),''));
  speedPanel.append(serverLabel,serverSelect);
  const speedActions = el('div','form-actions diagnostic-actions'), loadServers = button(tr('Load test servers'),async()=>{
    if (loadingServers || API.locked) return; loadingServers = true; const epoch = UI.epoch; loadServers.disabled = true;
    try { const servers = await request('/api/speedtest/servers',{timeoutMs:20000}); if (!UI.current(epoch)) return; if (!Array.isArray(servers)) throw new Error(tr('The router returned no server list.')); serverSelect.replaceChildren(); for (const server of servers) if (Number.isSafeInteger(Number(server.id)) && Number(server.id)>0) serverSelect.add(new Option(`${server.sponsor} · ${server.name}, ${server.country}`,String(server.id))); serversLoaded = !!serverSelect.options.length; if (!serversLoaded) serverSelect.add(new Option(tr('No servers available'),'')); serverSelect.disabled = !serversLoaded; say(speedFeedback,serversLoaded ? tr('Test servers loaded. Choose a server before starting.') : tr('No test servers available.'),!serversLoaded); renderSpeed(latestSpeed,false); }
    catch (error) { if (UI.current(epoch)) say(speedFeedback,tr("Could not load test servers. {0}", error.message),true); }
    finally { loadingServers = false; if (UI.current(epoch)) loadServers.disabled = false; }
  }), startSpeed = button(tr('Start speed test'),async()=>{
    const id = Number(serverSelect.value); if (!Number.isSafeInteger(id) || id<=0) return;
    if (!await confirm(tr('Start internet speed test?'),tr("Test using {0}. This can use a large amount of mobile data and reduce speeds for other users.", serverSelect.selectedOptions[0]?.textContent),{label:tr('Start test')})) return;
    await mutate(speedFeedback,async current=>{ await request('/api/speedtest/start',{...json('POST',{server_id:id}),timeoutMs:20000}); if (!current()) return; startingSpeed = Date.now(); say(speedFeedback,tr('Speed test requested. Live progress will arrive automatically.')); renderSpeed(latestSpeed,true); });
  },'primary'), stopSpeed = button(tr('Stop speed test'),async()=>{
    await mutate(speedFeedback,async current=>{ await request('/api/speedtest/stop',json('POST',{})); if (current()) { if (['cancelled','complete','error'].includes(latestSpeed?.phase)) renderSpeed(latestSpeed,true); else say(speedFeedback,tr('Stop requested. Waiting for the test worker to finish its current request.')); } });
  });
  speedActions.append(loadServers,startSpeed,stopSpeed); speedPanel.append(speedActions);
  const speedProgress = el('progress','task-progress'); speedProgress.max = 100; speedProgress.value = 0; speedProgress.setAttribute('aria-label',tr('Internet speed test progress')); speedPanel.append(speedProgress);
  const speedStats = stats(speedPanel,[['phase',tr('Test phase')],['live',tr('Current speed')],['download',tr('Download')],['upload',tr('Upload')],['ping',tr('Latency')],['jitter',tr('Jitter')],['received',tr('Test download data')],['sent',tr('Test upload data')]]); const speedCaption = caption(speedPanel), speedFeedback = UI.status(speedPanel);
  function renderSpeed(data,fresh = true) {
    const active = ['latency','download','upload'].includes(data?.phase) || data?.running === true;
    if (fresh && active) startingSpeed = 0;
    if (fresh && startingSpeed && Date.now()-startingSpeed >= 6000 && ['complete','cancelled','error'].includes(data?.phase)) startingSpeed = 0;
    const waiting = !!startingSpeed;
    const running = active || !!startingSpeed;
    startSpeed.disabled = !API.connected || !serversLoaded || running; stopSpeed.disabled = !API.connected || !running; serverSelect.disabled = !serversLoaded || running;
    if (waiting) { speedProgress.removeAttribute('value'); speedStats.phase.textContent = tr('Starting'); return; }
    speedProgress.value = Math.min(100,Math.max(0,Number(data?.progress)||0)); speedStats.phase.textContent = data?.phase || tr('Not reported');
    numeric(speedStats.live,number(data?.live_speed_mbps,tr(' Mbps'),2)); numeric(speedStats.download,number(data?.download_mbps,tr(' Mbps'),2)); numeric(speedStats.upload,number(data?.upload_mbps,tr(' Mbps'),2)); numeric(speedStats.ping,number(data?.ping_ms,' ms',2)); numeric(speedStats.jitter,number(data?.jitter_ms,' ms',2));
    numeric(speedStats.received,number(num(data?.download_bytes) === null ? null : data.download_bytes/1048576,tr(' MiB'),2)); numeric(speedStats.sent,number(num(data?.upload_bytes) === null ? null : data.upload_bytes/1048576,tr(' MiB'),2));
    if (data?.phase === 'error') say(speedFeedback,data.error || tr('The speed test failed.'),true);
    else if (data?.phase === 'complete') say(speedFeedback,tr('Internet speed test complete. These results describe the router’s internet link.'));
    else if (data?.phase === 'cancelled') say(speedFeedback,tr('Speed test stopped. Partial measurements are retained.'));
    stamp(speedCaption,'speedtest');
  }

  const terminal = UI.panel('tools/terminal','diagnostic-terminal',tr('AT terminal'),tr('Commands go directly to the modem. Some commands change radio settings, restart the modem or erase configuration. Review each command before sending.'));
  const port = el('p','reading-caption',tr('Waiting for modem port information.')), terminalForm = el('form','settings-form'), terminalGrid = el('div','form-grid');
  const commandField = el('div','form-field'), commandLabel = el('label','',tr('Modem command')), command = el('input'); command.id = 'at-command'; commandLabel.htmlFor = command.id; command.placeholder = 'AT+CSQ'; command.maxLength = 512; command.autocomplete = 'off'; command.spellcheck = false; command.setAttribute('autocapitalize','off'); command.required = true; commandField.append(commandLabel,command);
  const timeoutField = el('div','form-field'), timeoutLabel = el('label','',tr('Timeout (seconds)')), timeout = el('input'); timeout.id = 'at-timeout'; timeoutLabel.htmlFor = timeout.id; timeout.type = 'number'; timeout.min = '1'; timeout.max = '30'; timeout.value = '3'; timeout.required = true; timeoutField.append(timeoutLabel,timeout);
  terminalGrid.append(commandField,timeoutField); terminalForm.append(terminalGrid);
  const terminalActions = el('div','form-actions diagnostic-actions'), send = button(tr('Send command'),null,'primary'); send.type = 'submit'; const clearHistory = button(tr('Clear terminal history'),()=>{ history = []; drawHistory(); say(terminalFeedback,tr('This tab’s terminal history was cleared.')); }); terminalActions.append(send,clearHistory); terminalForm.append(terminalActions); terminal.append(port,terminalForm);
  const presets = el('div','form-actions diagnostic-actions'); for (const [label,value] of [[tr('Signal query'),'AT+CSQ'],[tr('Registration query'),'AT+CEREG?'],[tr('Device identity'),'ATI']]) presets.append(button(label,()=>{ command.value = value; command.focus(); terminalDirty = true; })); terminal.append(presets);
  const terminalFeedback = UI.status(terminal), terminalHistory = el('div','terminal-history'); terminal.append(terminalHistory);
  terminalForm.addEventListener('input',()=>{ terminalDirty = !!command.value; });
  terminalForm.addEventListener('submit',async event=>{
    event.preventDefault();
    try {
      const body = A.atCommand(command.value,timeout.value);
      if (!await confirm(tr('Send this modem command?'),tr("{0}\n\nThe command runs directly on the modem. Commands that write settings can interrupt service or change persistent configuration.", body.command),{label:tr('Send command')})) return;
      await mutate(terminalFeedback,async current=>{
        const data = await request('/api/at/send',{...json('POST',body),timeoutMs:(body.timeout+5)*1000}); if (!current()) return;
        history.unshift({command:body.command,response:String(data.response || '').slice(0,32768),truncated:String(data.response || '').length>32768,port:data.port,elapsed_ms:data.elapsed_ms,received_at:new Date().toISOString()}); history = history.slice(0,100); command.value = ''; terminalDirty = false; drawHistory(); say(terminalFeedback,tr('Modem response received. Inspect the response for OK, ERROR or command-specific results.'));
      });
    } catch (error) { say(terminalFeedback,error.message,true); }
  });
  function drawHistory() {
    terminalHistory.replaceChildren(); if (!history.length) terminalHistory.append(el('p','carrier-empty',tr('No commands sent in this tab. History clears when the console locks.')));
    for (const entry of history) { const card = el('article','terminal-entry'), heading = el('div','message-card-heading'); heading.append(el('strong','mono',entry.command),el('small','',tr("{0} · {1} ms", entry.port || tr('Modem'), entry.elapsed_ms ?? '—'))); card.append(heading,el('pre','terminal-output',entry.response || tr('(empty response)'))); if (entry.truncated) card.append(el('p','field-help',tr('Response shortened to 32,768 characters.'))); terminalHistory.append(card); }
  }
  function renderPort(data) { const available = data?.available === true; send.disabled = !API.connected || !available; port.textContent = available ? tr("Modem port · {0}", data.port || tr('Available')) : tr('No modem AT port is currently available.'); }

  const handlers = {resources:renderResources,neighbors:renderNeighbors,sweep:data=>{ latestSweep = data; renderSweep(data); },speedtest:data=>{ latestSpeed = data; renderSpeed(data); },at_port:renderPort};
  window.addEventListener('dashboard:readings',event=>{
    for (const [name,render] of Object.entries(handlers)) {
      const reading = event.detail.readings[name]; if (!reading) continue;
      if (reading.ok) {
        const data = structuredClone(reading.data), previous = samples.get(name)?.data;
        if (previous && ['resources','neighbors','sweep'].includes(name)) for (const [key,value] of Object.entries(data)) if (value?.ok === false && previous[key]?.data !== undefined) data[key] = {...value,data:previous[key].data,retained:true};
        if (name !== 'at_port') samples.set(name,{data,receivedAt:Date.now(),unavailable:['resources','neighbors','sweep'].includes(name) && Object.values(data).some(value=>value?.ok === false)}); render(data);
      }
      else { const target = {resources:healthCaption,neighbors:neighborCaption,sweep:sweepCaption,speedtest:speedCaption,at_port:port}[name]; target.textContent = tr('Reading unavailable. Last received values are retained where available.'); target.dataset.stale = 'true'; if (name === 'at_port') send.disabled = true; }
    }
  });
  window.addEventListener('dashboard:connection',()=>{
    for (const [node,name] of [[healthCaption,'resources'],[neighborCaption,'neighbors'],[sweepCaption,'sweep'],[speedCaption,'speedtest']]) stamp(node,name);
    if (!API.connected) { send.disabled = startSpeed.disabled = stopSpeed.disabled = startSweep.disabled = stopSweep.disabled = scanNeighbors.disabled = true; }
    else { scanNeighbors.disabled = false; renderPort(API.data('at_port')); }
  });
  const timer = setInterval(()=>{ if (!document.hidden && OpenU60Navigation.current.key === 'tools/radio' && sweepJob.pending) renderSweep(latestSweep,false); },1000);
  window.addEventListener('pagehide',event=>{ if (!event.persisted) clearInterval(timer); });
  function clear() {
    samples.clear(); history = []; latestSweep = latestSpeed = null; sweepJob.clear(); stopRequested = false; startingSpeed = 0; serversLoaded = false; terminalDirty = false; command.value = ''; timeout.value = '3';
    serverSelect.replaceChildren(new Option(tr('Load available test servers'),'')); serverSelect.disabled = true;
    for (const node of Object.values(healthStats)) numeric(node,'—'); cores.textContent = ''; sensors.replaceChildren(); interfaceOutput.textContent = tr('Unlock the console to read interfaces.');
    for (const [key,node] of Object.entries(speedStats)) { if (key === 'phase') node.textContent = '—'; else numeric(node,'—'); } speedProgress.value = 0; sweepProgress.removeAttribute('value'); includeHistory.checked = false;
    processQuery = ''; search.value = ''; drawProcesses(); rows(neighborTable.body,[],[()=>'',()=>'',()=>'',()=>'',()=>''],tr('Unlock the console to read neighboring cells.')); rows(sweepTable.body,[],[()=>'',()=>'',()=>'',()=>'',()=>'',()=>'',()=>''],tr('Unlock the console to read sweep results.'));
    for (const node of [reportFeedback,neighborFeedback,sweepFeedback,speedFeedback,terminalFeedback]) node.textContent = '';
    drawHistory(); renderPort(null); startSpeed.disabled = stopSpeed.disabled = startSweep.disabled = stopSweep.disabled = scanNeighbors.disabled = true;
  }
  UI.track({route:'tools/terminal',loaded:true,load:()=>{},get dirty(){return terminalDirty;},clear});
  drawHistory(); drawProcesses(); renderPort(API.data('at_port')); startSpeed.disabled = stopSpeed.disabled = stopSweep.disabled = true;
})();
