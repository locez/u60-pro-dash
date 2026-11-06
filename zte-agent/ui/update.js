'use strict';
(() => {
  const tr = globalThis.OpenU60I18n?.t || ((s,...v)=>s.replace(/\{(\d+)\}/g,(match,i)=>Number(i)<v.length?String(v[i]):match));
  const UI = globalThis.OpenU60UI;
  if (!UI) return;
  const $ = id => document.getElementById(id);
  const form = $('update-settings-form'), fields = $('update-settings-fields');
  const channel = $('update-channel'), autoCheck = $('update-auto-check'), autoDownload = $('update-auto-download');
  const save = $('update-save'), check = $('update-check'), download = $('update-download'), install = $('update-install'), rollback = $('update-rollback');
  const feedback = $('update-feedback');
  let status = null, busy = false, baseline = '';
  const bytes = value => {
    const n = Number(value);
    if (!Number.isFinite(n) || n < 0) return '—';
    const unit = n >= 1048576 ? 'MB' : n >= 1024 ? 'KB' : 'B';
    return `${(n / ({MB:1048576,KB:1024,B:1}[unit])).toFixed(unit === 'B' ? 0 : 1)} ${unit}`;
  };
  const title = value => String(value || 'idle').replaceAll('_',' ').replace(/^./,letter=>letter.toUpperCase());
  const currentSettings = () => JSON.stringify({channel:channel.value,automatic_checks:autoCheck.checked,automatic_downloads:autoDownload.checked});
  function setBusy(value) {
    busy = value; form.inert = value; check.disabled = value;
    render();
  }
  function render(next = status) {
    if (next) status = next;
    if (!status) return;
    const current = status.current || {}, progress = status.progress || {}, managed = status.deployment_mode === 'paired';
    $('update-current-version').textContent = current.version ? `v${current.version}` : tr('Unknown');
    $('update-current-commit').textContent = [current.commit,current.target].filter(Boolean).join(' · ') || tr('Build details unavailable');
    $('update-channel-state').textContent = status.channel === 'beta' ? tr('Beta') : tr('Stable');
    $('update-check-time').textContent = status.last_check ? tr("Last checked {0}", new Date(status.last_check * 1000).toLocaleString(globalThis.OpenU60I18n?.locale)) : tr('Not checked yet');
    $('update-phase').textContent = status.trial_in_progress ? tr('Health trial') : title(status.phase);
    $('update-guard').textContent = managed ? tr('Agent + screen deployment') : status.bootstrap_ready ? tr('A/B recovery ready') : tr('Bootstrap required');
    $('update-slot').textContent = managed ? tr('Managed by the project deployment script') : status.active_slot ? tr("Active slot {0} · agent-only restart", status.active_slot.toUpperCase()) : tr('Install once over SSH to enable OTA');
    const release = status.available, staged = status.staged;
    $('update-state-detail').textContent = (managed ? tr('Update both programs together with bash dash.sh install; use bash dash.sh rollback to restore the previous pair.') : status.last_error) || (!status.signing_configured ? tr('This development build has no release verification key.') : staged ? tr("v{0} is verified and staged.", staged.version) : release ? tr("v{0} is ready to download.", release.version) : tr('No newer signed release reported.'));
    $('update-release').hidden = !release;
    if (release) {
      $('update-available-version').textContent = `v${release.version}${staged?.version === release.version ? tr(' · STAGED') : ''}`;
      $('update-release-copy').textContent = release.channel === 'beta' ? tr('Signed Beta release. Review the notes before installing.') : tr('Signed Stable release available for this device.');
      $('update-notes').href = release.notes_url;
    }
    const total = Number(progress.total || 0), received = Number(progress.received || 0);
    const percent = total > 0 ? Math.min(100,Math.round(received / total * 100)) : 0;
    $('update-progress').hidden = status.phase !== 'downloading' && !(total && received);
    OpenU60Numbers.set($('update-percent'),percent);
    $('update-progress-bar').style.width = `${percent}%`;
    $('update-bytes').textContent = tr("{0} of {1} received and verified", bytes(received), bytes(total));
    if (!fields.matches(':focus-within') && !busy) {
      channel.value = status.settings?.channel || status.channel || 'stable';
      autoCheck.checked = status.settings?.automatic_checks !== false;
      autoDownload.checked = status.settings?.automatic_downloads === true;
      baseline = currentSettings();
    }
    fields.disabled = managed;
    save.disabled = managed || busy || currentSettings() === baseline;
    check.disabled = managed || busy || !status.signing_configured;
    download.disabled = managed || busy || !release || staged?.version === release.version;
    install.disabled = managed || busy || !staged || !status.bootstrap_ready || status.trial_in_progress;
    rollback.disabled = managed || busy || !status.bootstrap_ready || !status.rollback_available || status.trial_in_progress;
  }
  form.addEventListener('input',()=>{ save.disabled = busy || currentSettings() === baseline; });
  form.addEventListener('submit',async event => {
    event.preventDefault();
    if (channel.value === 'beta' && status?.settings?.channel !== 'beta' && !await UI.confirm(tr('Use the Beta channel?'),tr('Beta releases arrive earlier and may be less tested. Every release is still signed and protected by automatic rollback.'),{label:tr('Use Beta')})) return;
    await action(tr('Saving update preferences…'),tr('Preferences saved.'),()=>UI.request('/api/update/settings',UI.json('PUT',JSON.parse(currentSettings()))));
  });
  check.addEventListener('click',()=>action(tr('Checking signed release channels…'),tr('Signed release check complete.'),()=>UI.request('/api/update/check',{method:'POST',timeoutMs:360000})));
  download.addEventListener('click',()=>action(tr('Downloading and verifying the inactive slot…'),tr('Release downloaded, verified and staged.'),()=>UI.request('/api/update/download',{method:'POST',timeoutMs:360000})));
  install.addEventListener('click',async()=>{
    if (!await UI.confirm(tr('Install the staged agent?'),tr("Install v{0} and restart only the agent? The console reconnects after a 60-second health trial. Wi-Fi and cellular stay online.", status?.staged?.version || ''),{label:tr('Install agent')})) return;
    await action(tr('Handing off to the update supervisor…'),tr('Agent restart accepted. This console will reconnect automatically.'),()=>UI.request('/api/update/install',UI.json('POST',{confirm:true})));
  });
  rollback.addEventListener('click',async()=>{
    if (!await UI.confirm(tr('Return to the previous agent?'),tr('Restart the agent from its other verified slot. Wi-Fi and cellular stay online.'),{label:tr('Return to previous')})) return;
    await action(tr('Starting protected rollback…'),tr('Rollback accepted. This console will reconnect automatically.'),()=>UI.request('/api/update/rollback',UI.json('POST',{confirm:true})));
  });
  async function action(working,done,request) {
    if (busy) return;
    await UI.mutate(feedback,async current => {
      setBusy(true); UI.say(feedback,working);
      try { const result = await request(); if (!current()) return; render(result); baseline = currentSettings(); UI.say(feedback,done); }
      finally { setBusy(false); }
    });
  }
  window.addEventListener('dashboard:readings',event=>{
    const reading = event.detail.readings?.ota_status;
    if (reading?.ok) render(reading.data);
    else if (reading && !reading.ok) UI.say(feedback,tr('Live update status is unavailable.'),true);
  });
  window.addEventListener('dashboard:locked',()=>{
    status = null; baseline = ''; fields.disabled = true; save.disabled = check.disabled = download.disabled = install.disabled = rollback.disabled = true;
    UI.say(feedback,tr('Unlock the console to read update status.'));
  });
})();
