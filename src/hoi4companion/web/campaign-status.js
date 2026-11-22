'use strict';
((root)=>{
 function present(packet,projected,t){
  const code=packet?.usable===true?projected?.code:packet?.reader_status;
  const date=String(projected?.date??'');
  switch(code){
   case 'game_absent':
    return {message:t('HOI4 is not running. You can browse the plan; progress is checked once you start the game.')};
   case 'build_unsupported':
    return {message:t('This HOI4 version is not supported for reading yet: steps stay unverified.')};
   case 'campaign_unverified':
    return {message:t('HOI4 is running, but Companion cannot verify this campaign yet. Steps stay unverified.'),hint:t('Wait for the campaign to finish loading.')};
   case 'wrong_country':
    return {message:projected.country&&projected.guide_country?
     t('This game does not match the guide ({0}, guide for {1}). Progress is not counted.',projected.country,projected.guide_country):
     t('This game does not match the guide. Progress is not counted.'),action:'library'};
   case 'wrong_tree':
    return {message:t('This game uses a different focus tree from the guide. Progress is not counted.'),action:'library'};
   case 'wrong_campaign':
    return {message:t('This game does not match the saved checklist. Progress is not counted.'),action:'library'};
   case 'linked':
    return {message:/^\d{8}$/.test(date)?
     t('Game linked: {0}, {1}. Progress updates on its own.',projected.country,date.slice(0,4)+'-'+date.slice(4,6)+'-'+date.slice(6)):
     t('Game linked: {0}. Progress updates on its own.',projected.country),action:'walkthrough'};
   default:
    return {message:t('Checking the game…')};
  }
 }
 const api={present};
 if(typeof module==='object'&&module.exports)module.exports=api;else root.HoiCampaignStatus=api;
})(typeof window==='object'?window:globalThis);
