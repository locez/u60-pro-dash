'use strict';
(() => {
  const tr = globalThis.OpenU60I18n?.t || ((s,...v)=>s.replace(/\{(\d+)\}/g,(match,i)=>Number(i)<v.length?String(v[i]):match));
  const UI = OpenU60UI, API = OpenU60Console, A = OpenU60Advanced;
  const {el,button,say,json,request,confirm,mutate} = UI;
  const section = UI.panel('network/operators','operator-scan',tr('Available mobile networks'),tr('Scan from the router’s SIM, then choose a network. Scanning may interrupt mobile data for up to a few minutes.'));
  const actions = el('div','form-actions'), scan = button(tr('Scan networks'),startScan,'primary'), automatic = button(tr('Use automatic selection'),useAutomatic);
  actions.append(scan,automatic); section.append(actions);
  const feedback = UI.status(section), state = el('p','field-help',tr('Waiting for the modem’s scan status.')), list = el('div','operator-list');
  section.append(state,list);
  const scanJob = new A.Job('scan',120000), registerJob = new A.Job('registration',60000);
  let latest, selectedName = '', autoPending = false;
  function connected() { return API.connected && !API.locked; }
  function update(fresh = false) {
    const data = latest;
    const status = data?.status?.ok ? data.status.data?.m_netselect_status : undefined;
    const result = data?.registration?.ok ? data.registration.data?.m_netselect_result : undefined;
    const phase = fresh ? scanJob.observe(status) : scanJob.tick(), registration = registerJob.started !== null ? fresh ? registerJob.observe(result) : registerJob.tick() : 'idle';
    const scanning = ['running','waiting'].includes(phase) || scanJob.pending;
    scan.disabled = !connected() || scanning || registerJob.pending;
    automatic.disabled = !connected() || registerJob.pending;
    state.textContent = !data ? tr('Waiting for scan readings…') : data.status?.ok ? tr("Modem scan status: {0} · {1}", status ?? tr('not reported'), data.sampled_at ? new Date(Number(data.sampled_at)).toLocaleTimeString(globalThis.OpenU60I18n?.locale) : tr('live stream')) : tr('Scan status unavailable. Previously reported networks are retained.');
    if (registerJob.started !== null) {
      if (registration === 'complete') say(feedback,tr("The modem reports successful registration for {0}. Check Mobile Network for the resulting data connection.", selectedName));
      else if (registration === 'failed') say(feedback,tr("The modem reports registration failed for {0}. Try automatic selection or another allowed network.", selectedName),true);
      else if (registration === 'timeout') say(feedback,tr('Registration has not been confirmed within a minute. The modem may still be working; check its state before retrying.'),true);
      else say(feedback,tr("Waiting for registration to {0}…", selectedName));
    } else if (scanJob.started !== null) {
      if (phase === 'complete') say(feedback,tr('The modem reports the scan is complete. Select an available network below. Results can include cached operator information.'));
      else if (phase === 'failed') say(feedback,tr('The modem reports that the scan failed. Check the SIM and radio state before trying again.'),true);
      else if (phase === 'timeout') say(feedback,tr('The scan has not been confirmed within two minutes. Scanning may still continue on the router.'),true);
      else say(feedback,tr('Scanning mobile networks… Results arrive automatically.'));
    }
    if (data?.results?.ok) draw(A.operators(data.results.data),scanning);
  }
  function draw(rows,scanning) {
    list.replaceChildren();
    if (!rows.length) { list.append(el('p','carrier-empty',scanning ? tr('Waiting for scan results…') : tr('No networks reported. Start a scan to look for operators.'))); return; }
    for (const operator of rows) {
      const card = el('article','operator-card'), copy = el('div'), heading = el('div','title-count');
      heading.append(el('h3','',operator.name),el('span','tag',operator.status));
      copy.append(heading,el('p','mono',`${operator.plmn || tr('PLMN unavailable')} · ${operator.rat || tr('Access type unavailable')}`));
      const choose = button(tr('Register'),()=>register(operator));
      choose.disabled = !connected() || scanning || registerJob.pending || !operator.selectable;
      if (!operator.selectable) choose.title = tr('The operator is forbidden or has no usable network identifier.');
      card.append(copy,choose); list.append(card);
    }
  }
  async function startScan() {
    if (!await confirm(tr('Scan mobile networks?'),tr('The router will search available operators. Mobile data may disconnect during the scan. The scan continues if you leave this page.'),{label:tr('Start scan')})) return;
    await mutate(feedback,async current=>{
      await request('/api/modem/scan',json('POST',{})); if (!current()) return;
      registerJob.clear(); scanJob.start(); autoPending = false; list.replaceChildren(el('p','carrier-empty',tr('Waiting for new scan results…'))); update(true);
    });
  }
  async function register(operator) {
    if (!operator.selectable || !await confirm(tr('Register with this network?'),tr("Select {0} ({1}, {2}). This can disconnect mobile data. Roaming charges may apply; your SIM must allow access.", operator.name, operator.plmn, operator.rat),{label:tr('Register network')})) return;
    await mutate(feedback,async current=>{
      await request('/api/modem/register',json('POST',{m_mcc_mnc:operator.plmn,m_rat:operator.rat})); if (!current()) return;
      scanJob.clear(); registerJob.start(); selectedName = operator.name; autoPending = false; update(true);
    });
  }
  async function useAutomatic() {
    if (!await confirm(tr('Use automatic network selection?'),tr('Let the SIM and modem choose the mobile operator again. Mobile data may reconnect.'),{label:tr('Use automatic')})) return;
    await mutate(feedback,async current=>{
      await request('/api/modem/network-mode',json('PUT',{net_select_mode:'auto_select'})); if (!current()) return;
      registerJob.clear(); scanJob.clear(); autoPending = API.data('signal')?.net_select_mode !== 'auto_select'; say(feedback,autoPending ? tr('Automatic selection requested. Waiting for the modem to confirm it…') : tr('Automatic network selection confirmed by the modem.'));
    });
  }
  window.addEventListener('dashboard:readings',event=>{
    if (event.detail.readings.operators?.ok) { latest = event.detail.readings.operators.data; update(true); }
    if (autoPending && event.detail.readings.signal?.ok && event.detail.readings.signal.data?.net_select_mode === 'auto_select') { autoPending = false; say(feedback,tr('Automatic network selection confirmed by the modem.')); }
  });
  const timer = setInterval(()=>{ if (!document.hidden && OpenU60Navigation.current.key === 'network/operators' && (scanJob.pending || registerJob.pending)) update(); },1000);
  window.addEventListener('dashboard:navigate',()=>{ if (OpenU60Navigation.current.key === 'network/operators') update(); });
  UI.track({route:'network/operators',loaded:true,load:()=>{},get dirty(){return false;},clear:()=>{ latest = null; selectedName = ''; autoPending = false; scanJob.clear(); registerJob.clear(); state.textContent = tr('Unlock the console to read the modem status.'); list.replaceChildren(); feedback.textContent = ''; scan.disabled = automatic.disabled = true; }});
  window.addEventListener('pagehide',event=>{ if (!event.persisted) clearInterval(timer); });
  latest = API.data('operators'); update(true);
})();
