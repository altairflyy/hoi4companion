'use strict';
// Pure presentation geometry. Coordinates are independent of live/guide state.
((root)=>{
 const finite=v=>typeof v==='number'&&Number.isFinite(v)&&Math.abs(v)<=65536;
 const point=p=>p&&finite(p.x)&&finite(p.y);
 const size=p=>p&&finite(p.width)&&finite(p.height)&&p.width>0&&p.height>0;
 function layout(tree){
  const v=tree.visuals||{},item=v.templates?.national_focus_item,spacing=v.positions?.focus_spacing;
  const native=v.status==='observed'&&point(spacing)&&spacing.x>0&&spacing.y>0&&size(item?.size);
  const width=native?item.size.width:174,height=native?item.size.height:154;
  const dx=native?spacing.x:100,dy=native?spacing.y:190,gap=74;
  const focuses=tree.focuses.filter(f=>finite(f.x)&&finite(f.y));
  const minX=Math.min(0,...focuses.map(f=>f.x)),minY=Math.min(0,...focuses.map(f=>f.y));
  const coords=new Map(focuses.map(f=>[f.id,{x:(f.x-minX)*dx+gap,y:(f.y-minY)*dy+gap}]));
  return {native,node:{width,height},coords,
   width:Math.max(500,...[...coords.values()].map(p=>p.x+width+gap)),
   height:Math.max(410,...[...coords.values()].map(p=>p.y+height+gap))};
 }
 function component(child,node){
  if(!child||!point(child.position))return null;
  const anchors={UPPER_LEFT:[0,0],UPPER_CENTER:[.5,0],UPPER_RIGHT:[1,0],
   CENTER_LEFT:[0,.5],CENTER:[.5,.5],CENTER_RIGHT:[1,.5],LOWER_LEFT:[0,1],LOWER_CENTER:[.5,1],LOWER_RIGHT:[1,1]};
  const anchor=anchors[child.orientation];if(!anchor)return null;
  const scale=finite(child.scale)&&child.scale>0&&child.scale<=10?child.scale:1;
  return {x:child.position.x+node.width*anchor[0],y:child.position.y+node.height*anchor[1],center:!!child.center,scale};
 }
 function edge(from,to,geometry,visuals){
  const positions=visuals?.positions||{},template=visuals?.templates?.national_focus_link;
  const native=geometry.native&&point(positions.link_begin)&&point(positions.link_end);
  const shift=native&&point(template?.position)?template.position:{x:0,y:0};
  const half=native&&size(template?.size)?{x:template.size.width/2,y:template.size.height/2}:{x:0,y:0};
  const a=native?positions.link_begin:{x:geometry.node.width/2,y:geometry.node.height};
  const b=native?positions.link_end:{x:geometry.node.width/2,y:0};
  const start={x:from.x+a.x+shift.x+half.x,y:from.y+a.y+shift.y+half.y};
  const end={x:to.x+b.x+shift.x+half.x,y:to.y+b.y+shift.y+half.y};
  const mid=end.y>start.y?(start.y+end.y)/2:start.y+18;
  const points=start.x===end.x?[start,end]:[start,{x:start.x,y:mid},{x:end.x,y:mid},end];
  return {points,path:'M '+points.map(p=>p.x+' '+p.y).join(' L ')};
 }
 function tiles(points,assets,spacing){
  // Corners keep their real dimensions; straight textures stretch only along
  // the wire axis. Missing art leaves the caller's SVG fallback intact.
  if(!point(spacing)||spacing.x<=0||spacing.y<=0)return [];
  const out=[],w=spacing.x,h=spacing.y;
  const add=(key,x,y,width,height)=>{if(!assets?.[key]?.url||width<=0||height<=0)return false;out.push({sprite:assets[key],x,y,width,height});return true;};
  for(let i=0;i<points.length-1;i++){
   const a=points[i],b=points[i+1],horizontal=a.y===b.y;
   if(!horizontal&&a.x!==b.x)return [];
   const insetA=i? (horizontal?w:h)/2:0,insetB=i<points.length-2?(horizontal?w:h)/2:0;
   const length=Math.abs(horizontal?b.x-a.x:b.y-a.y)-insetA-insetB;
   if(length<0)return [];
   if(length>0){const forward=horizontal?b.x>a.x:b.y>a.y;
    const x=horizontal?Math.min(a.x,b.x)+(forward?insetA:insetB):a.x-w/2;
    const y=horizontal?a.y-h/2:Math.min(a.y,b.y)+(forward?insetA:insetB);
    if(!add(horizontal?'left_right':'up_down',x,y,horizontal?length:w,horizontal?h:length))return [];
   }
  }
  for(let i=1;i<points.length-1;i++){
   const p=points[i],neighbors=[points[i-1],points[i+1]];
   const dirs=neighbors.map(n=>n.x<p.x?'left':n.x>p.x?'right':n.y<p.y?'up':'down');
   const key=['up','down','left','right'].filter(d=>dirs.includes(d)).join('_');
   if(!add(key,p.x-w/2,p.y-h/2,w,h))return [];
  }
  return out;
 }
 function exclusive(a,b,geometry,visuals){
  if(a.x>b.x)[a,b]=[b,a];
  const p=visuals?.positions||{},t=visuals?.templates?.national_focus_exclusive_item;
  if(geometry.native&&point(p.exclusive_offset)&&point(p.exclusive_offset_left)&&point(t?.position)){
   const start={x:a.x+p.exclusive_offset.x+t.position.x,y:a.y+p.exclusive_offset.y+t.position.y};
   const end={x:b.x+p.exclusive_offset_left.x+t.position.x,y:b.y+p.exclusive_offset_left.y+t.position.y};
   // Diagonal or overlapping pairs need a visible conservative connector.
   if(start.y===end.y&&end.x>start.x)return {native:true,start,end,children:t.children};
  }
  return {native:false,start:{x:a.x+geometry.node.width/2,y:a.y+geometry.node.height/2},
   end:{x:b.x+geometry.node.width/2,y:b.y+geometry.node.height/2}};
 }
 const api={layout,component,edge,tiles,exclusive};
 if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiFocusLayout=api;
})(typeof window!=='undefined'?window:globalThis);
