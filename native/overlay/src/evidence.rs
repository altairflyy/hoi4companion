//! Pixel evidence is checked again after slow recognition; stale matches hide.
use serde_json::Value;

pub fn valid_rectangle(value:&Value,width:u32,height:u32)->Option<[u32;4]> {
    let values=value.as_array()?;
    if values.len()!=4 {return None;}
    let mut rect=[0.;4];
    for (i,v) in values.iter().enumerate() {rect[i]=v.as_f64()?;if !rect[i].is_finite(){return None;}}
    if rect[0]<0.||rect[1]<0.||rect[2]>width as f64||rect[3]>height as f64
        ||rect[2]<=rect[0]||rect[3]<=rect[1] {return None;}
    Some([rect[0].floor() as u32,rect[1].floor() as u32,rect[2].ceil() as u32,rect[3].ceil() as u32])
}
pub fn unchanged(before:&[u8],after:&[u8],width:u32,height:u32,row:&Value)->bool {
    if before.len()!=after.len() || before.len()!=width as usize*height as usize*4 {return false;}
    let mut rectangles=vec![&row["rect"]];
    if let Some(evidence)=row["evidence_rects"].as_array() {rectangles.extend(evidence);}
    rectangles.into_iter().all(|value| {
        let Some([left,top,right,bottom])=valid_rectangle(value,width,height) else {return false;};
        (top..bottom).all(|y|{
            let start=(y as usize*width as usize+left as usize)*4;
            let end=(y as usize*width as usize+right as usize)*4;
            before[start..end]==after[start..end]
        })
    })
}

/// Reuse only if the entire GUI scene is byte-identical. Checking only old
/// positive patches misses a newly appearing duplicate somewhere else.
/// WGC measurements place animated resource-bar pixels in rows 0..82; no
/// research/focus/political panel control is there. Patches are still checked
/// in that strip too, if any were admitted. Guide/view revisions are separate.
pub fn stable(before:&[u8],after:&[u8],width:u32,height:u32,rows:&[Value],ocr:&Value)->bool {
    if rows.is_empty() || !rows.iter().all(|row|unchanged(before,after,width,height,row)){return false;}
    let start=width as usize*height.min(82) as usize*4;
    if before[start..]!=after[start..] {return false;}
    let Some(lines)=ocr["lines"].as_array() else{return false;};
    let mut checked=0;
    for line in lines {
        let Some(words)=line["words"].as_array() else{return false;};
        for word in words {
            let (Some(x),Some(y),Some(w),Some(h))=(word["x"].as_f64(),word["y"].as_f64(),word["width"].as_f64(),word["height"].as_f64()) else{return false;};
            // The animated resources/date bar is excluded by the original
            // scene policy too. No admitted research/focus control lives here.
            if y<80. {continue;}
            if !unchanged(before,after,width,height,&serde_json::json!({"rect":[x,y,x+w,y+h]})){return false;}
            checked+=1;
        }
    }
    checked>0
}
#[cfg(test)]
mod tests {
    use super::*;use serde_json::json;
    #[test] fn hidden_or_changed_supporting_icon_invalidates_match() {
        let a=vec![7;8*8*4];let mut b=a.clone();
        let row=json!({"rect":[0,0,2,2],"evidence_rects":[[4,4,6,6]]});
        assert!(unchanged(&a,&b,8,8,&row));
        b[(4*8+4)*4]=8;
        assert!(!unchanged(&a,&b,8,8,&row));
        assert!(!unchanged(&a,&a,8,8,&json!({"rect":[0,0,20,20]})));
    }
    #[test] fn new_duplicate_anywhere_invalidates_global_admission() {
        let a=vec![7;100*100*4];let mut b=a.clone();
        let rows=vec![json!({"rect":[0,80,2,82],"evidence_rects":[[4,80,6,82]]})];
        let ocr=json!({"lines":[{"words":[{"x":10,"y":85,"width":2,"height":3}]}]});
        b[(95*100+90)*4]=8;
        assert!(!stable(&a,&b,100,100,&rows,&ocr));
        b=a.clone();b[(85*100+40)*4]=8;
        assert!(!stable(&a,&b,100,100,&rows,&ocr));
        b=a.clone();b[0]=8;
        assert!(stable(&a,&b,100,100,&rows,&ocr));
        b[(85*100+10)*4]=9;
        assert!(!stable(&a,&b,100,100,&rows,&ocr));
        assert!(!stable(&a,&a,100,100,&[],&ocr));
    }
}
