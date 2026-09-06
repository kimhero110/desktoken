const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');

function element() {
  return { style:{}, dataset:{}, classList:{add(){},remove(){},toggle(){},contains(){return false;}},
    textContent:'', innerHTML:'', appendChild(){}, setAttribute(){}, addEventListener(){},
    querySelectorAll(){return [];}, offsetHeight:100 };
}

test('actual page event renderer handles success, partial failure and complete failure', () => {
  const elements=new Map(), events=new Map();
  const get=id=> { if(!elements.has(id)) elements.set(id,element()); return elements.get(id); };
  const context=vm.createContext({
    console, navigator:{language:'zh-CN'},
    document:{getElementById:get,createElement:element,body:element()},
    localStorage:{getItem(){return null;},setItem(){}},setTimeout(){},setInterval(){},
    window:{addEventListener(){},__TAURI__:{core:{invoke:async()=>({opacity:0.7})},event:{listen:(name,fn)=>events.set(name,fn)}}}
  });
  const root=path.join(__dirname,'../src');
  vm.runInContext(fs.readFileSync(path.join(root,'quota-state.js'),'utf8'),context);
  const page=fs.readFileSync(path.join(root,'index.html'),'utf8');
  for(const match of page.matchAll(/<script>([\s\S]*?)<\/script>/g)) vm.runInContext(match[1],context);
  const emit=(id,pct,error=null)=>events.get('quota://snapshot')({payload:{
    provider_id:id,provider_name:id,fetched_at:Date.now()/1000,source:'official',error,
    windows:error?[]:[{label:'周',used_percent:pct}]
  }});
  emit('a',95);
  assert.equal(get('strip-worst').textContent,'95%');
  assert.equal(get('strip-dot').style.background,'#F85149');
  emit('b',0,'offline');
  assert.equal(get('strip-worst').textContent,'95% *');
  emit('a',0,'offline');
  assert.equal(get('strip-worst').textContent,'—');
  assert.equal(get('strip-dot').style.background,'#8B949E');
  emit('a',0);
  assert.equal(get('strip-worst').textContent,'0% *');
});

test('all bundled inline scripts parse', () => {
  for(const file of fs.readdirSync(path.join(__dirname,'../src')).filter(f=>f.endsWith('.html'))) {
    const html=fs.readFileSync(path.join(__dirname,'../src',file),'utf8');
    for(const match of html.matchAll(/<script>([\s\S]*?)<\/script>/g)) new vm.Script(match[1],{filename:file});
  }
});
