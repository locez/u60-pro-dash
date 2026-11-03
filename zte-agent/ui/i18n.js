'use strict';
(() => {
  const languages = Object.freeze({'zh-Hans':'简体中文','zh-Hant':'繁體中文',en:'English',ja:'日本語',ko:'한국어'});
  const key = 'openu60.language';
  function supportedLocale(value) {
    const code = String(value || '').toLowerCase().replace(/_/g, '-');
    if (/^zh(?:-|$)/.test(code)) {
      if (/(?:^|-)hans(?:-|$)/.test(code)) return 'zh-Hans';
      return /(?:^|-)(?:hant|tw|hk|mo)(?:-|$)/.test(code) ? 'zh-Hant' : 'zh-Hans';
    }
    if (/^ja(?:-|$)/.test(code)) return 'ja';
    if (/^ko(?:-|$)/.test(code)) return 'ko';
    if (/^en(?:-|$)/.test(code)) return 'en';
    return null;
  }
  function resolve(value) { return supportedLocale(value) || 'en'; }
  let saved;
  try { saved = globalThis.localStorage?.getItem(key); } catch { /* Private browser mode. */ }
  const preferred = globalThis.navigator?.languages?.length ? navigator.languages : [globalThis.navigator?.language];
  const locale = Object.hasOwn(languages, saved || '') ? saved : preferred.map(supportedLocale).find(Boolean) || 'en';
  const messages = globalThis.OpenU60Locales || {};
  function t(source, ...values) {
    const template = typeof source === 'string' ? source : source.reduce((out, part, i) => out + (i ? '{' + (i - 1) + '}' : '') + part, '');
    const translated = messages[template]?.[locale] || template;
    return translated.replace(/\{(\d+)\}/g, (match, index) => Number(index) < values.length ? String(values[index]) : match);
  }
  const api = {t, locale, languages, resolve, reloading:false};
  globalThis.OpenU60I18n = api;
  if (typeof module !== 'undefined') module.exports = api;
  if (!globalThis.document) return;
  document.documentElement.lang = locale;
  function initialize() {
    for (const node of document.querySelectorAll('[data-i18n]')) node.textContent = t(node.dataset.i18n);
    for (const attr of ['title','aria-label','placeholder']) {
      for (const node of document.querySelectorAll('[data-i18n-' + attr + ']')) node.setAttribute(attr, t(node.getAttribute('data-i18n-' + attr)));
    }
    for (const select of document.querySelectorAll('[data-language]')) {
      select.replaceChildren(...Object.entries(languages).map(([code,name]) => new Option(name,code)));
      select.value = locale;
      select.addEventListener('change', async () => {
        const next = select.value;
        select.value = locale;
        if (!Object.hasOwn(languages,next) || next === locale) return;
        const ui = globalThis.OpenU60UI;
        if (ui?.saving) { alert(t('Wait for the current operation before changing language.')); return; }
        if (ui?.dirty && !await ui.confirm(t('Change language?'),t('The page will reload. Unsaved edits will be discarded.'),{label:t('Change language')})) return;
        try { localStorage.setItem(key,next); } catch { alert(t('Your browser could not save the language preference.')); return; }
        api.reloading = true;
        location.reload();
      });
    }
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded',initialize,{once:true}); else initialize();
})();
