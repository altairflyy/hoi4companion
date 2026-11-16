//! Reference images for the step card (the design/template pictures the
//! walkthrough page shows). Bytes come from the backend's reference library
//! over loopback HTTP, are decoded off the HUD loop, scaled down once and
//! cached by id. A missing, slow or broken image never blocks or hides the
//! card: until its picture is ready the card shows the image's name only.
use std::collections::HashMap;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

/// Most images one card shows (the backend sends at most this many too).
pub const CARD_IMAGES: usize = 3;
/// Longest side of a decoded thumbnail, in pixels.
const MAX_SIDE: u32 = 1024;
/// Largest image file accepted from the backend.
const MAX_BYTES: usize = 20_000_000;
/// A failed image is asked for again only after this pause.
const RETRY_AFTER: Duration = Duration::from_secs(60);
/// Cached entries beyond this are dropped, except the current card's.
const MAX_ENTRIES: usize = 16;

/// A decoded thumbnail: premultiplied BGRA, `width * 4` bytes per row.
pub struct Thumb {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}
impl std::fmt::Debug for Thumb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Thumb({}x{})", self.width, self.height)
    }
}

/// One image on the card; `thumb` is None until it is decoded (or if it fails).
#[derive(Clone, Debug, Default)]
pub struct CardImage {
    pub id: String,
    pub name: String,
    pub thumb: Option<Arc<Thumb>>,
}
/// Cheap comparison (the card is compared every frame): same decoded picture.
impl PartialEq for CardImage {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && match (&self.thumb, &other.thumb) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}

enum Entry {
    Pending,
    Ready(Arc<Thumb>),
    Failed(Instant),
}
type Fetch = fn(u16, &str) -> Result<Vec<u8>, String>;
type Done = (String, Result<Thumb, String>);

pub struct Cache {
    port: u16,
    fetch: Fetch,
    entries: HashMap<String, Entry>,
    sender: mpsc::Sender<Done>,
    receiver: mpsc::Receiver<Done>,
}

impl Cache {
    pub fn new(port: u16) -> Self {
        Self::with_fetch(port, fetch)
    }
    fn with_fetch(port: u16, fetch: Fetch) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self { port, fetch, entries: HashMap::new(), sender, receiver }
    }
    /// Give the card's images their decoded pictures where ready, and start
    /// fetching the ones not asked for yet. Never waits.
    pub fn attach(&mut self, images: &mut [CardImage]) {
        let now = Instant::now();
        while let Ok((id, result)) = self.receiver.try_recv() {
            if let Some(entry) = self.entries.get_mut(&id) {
                *entry = match result {
                    Ok(thumb) => Entry::Ready(Arc::new(thumb)),
                    Err(_) => Entry::Failed(now),
                };
            }
        }
        if self.entries.len() > MAX_ENTRIES {
            self.entries.retain(|id, _| images.iter().any(|i| &i.id == id));
        }
        for image in images.iter_mut() {
            let start = match self.entries.get(&image.id) {
                Some(Entry::Ready(thumb)) => {
                    image.thumb = Some(thumb.clone());
                    false
                }
                Some(Entry::Pending) => false,
                Some(Entry::Failed(at)) => now.saturating_duration_since(*at) >= RETRY_AFTER,
                None => true,
            };
            if start {
                self.entries.insert(image.id.clone(), Entry::Pending);
                let (port, fetch, sender, id) = (self.port, self.fetch, self.sender.clone(), image.id.clone());
                let spawned = std::thread::Builder::new().name("hud-card-image".into()).spawn(move || {
                    let result = fetch(port, &id).and_then(|bytes| decode(&bytes));
                    let _ = sender.send((id, result));
                });
                if spawned.is_err() {
                    self.entries.insert(image.id.clone(), Entry::Failed(now));
                }
            }
        }
    }
}

fn fetch(port: u16, id: &str) -> Result<Vec<u8>, String> {
    let path = format!("/api/reference-library/image?id={}", encode(id));
    let (status, bytes) = crate::local_api::Api { port }.request(&path, None, MAX_BYTES)?;
    if status != 200 {
        return Err(format!("Reference image unavailable ({status})"));
    }
    Ok(bytes)
}

/// Percent-encode a query value (ids are normally hex digests).
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// PNG or JPEG bytes to a premultiplied BGRA thumbnail of at most `MAX_SIDE`.
pub fn decode(bytes: &[u8]) -> Result<Thumb, String> {
    let mut picture = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    if picture.width() == 0 || picture.height() == 0 {
        return Err("Empty reference image".into());
    }
    if picture.width() > MAX_SIDE || picture.height() > MAX_SIDE {
        picture = picture.thumbnail(MAX_SIDE, MAX_SIDE);
    }
    let rgba = picture.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut bgra = rgba.into_raw();
    for pixel in bgra.chunks_exact_mut(4) {
        let alpha = pixel[3] as u16;
        let premultiply = |c: u8| ((c as u16 * alpha + 127) / 255) as u8;
        let (r, g, b) = (premultiply(pixel[0]), premultiply(pixel[1]), premultiply(pixel[2]));
        pixel[0] = b;
        pixel[1] = g;
        pixel[2] = r;
    }
    Ok(Thumb { width, height, bgra })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        let picture = image::RgbaImage::from_pixel(width, height, image::Rgba(rgba));
        let mut bytes = std::io::Cursor::new(Vec::new());
        picture.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn decodes_to_premultiplied_bgra_and_bounds_the_size() {
        let thumb = decode(&png(2, 1, [200, 100, 50, 128])).unwrap();
        assert_eq!((thumb.width, thumb.height), (2, 1));
        assert_eq!(&thumb.bgra[..4], &[25, 50, 100, 128]);
        let large = decode(&png(3000, 1500, [0, 0, 0, 255])).unwrap();
        assert_eq!((large.width, large.height), (1024, 512));
        assert!(decode(b"not an image").is_err());
    }

    #[test]
    fn decodes_jpeg_reference_images() {
        let picture = image::RgbImage::from_pixel(8, 4, image::Rgb([10, 200, 30]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        picture.write_to(&mut bytes, image::ImageFormat::Jpeg).unwrap();
        let thumb = decode(&bytes.into_inner()).unwrap();
        assert_eq!((thumb.width, thumb.height, thumb.bgra[3]), (8, 4, 255));
    }

    #[test]
    fn ids_are_query_safe() {
        assert_eq!(encode("ab12-_.~"), "ab12-_.~");
        assert_eq!(encode("a b&c/é"), "a%20b%26c%2F%C3%A9");
    }

    fn served(_: u16, id: &str) -> Result<Vec<u8>, String> {
        if id == "good" { Ok(png(4, 2, [1, 2, 3, 255])) } else { Err("missing".into()) }
    }

    #[test]
    fn images_arrive_without_blocking_and_failures_keep_the_name() {
        let mut cache = Cache::with_fetch(0, served);
        let mut images = vec![
            CardImage { id: "good".into(), name: "F1ca".into(), thumb: None },
            CardImage { id: "bad".into(), name: "Broken".into(), thumb: None },
        ];
        cache.attach(&mut images);
        assert!(images.iter().all(|i| i.thumb.is_none()), "first call only starts the fetches");
        let deadline = Instant::now() + Duration::from_secs(10);
        let settled = |cache: &Cache, images: &[CardImage]| {
            images[0].thumb.is_some() && matches!(cache.entries.get("bad"), Some(Entry::Failed(_)))
        };
        while !settled(&cache, &images) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
            cache.attach(&mut images);
        }
        let thumb = images[0].thumb.clone().expect("decoded picture attached");
        assert_eq!((thumb.width, thumb.height), (4, 2));
        assert!(images[1].thumb.is_none());
        assert!(matches!(cache.entries.get("bad"), Some(Entry::Failed(_))));
        // The same picture is reused, so the card compares equal and is not redrawn.
        let mut again = vec![CardImage { id: "good".into(), name: "F1ca".into(), thumb: None }];
        cache.attach(&mut again);
        assert_eq!(again[0], images[0]);
        assert_ne!(again[0], CardImage { thumb: None, ..images[0].clone() });
    }
}
