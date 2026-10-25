// Torrent background: thousands of streaks advected by a curl-ish flow field, drifting right->left (RTL reading direction)
(function(){
const c=document.getElementById('flow');if(!c)return;const x=c.getContext('2d');
const reduce=matchMedia('(prefers-reduced-motion: reduce)').matches;
let W,H,P=[],t=0;const dpr=Math.min(devicePixelRatio||1,2);
function size(){W=c.width=innerWidth*dpr;H=c.height=innerHeight*dpr;c.style.width=innerWidth+'px';c.style.height=innerHeight+'px';
 const n=Math.round(innerWidth*innerHeight/900);P=[];for(let i=0;i<n;i++)P.push(spawn(true));x.fillStyle='#070F2E';x.fillRect(0,0,W,H);}
function spawn(any){return{x:any?Math.random()*W:W+Math.random()*80,y:Math.random()*H,s:.6+Math.random()*1.8,l:0}}
function field(px,py){const k=.0016/dpr;const a=Math.sin(py*k*1.3+t*.6)*1.1+Math.cos(px*k*.7-t*.4)*.8+Math.sin((px+py)*k*.5+t*.3)*.6;
 // channel: stronger current through a band that bends across the page
 const band=H*(.55+.18*Math.sin(px*k*.6+t*.2));const d=Math.exp(-Math.pow((py-band)/(H*.22),2));
 return{vx:-(1.2+3.2*d),vy:a*.9*(1-d*.5),d}}
function step(){t+=.008;x.globalCompositeOperation='source-over';x.fillStyle='rgba(7,15,46,0.085)';x.fillRect(0,0,W,H);
 x.globalCompositeOperation='lighter';x.lineWidth=1*dpr;
 for(const p of P){const f=field(p.x,p.y);const nx=p.x+f.vx*p.s*dpr,ny=p.y+f.vy*p.s*dpr;
  const g=Math.round(120+f.d*90);x.strokeStyle=`rgba(0,${g},${150+f.d*40},${.10+f.d*.32})`;
  x.beginPath();x.moveTo(p.x,p.y);x.lineTo(nx,ny);x.stroke();p.x=nx;p.y=ny;
  if(p.x<-10||p.y<-10||p.y>H+10)Object.assign(p,spawn(false));}
 if(!reduce)requestAnimationFrame(step);}
addEventListener('resize',size);size();if(reduce){for(let i=0;i<120;i++)step();}else step();
})();
