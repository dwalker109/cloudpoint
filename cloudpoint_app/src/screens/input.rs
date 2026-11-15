use ctru::services::hid::KeyPad;

pub struct DelayRepeat<const D: u8, const R: u8> {
    held: Option<KeyPad>,
    delay: u8,
    repeat: u8,
}

impl<const D: u8, const R: u8> DelayRepeat<D, R> {
    pub fn new() -> Self {
        Self {
            held: None,
            delay: 0,
            repeat: 0,
        }
    }

    pub fn tick(&mut self, keys_held: &KeyPad) -> KeyPad {
        match self.held {
            Some(ref held) if held == keys_held => {
                if self.delay < D {
                    self.delay += 1;

                    return KeyPad::empty();
                }

                if self.repeat < R {
                    self.repeat += 1;

                    return KeyPad::empty();
                }

                self.repeat = 0;
                *keys_held
            }
            _ => {
                self.held = (!keys_held.is_empty()).then_some(*keys_held);
                self.delay = 0;
                self.repeat = 0;

                KeyPad::empty()
            }
        }
    }
}
