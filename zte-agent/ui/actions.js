'use strict';
(() => {
  const tr = globalThis.OpenU60I18n?.t || ((s,...v)=>s.replace(/\{(\d+)\}/g,(match,i)=>Number(i)<v.length?String(v[i]):match));
  const bytes = value => new TextEncoder().encode(value).length;
  const controls = /[\u0000-\u001f\u007f-\u009f]/;
  function wifiText(value,label,max = 32) {
    if (!value || bytes(value) > max) throw new Error(tr("{0} must contain 1–{1} UTF-8 bytes.", label, max));
    if (controls.test(value)) throw new Error(`${label}: control characters cannot be stored.`);
  }
  function wifiPassword(value) {
    if (!/^[\x20-\x7e]*$/.test(value)) throw new Error(tr('The router cannot store one or more characters in this Wi-Fi password. Choose a different password.'));
    if (!((value.length >= 8 && bytes(value) <= 63) || /^[0-9a-f]{64}$/i.test(value))) throw new Error(tr('Use an 8–63 byte Wi-Fi password, or a 64-digit hexadecimal key.'));
  }
  function encodeSMS(value) {
    let body = '';
    for (let index = 0; index < value.length; index++) body += value.charCodeAt(index).toString(16).padStart(4,'0').toUpperCase();
    return body;
  }
  function smsBody(number,message,date = new Date()) {
    const recipient = number.trim().replace(/[ ()-]/g,'');
    if (!/^\+?\d{1,20}$/.test(recipient)) throw new Error(tr('Enter a valid phone number, including the country code when needed.'));
    if (!message.trim()) throw new Error(tr('Enter a message.'));
    if (message.length > 1530) throw new Error(tr('Keep the message within 1,530 characters.'));
    const pad = value => String(value).padStart(2,'0');
    const offset = -date.getTimezoneOffset()/60;
    const timestamp = [pad(date.getFullYear()%100),pad(date.getMonth()+1),pad(date.getDate()),pad(date.getHours()),pad(date.getMinutes()),pad(date.getSeconds()),`${offset >= 0 ? '+' : ''}${offset}`].join(';');
    const gsm = '@£$¥èéùìòÇ\nØø\rÅåΔ_ΦΓΛΩΠΨΣΘΞÆæßÉ !"#¤%&\'()*+,-./0123456789:;<=>?¡ABCDEFGHIJKLMNOPQRSTUVWXYZÄÖÑÜ§¿abcdefghijklmnopqrstuvwxyzäöñüà^{}\\[~]|€';
    return {number:recipient,sms_time:timestamp,message_body:encodeSMS(message),id:'-1',encode_type:[...message].every(char => gsm.includes(char)) ? 'GSM7_default' : 'UNICODE'};
  }
  function ids(values) {
    if (!values.length || values.some(value => !/^\d+$/.test(String(value)))) throw new Error(tr('No valid messages selected.'));
    return [...new Set(values.map(String))].join(';')+';';
  }
  function bands(value,maximum) {
    const list = value.split(/[\s,;]+/).filter(Boolean).map(part => Number(part.replace(/^[bn]/i,'')));
    if (!list.length || list.some(band => !Number.isInteger(band) || band < 1 || band > maximum)) throw new Error(tr("Enter band numbers between 1 and {0}, separated by commas.", maximum));
    return [...new Set(list)].sort((a,b)=>a-b).join(',');
  }
  const api = {wifiText,wifiPassword,smsBody,encodeSMS,ids,bands};
  if (typeof module === 'object' && module.exports) module.exports = api; else globalThis.OpenU60Actions = api;
})();
