'use strict';
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const root=path.join(__dirname,'..'),localeSource=fs.readFileSync(path.join(root,'locales.js'),'utf8'),source=fs.readFileSync(path.join(root,'i18n.js'),'utf8');
function runtime(language,saved,storageThrows=false){const context={module:{exports:{}},navigator:{languages:[language]},localStorage:{getItem(){if(storageThrows)throw Error('blocked');return saved;}}};vm.createContext(context);vm.runInContext(localeSource,context);vm.runInContext(source,context);return context.module.exports;}
for(const [input,expected]of [['zh-CN','zh-Hans'],['zh-SG','zh-Hans'],['zh-TW','zh-Hant'],['zh-HK','zh-Hant'],['zh-Hant-MO','zh-Hant'],['en-GB','en'],['ja-JP','ja'],['ko-KR','ko'],['de-DE','en']])assert.equal(runtime(input).locale,expected);
assert.equal(runtime('zh-CN','en').locale,'en');assert.equal(runtime('ja-JP','bad').locale,'ja');assert.equal(runtime('ko-KR',null,true).locale,'ko');
for(const locale of ['zh-Hans','zh-Hant','en','ja','ko']){
 const i=runtime('en',locale),value='<img src=x onerror=alert(1)> Network {1}';
 assert.ok(i.t('Current: {0}',value).includes(value),'interpolation preserves raw values');
 assert.equal(i.t('Unregistered backend error'), 'Unregistered backend error');
 assert.ok(!i.t('Save changes').includes('{0}'));
}
assert.equal(runtime('en','zh-Hans').t('Network'),'网络');assert.equal(runtime('en','zh-Hant').t('Network'),'網路');assert.equal(runtime('en','ja').t('Network'),'ネットワーク');assert.equal(runtime('en','ko').t('Network'),'네트워크');
console.log('i18n: locale negotiation, storage fallback, translations and literal interpolation passed');
