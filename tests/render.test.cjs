const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');

function element() {
  return { style:{}, dataset:{}, events:{}, children:[], classList:{add(){},remove(){},toggle(){},contains(){return false;}},
    textContent:'', innerHTML:'', appendChild(child){this.children.push(child);}, setAttribute(){}, addEventListener(name,fn){this.events[name]=fn;},
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
  vm.runInContext(fs.readFileSync(path.join(root,'task-state.js'),'utf8'),context);
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
  events.get('tasks://snapshot')({payload:[{tool:'codex',session_id:'session1',project:'demo',state:'waiting_approval',updated_at:Date.now()}]});
  assert.equal(get('strip-dot').style.background,'#D29922');
  assert.equal(get('strip-worst').textContent,'0% *'); // quota and task meanings stay separate
  get('task-tab').events.click();
  assert.equal(get('tasks').style.display,'block');
  assert.equal(get('providers').style.display,'none');
  assert.equal(get('detail').style.display,'none');
  assert.equal(get('tasks').children.at(-1).children[0].textContent,'codex · 等待授权');
  get('quota-tab').events.click();
  assert.equal(get('tasks').style.display,'none');
  assert.equal(get('providers').style.display,'');
});

test('all bundled inline scripts parse', () => {
  for(const file of fs.readdirSync(path.join(__dirname,'../src')).filter(f=>f.endsWith('.html'))) {
    const html=fs.readFileSync(path.join(__dirname,'../src',file),'utf8');
    for(const match of html.matchAll(/<script>([\s\S]*?)<\/script>/g)) new vm.Script(match[1],{filename:file});
  }
});
