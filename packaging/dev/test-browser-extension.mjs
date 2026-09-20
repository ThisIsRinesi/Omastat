import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
const event = () => ({ listeners: [], addListener(fn) { this.listeners.push(fn) } });
let now = 1000000;
let focused = true;
let url = 'https://www.example.com/private/path';
let fail = false;
let audible = [];
const domains = () => sent.at(-1).audible_domains;
let tick;
const sent = [];
const browser = {
  tabs: { onActivated: event(), onUpdated: event(), onRemoved: event(), query: async (query) => query.audible ? audible : [{url}] },
  windows: { onFocusChanged: event(), onRemoved: event(), getLastFocused: async () => ({id: 1, focused}) },
  runtime: { onStartup: event(), onInstalled: event(), sendNativeMessage: async (_host, data) => {
    if (fail) throw new Error('disconnected');
    sent.push(data); return {ok:true};
  } }
};
vm.runInNewContext(readFileSync(new URL('../browser-extension/domain-tracker/background.js',import.meta.url),'utf8'),{
  browser, URL, Promise, Date: {now:()=>now}, setInterval(fn,ms) { assert.equal(ms,30000); tick=fn; }
});
const flush = async () => { for(let i=0;i<12;i++) await Promise.resolve(); };
await flush();
assert.equal(sent.at(-1).domain,'example.com');
assert.ok(!JSON.stringify(sent).includes('/private/path'));
now+=30000;tick();await flush();
assert.equal(sent.length,2,'heartbeats refresh an unchanged domain');
url='about:blank';browser.tabs.onUpdated.listeners[0](1,{url}, {active:true});await flush();
assert.equal(sent.at(-1).type,'clear-domain');
url='https://other.example';focused=false;browser.windows.onFocusChanged.listeners[0](-1);await flush();
assert.notEqual(sent.at(-1).domain,'other.example','unfocused browser never supplies attribution');
focused=true;now+=30000;fail=true;tick();await flush();
fail=false;tick();await flush();
assert.equal(sent.at(-1).domain,'other.example','failed delivery is retried without cached success');
url='file:///tmp/private';now+=30000;tick();await flush();
assert.equal(sent.at(-1).type,'clear-domain');
console.log('Browser attribution event checks passed');

focused=false;
audible=[{url:'https://www.youtube.com/watch?v=private',audible:true,mutedInfo:{muted:false}},
{url:'https://muted.example',audible:true,mutedInfo:{muted:true}},
{url:'https://private.example',audible:true,incognito:true}];
now+=30000;tick();await flush();
assert.deepEqual(Array.from(domains()),['youtube.com']);
assert.equal(sent.at(-1).type,'clear-domain');
assert.ok(!JSON.stringify(sent).includes('watch?v='));
audible=[];browser.tabs.onUpdated.listeners[0](1,{audible:false});await flush();
assert.equal(domains().length,0,'pause clears background domains without waiting for heartbeat');
console.log('Background audio domain checks passed');
