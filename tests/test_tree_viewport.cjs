'use strict';

const test=require('node:test');
const assert=require('node:assert/strict');
const {create}=require('../src/hoi4companion/web/tree-viewport.js');

test('tree overview fits, edge targets are centered and only explicit fit resets manual zoom',()=>{
 const viewport={
  clientWidth:600,clientHeight:380,
  scrollTo(left,top){this.scrollLeft=Math.max(0,left);this.scrollTop=Math.max(0,top);}
 };
 const world={style:{}},sizer={style:{}},slider={min:40},output={},caption={hidden:true};
 const camera=create({viewport,world,sizer,slider,output,caption,targetLabel:'Target',hideTooltip(){}});
 camera.size(2400,1000);
 assert.equal(slider.value,24);
 assert.equal(slider.min,24);
 assert.equal(sizer.style.width,'576px');
 assert.equal(viewport.scrollLeft,0);

 const target={name:'Edge node',x:0,y:0,width:100,height:100};
 camera.showTarget(target);
 assert.equal(output.textContent,'120%');
 assert.equal(caption.textContent,'Target · Edge node');
 assert.equal(caption.hidden,false);
 assert.equal(world.style.left,'240px');
 assert.equal(world.style.top,'130px');
 assert.equal(viewport.scrollLeft,0);
 assert.equal(viewport.scrollTop,0);

 slider.value=110;
 camera.manual(target);
 camera.overview();
 viewport.clientWidth=450;
 camera.size(3200,1200);
 camera.showTarget({name:'Far node',x:2500,y:600,width:100,height:100});
 assert.equal(output.textContent,'110%');
 assert.equal(world.style.left,'0px');
 assert.equal(Math.round(parseFloat(sizer.style.width)),3520);
 assert.ok(viewport.scrollLeft>0);

 camera.fit();
 assert.equal(output.textContent,'13%');
 assert.equal(caption.hidden,true);
 assert.equal(viewport.scrollLeft,0);
 assert.equal(viewport.scrollTop,0);
});
