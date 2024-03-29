
use std::io::{ Read, Seek, SeekFrom };

use hyper::{Client, Request};
use hyper_tls::HttpsConnector;

use crossbeam::channel::{ unbounded, Sender, Receiver };

use rodio::source::Source;
use lewton::inside_ogg::OggStreamReader;

mod args;
mod websocket;
mod shared;

struct ChannelReader {
    recv: Receiver<u8>,
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut written = 0;
        for x in buf.iter_mut() {
            *x = self.recv.recv().unwrap();
            written += 1;
        }
        Ok(written)
    }
}

impl Seek for ChannelReader {
    fn seek(&mut self, _pos: SeekFrom) -> std::io::Result<u64> {
        Ok(0)
    }
}

struct VorbisStream {
    ogg: OggStreamReader<ChannelReader>,
    buf: Option<Vec<i16>>,
    pos: usize,
}

impl std::iter::Iterator for VorbisStream {
    type Item = i16;
    
    fn next(&mut self) -> Option<Self::Item> {
        let buf = if let Some(buf) = self.buf.as_ref() {
            buf
        } else {
            self.pos = 0;
            let maybe_buf = self.ogg.read_dec_packet_itl().unwrap();
            if let Some(buf) = maybe_buf {
                if buf.len() == 0 {
                    // println!("buf len 0");
                    return Some(0);
                }
                self.buf = Some(buf);
                self.buf.as_ref().unwrap()
            } else {
                // println!("no maybe buf");
                return Some(0);
            }
        };
        
        let x = buf[self.pos];
        
        self.pos += 1;
        
        if self.pos == buf.len() {
            self.buf = None
        }
        
        Some(x)
    }
}

impl rodio::source::Source for VorbisStream {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    
    fn channels(&self) -> u16 {
        2
    }
    
    fn sample_rate(&self) -> u32 {
        48000
    }
    
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

#[tokio::main]
async fn main() {
    color_backtrace::install();
    
    let args = args::get_args();
    // println!("{}", args.volume);
    
    let (ac_s, ac_r): (Sender<u8>, _) = unbounded();
    
    let reader = ChannelReader {
        recv: ac_r,
    };
    
    let volume = args.volume;
    std::thread::spawn(move || {
        let decoder = VorbisStream {
            ogg: OggStreamReader::new(reader).unwrap(),
            buf: None,
            pos: 0,
        };
        
        let device = rodio::default_output_device().unwrap();
        
        rodio::play_raw(&device, decoder.amplify(volume).convert_samples());
    });
    
    let https = HttpsConnector::new().unwrap();
    let client = Client::builder().build::<_, hyper::Body>(https);
    
    let url = if args.kpop {
        "https://listen.moe/kpop/stream"
    } else {
        "https://listen.moe/stream"
    };
    
    let mut req = Request::builder();
    let req = req
        .method("GET")
        .uri(url)
        .header("Range", "bytes=0-")
        .header("Referer", url)
        .header("User-Agent", shared::USER_AGENT)
        .body(hyper::Body::empty())
        .expect("request builder");
        ;
    
    let res = client.request(req).await.unwrap();
    
    // println!("{:?}", res);
    
    let kpop = args.kpop;
    tokio::spawn(async move {
        let ws = websocket::connect(kpop).await;
        
        let (_send, mut recv) = websocket::wrap_ws(ws).await;
        
        loop {
            let msg = recv.get_json().await;
            
            let op = msg.get("op").unwrap().as_u64().unwrap();
            if op == 1 {
                let t = msg.get("t").unwrap().as_str().unwrap();
                if t == "TRACK_UPDATE" {
                    let d = msg.get("d").unwrap();
                    let song = d.get("song").unwrap();
                    let title = song.get("title").unwrap().as_str().unwrap();
                    println!("now playing: {}", title);
                    let artists = song.get("artists").unwrap().as_array().unwrap();
                    for artist in artists {
                        if let Some(name) = artist.get("name") {
                            let name = name.as_str().unwrap_or("");
                            let name_romaji = artist.get("nameRomaji").map(|x| x.as_str().unwrap_or("")).unwrap_or("");
                            println!("  artist: {} ({})", name, name_romaji);
                        }
                    }
                }
            }
        }
    });
    
    let mut body = res.into_body();
    while let Some(next) = body.next().await {
        let chunk = next.unwrap();
        for x in chunk {
            ac_s.send(x).unwrap();
        }
    }
}
