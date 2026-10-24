//! Direct Windows OCR over bounded raw pixel buffers. No shell or image codec.
use std::path::Path;
use serde::{Deserialize,Serialize};
use windows::core::{Error, Result};
use windows::Graphics::Imaging::{BitmapPixelFormat,SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;
use windows::Win32::Foundation::E_INVALIDARG;
use crate::capture::{error,Apartment,Frame};

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct OcrWord {
    pub text:String,
    pub x:f64,
    pub y:f64,
    pub width:f64,
    pub height:f64,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct OcrLine {pub text:String,pub words:Vec<OcrWord>}
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct OcrPage {
    pub width:u32,
    pub height:u32,
    pub language:String,
    pub lines:Vec<OcrLine>,
}

pub struct Ocr {engine:OcrEngine,maximum:u32,language:String,_apartment:Apartment}

fn validate(width:u32,height:u32,data:&[u8])->Result<()> {
    let bytes=(width as usize).checked_mul(height as usize).and_then(|n|n.checked_mul(4));
    if width==0||height==0||bytes!=Some(data.len())||data.len()>128_000_000 {
        return Err(Error::new(E_INVALIDARG,"Invalid or oversized BGRA image"));
    }
    Ok(())
}

/// Pillow-compatible 8-bit bicubic sampling. The compatibility arithmetic is
/// derived from Pillow12.3.0 libImaging/Resample.c (MIT-CMU license):
/// https://github.com/python-pillow/Pillow/blob/12.3.0/src/libImaging/Resample.c
/// Attribution/license: src/hoi4companion/licenses/pillow-resampling.txt.
/// normalized double coefficients -> signed22-bit weights -> clipped8-bit
/// horizontal intermediate -> clipped8-bit vertical result.
struct Kernel {start:usize,weights:Vec<i32>}
fn bicubic(value:f64)->f64 {
    let x=value.abs();
    if x<1. {((1.5*x-2.5)*x)*x+1.}
    else if x<2. {(((x-5.)*x+8.)*x-4.)* -0.5}
    else {0.}
}
fn kernels(input:usize,output:usize)->Vec<Kernel> {
    let scale=input as f64/output as f64;
    (0..output).map(|position|{
        let center=(position as f64+0.5)*scale;
        let start=((center-2.+0.5) as i32).max(0) as usize;
        let end=((center+2.+0.5) as usize).min(input);
        let raw:Vec<_>=(start..end).map(|x|bicubic(x as f64-center+0.5)).collect();
        let total:f64=raw.iter().sum();
        let weights=raw.into_iter().map(|weight|{
            let normalized=if total==0. {weight} else {weight/total};
            let adjustment=if normalized<0. {-0.5} else {0.5};
            (adjustment+normalized*((1u32<<22) as f64)) as i32
        }).collect();
        Kernel {start,weights}
    }).collect()
}
fn resize_bicubic(gray:&[u8],width:u32,height:u32,scale:u32)->Vec<u8> {
    let input_width=width as usize;let input_height=height as usize;
    let output_width=input_width*scale as usize;let output_height=input_height*scale as usize;
    let horizontal=kernels(input_width,output_width);let vertical=kernels(input_height,output_height);
    let mut intermediate=vec![0;output_width*input_height];
    for y in 0..input_height {
        for (x,kernel) in horizontal.iter().enumerate() {
            let sum=kernel.weights.iter().enumerate().fold(1i64<<21,|sum,(offset,weight)|
                sum+gray[y*input_width+kernel.start+offset] as i64* *weight as i64);
            intermediate[y*output_width+x]=(sum>>22).clamp(0,255) as u8;
        }
    }
    let mut output=vec![0;output_width*output_height];
    for (y,kernel) in vertical.iter().enumerate() {
        for x in 0..output_width {
            let sum=kernel.weights.iter().enumerate().fold(1i64<<21,|sum,(offset,weight)|
                sum+intermediate[(kernel.start+offset)*output_width+x] as i64* *weight as i64);
            output[y*output_width+x]=(sum>>22).clamp(0,255) as u8;
        }
    }
    output
}

/// Exact Pillow grayscale/contrast1.4/BICUBIC behavior used by the old overlay.
fn tile_pixels(width:u32,data:&[u8],region:[u32;4],scale:u32)->Vec<u8> {
    let [left,top,tw,th]=region;
    debug_assert!(left+tw<=width && ((top+th)*width*4) as usize<=data.len());
    let mut gray=Vec::with_capacity((tw*th) as usize);
    for y in top..top+th {
        for x in left..left+tw {
            let p=((y*width+x)*4) as usize;
            let luma=(19595u32*data[p+2] as u32+38470u32*data[p+1] as u32+7471u32*data[p] as u32+32768)>>16;
            gray.push(luma as u8);
        }
    }
    let total:u64=gray.iter().map(|v|*v as u64).sum();
    // ImageEnhance.Contrast uses int(ImageStat.mean +0.5), then ImagingBlend
    // evaluates the extrapolation with float32 alpha/temp, truncating to u8.
    let mean=(total as f64/gray.len() as f64+0.5) as i32;
    for pixel in &mut gray {
        *pixel=(mean as f32+1.4f32*(*pixel as i32-mean) as f32).clamp(0.,255.) as u8;
    }
    let resized=if scale==1 {gray} else {resize_bicubic(&gray,tw,th,scale)};
    let mut bgra=Vec::with_capacity((tw*th*scale*scale*4) as usize);
    for luma in resized {bgra.extend_from_slice(&[luma,luma,luma,255]);}
    bgra
}

pub struct PreparedTile {pub left:u32,pub top:u32,pub width:u32,pub height:u32,pub bgra:Vec<u8>}

/// Diagnostic export for byte-level reference comparisons. Runtime OCR retains
/// one prepared tile at a time; this batch helper has an additional output cap.
pub fn prepare_tiles(width:u32,height:u32,data:&[u8],scale:u32)->Result<Vec<PreparedTile>> {
    validate(width,height,data)?;
    if !matches!(scale,1|2)||data.len()*scale as usize*scale as usize>128_000_000 {
        return Err(Error::new(E_INVALIDARG,"Invalid OCR scale or prepared-image batch budget"));
    }
    let mut tiles=Vec::new();
    for top in (0..height).step_by(720) {
        for left in (0..width).step_by(1280) {
            let tw=(width-left).min(1280);let th=(height-top).min(720);
            tiles.push(PreparedTile {left,top,width:tw*scale,height:th*scale,
                bgra:tile_pixels(width,data,[left,top,tw,th],scale)});
        }
    }
    Ok(tiles)
}

impl Ocr {
    pub fn new()->Result<Self> {
        let apartment=Apartment::new()?;
        let engine=OcrEngine::TryCreateFromUserProfileLanguages()?;
        let language=engine.RecognizerLanguage()?.LanguageTag()?.to_string();
        Ok(Self {maximum:OcrEngine::MaxImageDimension()?,engine,language,_apartment:apartment})
    }

    pub fn recognize(&self,frame:&Frame,scale:u32)->Result<OcrPage> {
        self.recognize_bgra(frame.width,frame.height,&frame.bgra,scale)
    }

    pub fn recognize_bgra(&self,width:u32,height:u32,data:&[u8],scale:u32)->Result<OcrPage> {
        validate(width,height,data)?;
        if !matches!(scale,1|2) {return Err(Error::new(E_INVALIDARG,"OCR scale must be 1 or 2"));}
        let tile_width=1280.min(self.maximum/scale);
        let tile_height=720.min(self.maximum/scale);
        if tile_width==0||tile_height==0 {return Err(error("Invalid Windows OCR dimension limit"));}
        let mut page=OcrPage {width,height,language:self.language.clone(),lines:Vec::new()};
        for top in (0..height).step_by(tile_height as usize) {
            for left in (0..width).step_by(tile_width as usize) {
                let tw=tile_width.min(width-left);let th=tile_height.min(height-top);
                let pixels=tile_pixels(width,data,[left,top,tw,th],scale);
                let tile=self.recognize_bitmap_bgra(tw*scale,th*scale,&pixels)?;
                for mut line in tile.lines {
                    for word in &mut line.words {
                        word.x=word.x/scale as f64+left as f64;
                        word.y=word.y/scale as f64+top as f64;
                        word.width/=scale as f64;word.height/=scale as f64;
                    }
                    page.lines.push(line);
                }
            }
        }
        Ok(page)
    }

    /// Single already-prepared bitmap; used to compare exactly identical pixels
    /// with the former OCR transport and to isolate preprocessing cost.
    pub fn recognize_bitmap_bgra(&self,width:u32,height:u32,data:&[u8])->Result<OcrPage> {
        validate(width,height,data)?;
        if width>self.maximum||height>self.maximum {return Err(Error::new(E_INVALIDARG,"Bitmap exceeds Windows OCR limit"));}
        let writer=DataWriter::new()?;
        writer.WriteBytes(data)?;
        let buffer=writer.DetachBuffer()?;
        let bitmap=SoftwareBitmap::CreateCopyFromBuffer(&buffer,BitmapPixelFormat::Bgra8,width as i32,height as i32)?;
        let result=self.engine.RecognizeAsync(&bitmap)?.get();
        let _=bitmap.Close();
        let result=result?;
        let mut lines=Vec::new();
        let source_lines=result.Lines()?;
        for index in 0..source_lines.Size()? {
            let line=source_lines.GetAt(index)?;let source_words=line.Words()?;
            let mut words=Vec::new();
            for index in 0..source_words.Size()? {
                let word=source_words.GetAt(index)?;let rect=word.BoundingRect()?;
                words.push(OcrWord {text:word.Text()?.to_string(),x:rect.X as f64,y:rect.Y as f64,
                    width:rect.Width as f64,height:rect.Height as f64});
            }
            lines.push(OcrLine {text:line.Text()?.to_string(),words});
        }
        Ok(OcrPage {width,height,language:self.language.clone(),lines})
    }

    pub fn recognize_file(&self,path:&Path,scale:u32)->Result<OcrPage> {
        let image=image::open(path).map_err(|e|error(e.to_string()))?.into_rgba8();
        let (width,height)=image.dimensions();let mut data=image.into_raw();
        for pixel in data.as_chunks_mut::<4>().0 {pixel.swap(0,2);}
        self.recognize_bgra(width,height,&data,scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_image_bounds_reject_truncation_and_empty_dimensions() {
        assert!(validate(0,1,&[]).is_err());
        assert!(validate(2,2,&[0;15]).is_err());
        assert!(validate(2,2,&[0;16]).is_ok());
        assert!(validate(u32::MAX,u32::MAX,&[]).is_err());
    }
    #[test]
    fn tiles_preserve_position_and_opaque_uniform_pixels() {
        let image=vec![42;4*4*4];
        let tile=tile_pixels(4,&image,[1,2,2,2],2);
        assert_eq!(tile.len(),4*4*4);
        assert!(tile.as_chunks::<4>().0.iter().all(|p|*p==[42,42,42,255]));
    }
    #[test]
    fn contrast_does_not_mix_pixels_from_another_tile() {
        let image=[0,0,0,255,255,255,255,255,70,70,70,255,70,70,70,255];
        let tile=tile_pixels(4,&image,[2,0,2,1],1);
        assert_eq!(tile,[70,70,70,255,70,70,70,255]);
    }
    #[test]
    fn pillow_reference_colored_fixture_is_byte_identical_at_both_scales() {
        let rgb=[[0,0,0],[255,255,255],[255,0,0],[0,255,0],[0,0,255],[13,47,91],
            [119,120,121],[12,200,234],[1,2,3],[127,128,129],[70,80,90],[180,30,140]];
        let mut pixels=Vec::new();
        for [r,g,b] in rgb {pixels.extend_from_slice(&[b,g,r,255]);}
        let scale1=tile_pixels(4,&pixels,[0,0,4,3],1);
        assert_eq!(scale1.as_chunks::<4>().0.iter().map(|p|p[0]).collect::<Vec<_>>(),[0,255,69,172,3,21,130,170,0,142,72,84]);
        let actual=tile_pixels(4,&pixels,[0,0,4,3],2);
        let grayscale:Vec<_>=actual.as_chunks::<4>().0.iter().map(|p|p[0]).collect();
        assert_eq!(grayscale,vec![0,56,229,250,106,73,146,182,0,42,171,195,106,93,153,182,1,13,47,79,106,136,167,182,1,6,19,51,101,134,147,154,0,22,91,115,92,86,97,102,0,30,124,145,88,63,72,77]);
    }
}
