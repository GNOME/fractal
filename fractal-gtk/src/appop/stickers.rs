use appop::AppOp;


use types::StickerGroup;
use types::Sticker;


impl AppOp {
    pub fn stickers_loaded(&self, stickers: Vec<StickerGroup>) {
        for sg in stickers {
            println!("STICKER GROUP: {}, {}", sg.name, sg.purchased);
        }
    }
}
