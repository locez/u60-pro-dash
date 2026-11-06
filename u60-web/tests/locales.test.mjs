import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
const messages=JSON.parse(await readFile(new URL('../src/messages.json',import.meta.url),'utf8'));
test('product messages cover five languages and preserve interpolated arguments',()=>{
  const languages=['en','ja','ko','zh-Hans','zh-Hant'];
  const args=text=>[...text.matchAll(/\{\d+\}/g)].map(x=>x[0]).sort();
  for(const [key,values] of Object.entries(messages)){
    assert.deepEqual(Object.keys(values).sort(),languages,key);
    for(const [language,text] of Object.entries(values)){
      assert.equal(typeof text,'string',key+'/'+language);
      assert(text.length>0,key+'/'+language);
      assert.deepEqual(args(text),args(key),key+'/'+language);
    }
  }
  for(const key of ['SIM security','Exit node','Settings changed elsewhere. Review the current values before trying again.']){
    assert(messages[key],key);
    for(const language of languages.filter(k=>k!=='en')) assert.notEqual(messages[key][language],key);
  }
});
