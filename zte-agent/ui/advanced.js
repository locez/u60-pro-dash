'use strict';
(() => {
  const tr = globalThis.OpenU60I18n?.t || ((s,...v)=>s.replace(/\{(\d+)\}/g,(match,i)=>Number(i)<v.length?String(v[i]):match));
  const list = value => String(value || '').split(/[\n,]+/).map(item=>item.trim()).filter(Boolean);
  function httpURL(value,label = tr('Endpoint')) {
    let url;
    try { url = new URL(value); } catch { throw new Error(tr("{0} must be a complete HTTP or HTTPS address.", label)); }
    if (!['http:','https:'].includes(url.protocol) || !url.hostname || url.username || url.password) throw new Error(tr("{0} must use HTTP or HTTPS without a username or password in the address.", label));
    return value.trim();
  }
  function headers(value) {
    return String(value || '').split('\n').map(line=>line.trim()).filter(Boolean).map(line=>{
      const colon = line.indexOf(':');
      if (colon < 1) throw new Error(tr('Write each request header as Name: value, one per line.'));
      const name = line.slice(0,colon).trim(), value = line.slice(colon+1).trim();
      if (!/^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/.test(name) || /[\r\n\x00]/.test(value)) throw new Error(tr('A request header contains unsupported characters.'));
      return {name,value};
    });
  }
  function filter(values) {
    const type = values.filter, result = {type};
    if (!['all','sender','content','sender_and_content'].includes(type)) throw new Error(tr('Choose a message filter.'));
    if (['sender','sender_and_content'].includes(type)) {
      result.patterns = list(values.patterns);
      if (!result.patterns.length) throw new Error(tr('Enter at least one sender pattern.'));
      if (result.patterns.some(pattern=>pattern.includes('*') && !(pattern === '*' || /^\*[^*]+$|^[^*]+\*$/.test(pattern)))) throw new Error(tr('Use a single * at the beginning or end of a sender pattern.'));
    }
    if (['content','sender_and_content'].includes(type)) {
      result.keywords = list(values.keywords);
      if (!result.keywords.length) throw new Error(tr('Enter at least one message keyword.'));
    }
    return result;
  }
  function destination(values,saved) {
    const type = values.destination, previous = saved?.type === type ? saved : {};
    const required = (key,label,secret = false) => {
      const raw = String(values[key] || '').trim();
      const result = secret && !raw ? previous[key] : raw;
      if (!result) throw new Error(tr("Enter {0}.", label));
      return result;
    };
    if (type === 'telegram') {
      const bot_token = required('bot_token',tr('a Telegram bot token'),true), chat_id = required('chat_id',tr('a chat ID'));
      if (!/^\d+:[\w-]+$/.test(bot_token)) throw new Error(tr('Enter a Telegram bot token in number:token format.'));
      if (!/^-?\d+$|^@[A-Za-z0-9_]+$/.test(chat_id)) throw new Error(tr('Use a numeric chat ID or an @channel name.'));
      return {type,bot_token,chat_id,silent:values.silent === 'true'};
    }
    if (type === 'webhook') {
      const url = httpURL(required('url',tr('a webhook address'),true));
      if (!['POST','PUT'].includes(values.method)) throw new Error(tr('Choose POST or PUT for the webhook.'));
      return {type,url,method:values.method,headers:values.clear_headers ? [] : String(values.headers || '').trim() ? headers(values.headers) : previous.headers || []};
    }
    if (type === 'sms') {
      const forward_number = required('forward_number',tr('a forwarding phone number')).replace(/[ ()-]/g,'');
      if (!/^\+?\d{1,20}$/.test(forward_number)) throw new Error(tr('Enter a valid forwarding phone number.'));
      return {type,forward_number};
    }
    if (type === 'ntfy') {
      const url = httpURL(required('ntfy_url',tr('an ntfy server address'))), topic = required('topic',tr('an ntfy topic'));
      if (!/^[\w-]{1,128}$/.test(topic)) throw new Error(tr('Use letters, numbers, underscores or hyphens for the ntfy topic.'));
      const token = values.clear_token ? null : String(values.token || '').trim() || previous.token || null;
      return {type,url,topic,...(token ? {token} : {})};
    }
    if (['discord','slack'].includes(type)) return {type,webhook_url:httpURL(required('webhook_url',tr("a {0} webhook address", type),true))};
    throw new Error(tr('Choose a forwarding destination.'));
  }
  function rule(values,saved) {
    const name = values.name.trim(); if (!name || name.length > 100) throw new Error(tr('Use a rule name between 1 and 100 characters.'));
    return {...(saved ? {id:saved.id} : {}),name,enabled:values.enabled === 'true',filter:filter(values),destination:destination(values,saved?.destination)};
  }
  const canonical = value => JSON.stringify(value && typeof value === 'object' ? Array.isArray(value) ? value.map(item=>JSON.parse(canonical(item))) : Object.fromEntries(Object.keys(value).sort().map(key=>[key,JSON.parse(canonical(value[key]))])) : value);
  function operators(data) {
    const rows = Array.isArray(data?.m_netselect_contents) ? data.m_netselect_contents : [];
    const seen = new Set();
    return rows.flatMap(item=>{
      const plmn = String(item.mcc_mnc ?? item.plmn ?? ''), rat = String(item.rat ?? ''), key = `${plmn}/${rat}`;
      if (seen.has(key)) return []; seen.add(key);
      return [{name:String(item.name || item.operator_name || tr('Unnamed network')),plmn,rat,status:String(item.status || 'unreported').toLowerCase(),selectable:/^\d{5,6}$/.test(plmn) && !!rat && String(item.status).toLowerCase() !== 'forbidden'}];
    });
  }
  function atCommand(value,timeout) {
    const command = value.trim();
    if (!/^AT/i.test(command) || command.length > 512 || /[\x00-\x1f\x7f'`$;|&]/.test(command)) throw new Error(tr('Enter one AT command, up to 512 characters. The router cannot accept control characters, apostrophes, backticks, $, ;, | or &.'));
    if (!Number.isInteger(Number(timeout)) || Number(timeout) < 1 || Number(timeout) > 30) throw new Error(tr('Use a timeout between 1 and 30 seconds.'));
    return {command,timeout:Number(timeout)};
  }
  class Job {
    constructor(kind,limit = 90000) { this.kind = kind; this.limit = limit; this.clear(); }
    clear() { this.started = null; this.seenRunning = false; this.phase = 'idle'; }
    start(now = Date.now()) { this.started = now; this.seenRunning = false; this.phase = 'waiting'; }
    observe(value,now = Date.now()) {
      const raw = String(value ?? '').toLowerCase();
      let phase;
      if (this.kind === 'registration') phase = ['success','1'].includes(raw) ? 'complete' : ['fail','failed','0'].includes(raw) ? 'failed' : ['pending','running','registering'].includes(raw) ? 'running' : 'unknown';
      else if (this.kind === 'sweep') phase = Number.isFinite(Number(value)) && Number(value) >= 100 ? 'complete' : Number.isFinite(Number(value)) && Number(value) >= 0 && raw !== '' ? 'running' : 'unknown';
      else phase = ['done','complete','completed','2'].includes(raw) ? 'complete' : ['scanning','running','in_progress','1'].includes(raw) ? 'running' : ['error','fail','failed'].includes(raw) ? 'failed' : 'unknown';
      if (phase === 'running') this.seenRunning = true;
      if (this.started !== null) {
        if (now-this.started >= this.limit && !['complete','failed'].includes(phase)) phase = 'timeout';
        else if (['complete','failed'].includes(phase) && !this.seenRunning && now-this.started < 6000) phase = 'waiting';
      }
      this.phase = phase;
      return phase;
    }
    tick(now = Date.now()) { if (this.pending && now-this.started >= this.limit) this.phase = 'timeout'; return this.phase; }
    get pending() { return this.started !== null && ['waiting','running','unknown'].includes(this.phase); }
  }
  const api = {httpURL,headers,filter,destination,rule,canonical,operators,atCommand,Job};
  if (typeof module === 'object' && module.exports) module.exports = api; else globalThis.OpenU60Advanced = api;
})();
