//! minifb 0.29 forwards Windows system key messages but does not track Alt
//! scan codes. Read the thread's Win32 keyboard state for that modifier.

const VK_MENU: i32 = 0x12;
const VK_LCONTROL: i32 = 0xa2;
const VK_LMENU: i32 = 0xa4;
const VK_RMENU: i32 = 0xa5;

#[link(name = "user32")]
unsafe extern "system" {
    #[link_name = "GetKeyState"]
    fn get_key_state(vkey: i32) -> i16;
}

fn key_down(vkey: i32) -> bool {
    // The high bit is set while the key is down; GetKeyState returns a signed SHORT.
    unsafe { get_key_state(vkey) < 0 }
}

pub fn alt_state() -> (bool, bool) {
    let alt = key_down(VK_MENU);
    // Windows synthesizes left Ctrl for AltGr. Keep AltGr available for text input.
    let altgr = key_down(VK_RMENU) && key_down(VK_LCONTROL) && !key_down(VK_LMENU);
    (alt, altgr)
}
