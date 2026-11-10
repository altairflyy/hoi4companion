'use strict';
const zlib=require('node:zlib');
function crc32(bytes){let crc=0xffffffff;for(const byte of bytes){crc^=byte;for(let i=0;i<8;i++)crc=(crc>>>1)^((crc&1)?0xedb88320:0);}return (crc^0xffffffff)>>>0;}
function zip(entries){
 const local=[],central=[];let offset=0;
 for(const [name,value] of Object.entries(entries)){
  const n=Buffer.from(name),bytes=Buffer.isBuffer(value)?value:Buffer.from(value),header=Buffer.alloc(30),directory=Buffer.alloc(46),crc=crc32(bytes);
  header.writeUInt32LE(0x04034b50);header.writeUInt16LE(20,4);header.writeUInt16LE(0x800,6);header.writeUInt32LE(crc,14);header.writeUInt32LE(bytes.length,18);header.writeUInt32LE(bytes.length,22);header.writeUInt16LE(n.length,26);local.push(header,n,bytes);
  directory.writeUInt32LE(0x02014b50);directory.writeUInt16LE(20,4);directory.writeUInt16LE(20,6);directory.writeUInt16LE(0x800,8);directory.writeUInt32LE(crc,16);directory.writeUInt32LE(bytes.length,20);directory.writeUInt32LE(bytes.length,24);directory.writeUInt16LE(n.length,28);directory.writeUInt32LE(offset,42);central.push(directory,n);offset+=header.length+n.length+bytes.length;
 }
 const directory=Buffer.concat(central),end=Buffer.alloc(22);end.writeUInt32LE(0x06054b50);end.writeUInt16LE(Object.keys(entries).length,8);end.writeUInt16LE(Object.keys(entries).length,10);end.writeUInt32LE(directory.length,12);end.writeUInt32LE(offset,16);return Buffer.concat([...local,directory,end]);
}
function word(actions=[],research=[],focus=[]){
 const escape=s=>s.replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;');
 const cell=lines=>'<w:tc>'+lines.map(text=>'<w:p><w:r><w:t>'+escape(text)+'</w:t></w:r></w:p>').join('')+'</w:tc>';
 return zip({'word/document.xml':'<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr>'+['ACTIONS','RESEARCH PRIORITY','FOCUS PRIORITY'].map(s=>cell([s])).join('')+'</w:tr><w:tr>'+[actions,research,focus].map(cell).join('')+'</w:tr></w:tbl></w:body></w:document>'});
}
function png(){
 const chunk=(type,data)=>{const t=Buffer.from(type),length=Buffer.alloc(4),crc=Buffer.alloc(4);length.writeUInt32BE(data.length);crc.writeUInt32BE(crc32(Buffer.concat([t,data])));return Buffer.concat([length,t,data,crc]);};
 const header=Buffer.alloc(13);header.writeUInt32BE(1);header.writeUInt32BE(1,4);header[8]=8;header[9]=6;
 return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',header),chunk('IDAT',zlib.deflateSync(Buffer.from([0,255,0,0,255]))),chunk('IEND',Buffer.alloc(0))]);
}
module.exports={zip,word,png};
