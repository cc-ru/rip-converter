const RESP_PREC: i32 = 10;

pub struct DFPWM {
    response: i32,
    level: i32,
    last_bit: bool,
}

impl DFPWM {
    pub fn new() -> DFPWM {
        DFPWM {
            response: 0,
            level: 0,
            last_bit: false,
        }
    }

    fn ctx_update(&mut self, cur_bit: bool) {
        let target = match cur_bit {
            true => 127,
            false => -128,
        };
        let mut nlevel = self.level + ((self.response * (target - self.level) +
                                       (1 << (RESP_PREC - 1))) >> RESP_PREC);
        if nlevel == self.level && self.level != target {
            nlevel += match cur_bit {
                true => 1,
                false => -1,
            };
        }

        let rtarget = if cur_bit == self.last_bit {
            (1 << RESP_PREC) - 1
        } else {
            0
        };

        let mut nresponse = self.response;
        if self.response != rtarget {
            nresponse += if cur_bit == self.last_bit {
                1
            } else {
                -1
            };
        }

        if RESP_PREC > 8 {
            if nresponse < 2 << (RESP_PREC - 8) {
                nresponse = 2 << (RESP_PREC - 8);
            }
        }

        self.response = nresponse;
        self.last_bit = cur_bit;
        self.level = nlevel;
    }

    pub fn compress(&mut self, src: &Vec<u8>, dest: &mut Vec<u8>) {
        let mut src_offset: usize = 0;
        let mut dest_offset: usize = 0;
        for _ in 0..src.len() {
            let mut d = 0;

            for _ in 0..8 {
                let in_level = src[src_offset] as i32;
                src_offset += 1;

                let cur_bit = in_level > self.level ||
                              in_level == self.level && self.level == 127;
                d >>= 1;
                d += match cur_bit {
                    true => 128,
                    false => 0,
                };
                self.ctx_update(cur_bit);
            }
            dest[dest_offset] = d as u8;
            dest_offset += 1;
        }
    }
}
