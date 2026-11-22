'use strict';

// Presentation only. Each page supplies its own catalog geometry and target.
((root)=>{
 function create({viewport, world, sizer, slider, output, caption, targetLabel, hideTooltip}){
  const manualMinimum=Number(slider.min);
  let width=0,height=0,zoom=1,offsetX=0,offsetY=0,mode='overview',target=null;

  function apply(percent, left=0, top=0, right=0, bottom=0){
   zoom=percent/100;
   offsetX=left;
   offsetY=top;
   slider.value=percent;
   output.textContent=percent+'%';
   world.style.transform=`scale(${zoom})`;
   world.style.left=left+'px';
   world.style.top=top+'px';
   sizer.style.width=left+width*zoom+right+'px';
   sizer.style.height=top+height*zoom+bottom+'px';
   hideTooltip();
  }

  function center(node){
   if(!node)return;
   viewport.scrollTo(
    offsetX+(node.x+node.width/2)*zoom-viewport.clientWidth/2,
    offsetY+(node.y+node.height/2)*zoom-viewport.clientHeight/2
   );
  }

  function frame(node,percent){
   const w=viewport.clientWidth,h=viewport.clientHeight,scale=percent/100;
   const cx=(node.x+node.width/2)*scale,cy=(node.y+node.height/2)*scale;
   // Leave enough room to center nodes at either edge of the sheet.
   apply(percent,Math.max(0,w/2-cx),Math.max(0,h/2-cy),Math.max(0,w/2+cx-width*scale),Math.max(0,h/2+cy-height*scale));
   center(node);
  }

  function update(){
   const w=viewport.clientWidth,h=viewport.clientHeight;
   if(!width||!height||!w||!h)return;
   const fit=Math.max(1,Math.min(100,Math.floor(100*Math.min((w-16)/width,(h-16)/height))));
   slider.min=Math.min(manualMinimum,fit,Math.round(zoom*100));
   if(mode==='overview'){
    apply(fit);
    viewport.scrollTo(0,0);
   }else if(mode==='target'){
    const percent=Math.max(fit,Math.floor(Math.min(120,100*(w-24)/target.width,100*(h-24)/target.height)));
    frame(target,percent);
   }
  }

  function showTarget(node){
   if(!node)return;
   caption.textContent=targetLabel+' · '+node.name;
   caption.hidden=false;
   if(mode==='manual'){frame(node,Math.round(zoom*100));return;}
   mode='target';
   target=node;
   update();
  }

  function overview(){
   caption.hidden=true;
   if(mode==='manual')return;
   mode='overview';
   target=null;
   update();
  }

  function size(nextWidth,nextHeight){
   if(width===nextWidth&&height===nextHeight)return;
   width=nextWidth;
   height=nextHeight;
   world.style.width=width+'px';
   world.style.height=height+'px';
   if(mode==='manual')apply(Math.round(zoom*100));
   else update();
  }

  function manual(node){
   const percent=Number(slider.value);
   const centerPoint=node||{
    x:(viewport.scrollLeft+viewport.clientWidth/2-offsetX)/zoom,
    y:(viewport.scrollTop+viewport.clientHeight/2-offsetY)/zoom,
    width:0,height:0
   };
   const wasTarget=mode==='target'||!caption.hidden;
   mode='manual';
   if(wasTarget)frame(centerPoint,percent);
   else{apply(percent);center(centerPoint);}
  }

  if(typeof ResizeObserver!=='undefined')new ResizeObserver(update).observe(viewport);
  return {center,showTarget,overview,size,manual,fit(){mode='overview';overview();}};
 }

 if(typeof module==='object'&&module.exports)module.exports={create};
 else root.HoiTreeViewport={create};
})(globalThis);
