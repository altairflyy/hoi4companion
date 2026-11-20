/* Full-screen viewer for reference images (division templates, designs,
   advanced-guide illustrations). Presentation only: it reads the same local
   image URLs as the thumbnails and never changes the reference library. */
'use strict';
(function(root,factory){
 const api=factory();
 if(typeof module==='object'&&module.exports)module.exports=api;else root.HoiReferenceViewer=api;
})(typeof globalThis==='object'?globalThis:this,function(){
 const MIN=0.1,MAX=8,STEP=1.25;
 // Geometry is pure so it can be tested without a DOM.
 function fitScale(stage,image){
  if(!(image.width>0&&image.height>0&&stage.width>0&&stage.height>0))return 1;
  return Math.min(stage.width/image.width,stage.height/image.height,2);
 }
 function clampScale(scale){return Math.min(MAX,Math.max(MIN,scale));}
 function clampAxis(offset,stageSize,imageSize){
  if(imageSize<=stageSize)return (stageSize-imageSize)/2;
  return Math.min(0,Math.max(stageSize-imageSize,offset));
 }
 function clampView(view,stage,image){
  return {scale:view.scale,x:clampAxis(view.x,stage.width,image.width*view.scale),y:clampAxis(view.y,stage.height,image.height*view.scale)};
 }
 // Zooms around a stage point so the pixel under the cursor stays in place.
 function zoomAt(view,scale,point,stage,image){
  const next=clampScale(scale),ratio=next/view.scale;
  return clampView({scale:next,x:point.x-(point.x-view.x)*ratio,y:point.y-(point.y-view.y)*ratio},stage,image);
 }
 function fitView(stage,image){const scale=fitScale(stage,image);return clampView({scale,x:0,y:0},stage,image);}

 let dialog=null,parts=null,items=[],index=0,view={scale:1,x:0,y:0},fitted=true,drag=null,opener=null;
 const t=(text,...values)=>typeof HoiI18n==='object'?HoiI18n.t(text,...values):text.replace(/\{(\d+)\}/g,(m,i)=>String(values[i]??m));
 function el(tag,className,text){const n=document.createElement(tag);if(className)n.className=className;if(text!==undefined)n.textContent=text;return n;}
 function button(label,title,action){const b=el('button','viewer-button',label);b.type='button';b.title=title;b.setAttribute('aria-label',title);b.addEventListener('click',action);return b;}
 function stageSize(){const r=parts.stage.getBoundingClientRect();return {width:r.width,height:r.height};}
 function imageSize(){return {width:parts.image.naturalWidth,height:parts.image.naturalHeight};}
 function apply(){
  parts.image.style.transform=`translate(${view.x}px,${view.y}px) scale(${view.scale})`;
  parts.zoom.textContent=Math.round(view.scale*100)+'%';
  parts.stage.dataset.pannable=String(imageSize().width*view.scale>stageSize().width+1||imageSize().height*view.scale>stageSize().height+1);
 }
 function fit(){fitted=true;view=fitView(stageSize(),imageSize());apply();}
 function actual(){fitted=false;const s=stageSize();view=zoomAt(view,1,{x:s.width/2,y:s.height/2},s,imageSize());apply();}
 function zoomBy(factor,point){fitted=false;const s=stageSize();view=zoomAt(view,view.scale*factor,point||{x:s.width/2,y:s.height/2},s,imageSize());apply();}
 function show(i){
  if(!items.length)return;index=(i+items.length)%items.length;const item=items[index];
  parts.title.textContent=item.title||'';parts.count.textContent=items.length>1?t('{0} of {1}',index+1,items.length):'';
  parts.original.href=item.src;parts.prev.hidden=parts.next.hidden=items.length<2;
  parts.stage.dataset.loading='true';parts.image.alt=item.title||'';parts.image.style.transform='';
  parts.image.onload=()=>{parts.stage.dataset.loading='false';fit();};
  parts.image.onerror=()=>{parts.stage.dataset.loading='error';};
  parts.image.src=item.src;
 }
 function build(){
  dialog=el('dialog','reference-viewer');dialog.setAttribute('aria-label',t('Reference image viewer'));
  const bar=el('header','viewer-bar'),heading=el('div','viewer-heading'),title=el('strong','viewer-title'),count=el('span','viewer-count');heading.append(title,count);
  const zoom=el('span','viewer-zoom','100%');zoom.setAttribute('aria-live','polite');
  const original=el('a','viewer-button',t('Open original'));original.target='_blank';original.rel='noopener';
  const tools=el('div','viewer-tools');
  tools.append(button('−',t('Zoom out (−)'),()=>zoomBy(1/STEP)),zoom,button('+',t('Zoom in (+)'),()=>zoomBy(STEP)),
   button(t('Fit'),t('Fit to window (0)'),fit),button('1:1',t('Actual size (1)'),actual),original,button('✕',t('Close (Esc)'),close));
  bar.append(heading,tools);
  const stage=el('div','viewer-stage'),image=el('img','viewer-image');image.draggable=false;image.decoding='async';stage.append(image);
  const status=el('p','viewer-status',t('Image unavailable'));stage.append(status);
  const prev=button('‹',t('Previous image (←)'),()=>show(index-1)),next=button('›',t('Next image (→)'),()=>show(index+1));
  prev.classList.add('viewer-nav','prev');next.classList.add('viewer-nav','next');stage.append(prev,next);
  const hint=el('p','viewer-hint',t('Wheel or +/− to zoom, drag to move, double-click to switch between fit and actual size.'));
  dialog.append(bar,stage,hint);document.body.append(dialog);
  parts={title,count,zoom,original,stage,image,prev,next};
  stage.addEventListener('wheel',event=>{event.preventDefault();const r=stage.getBoundingClientRect();zoomBy(event.deltaY<0?STEP:1/STEP,{x:event.clientX-r.left,y:event.clientY-r.top});},{passive:false});
  stage.addEventListener('dblclick',event=>{if(event.target.closest('button'))return;const r=stage.getBoundingClientRect(),s=stageSize(),point={x:event.clientX-r.left,y:event.clientY-r.top};
   if(fitted&&Math.abs(view.scale-1)<0.01)view=zoomAt(view,2,point,s,imageSize());else if(fitted)view=zoomAt(view,1,point,s,imageSize());else{fit();return;}fitted=false;apply();});
  stage.addEventListener('pointerdown',event=>{if(event.button!==0||event.target.closest('button'))return;drag={id:event.pointerId,x:event.clientX,y:event.clientY,view:{...view}};stage.setPointerCapture(event.pointerId);stage.dataset.dragging='true';});
  stage.addEventListener('pointermove',event=>{if(!drag||drag.id!==event.pointerId)return;fitted=false;
   view=clampView({scale:view.scale,x:drag.view.x+event.clientX-drag.x,y:drag.view.y+event.clientY-drag.y},stageSize(),imageSize());apply();});
  const end=event=>{if(drag?.id===event.pointerId){drag=null;stage.dataset.dragging='false';}};
  stage.addEventListener('pointerup',end);stage.addEventListener('pointercancel',end);
  dialog.addEventListener('keydown',event=>{
   if(event.target.closest?.('input,select,textarea'))return;
   const key=event.key,actions={ArrowLeft:()=>show(index-1),ArrowRight:()=>show(index+1),'+':()=>zoomBy(STEP),'=':()=>zoomBy(STEP),'-':()=>zoomBy(1/STEP),'0':fit,'1':actual};
   if(actions[key]){event.preventDefault();actions[key]();}
  });
  dialog.addEventListener('close',()=>{parts.image.removeAttribute('src');opener?.focus?.();opener=null;});
  dialog.addEventListener('click',event=>{if(event.target===dialog)close();});
  root.addEventListener?.('resize',()=>{if(dialog.open&&fitted)fit();else if(dialog.open){view=clampView(view,stageSize(),imageSize());apply();}});
 }
 const root=typeof window==='object'?window:null;
 function open(list,start=0){
  items=(Array.isArray(list)?list:[]).filter(item=>item&&typeof item.src==='string');if(!items.length)return false;
  if(!dialog)build();opener=document.activeElement;
  if(!dialog.open)dialog.showModal();show(start);dialog.querySelector('.viewer-tools button:last-child').focus();return true;
 }
 function close(){if(dialog?.open)dialog.close();}
 return {open,close,fitScale,fitView,zoomAt,clampView};
});
