// Academy-owned narrow host bridge. No document commands or arbitrary messages.
(()=>{
 const appId='video',origin=location.origin,parentFrame=parent,embedded=parentFrame!==window;
 let requested=null,announced=false,stopped=false;
 const parse=v=>v==='light'||v==='dark'?v:null;
 const send=m=>{if(embedded)parentFrame.postMessage(m,origin);};
 function request(theme){if(!parse(theme))return;requested=theme;apply();}
 function apply(){const b=window.wasmBindings;if(requested&&b?.academyReady?.()&&typeof b.academyTheme==='function'){b.academyTheme(requested==='light');document.documentElement.dataset.theme=requested;document.documentElement.style.colorScheme=requested;}}
 function tick(){if(stopped)return;const ready=window.wasmBindings?.academyReady?.();if(ready&&!announced){announced=true;apply();send({type:'aca:ready',version:1});}if(!announced)setTimeout(tick,100);}
 function receive(event){const m=event.data;if(event.origin!==origin||event.source!==parentFrame||!embedded||!m||m.version!==1||m.type!=='aca:theme'||!parse(m.theme))return;request(m.theme);}
 function stored(){if(embedded)return;try{const raw=localStorage.getItem('aca.theme.v1');let value;try{value=JSON.parse(raw);}catch{value=raw;}request(parse(value?.theme)||parse(value?.mode)||parse(value)||'dark');}catch{request('dark');}}
 addEventListener('message',receive);addEventListener('storage',e=>{if(e.key==='aca.theme.v1')stored();});
 const originalOpen=window.open.bind(window);
 window.open=function(url,...args){let u;try{u=new URL(url,location.href);}catch{return originalOpen(url,...args);}if(u.origin===origin&&u.pathname==='/2d/'+appId&&u.hash==='#tutorial'){if(embedded)send({type:'aca:tutorial',appId,version:1});else location.assign(u.href);return null;}return originalOpen(url,...args);};
 addEventListener('pagehide',()=>{stopped=true;removeEventListener('message',receive);},{once:true});
 stored();tick();
})();
