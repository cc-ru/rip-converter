const RESP_PREC: i8 = 10;

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
            if nresponse < 1 + (1 << (RESP_PREC - 8)) {
                nresponse = 1 + (1 << (RESP_PREC - 8));
            }
        }

        self.response = nresponse;
        self.last_bit = cur_bit;
        self.level = nlevel;
    }

    pub fn compress(&mut self, src: &Vec<u8>, dest: &mut Vec<u8>) {
        let len = src.len();
        dest.reserve(len);
        for i in 0..(len / 8) {
            let mut d = 0u8;

            for j in 0..8 {
                let index = i * 8 + j;
                let in_level = if index < len {
                    src[index] as i8
                } else {
                    0i8
                };

                let cur_bit = !((in_level as i32) < self.level ||
                    ((in_level as i32) == -128));
                d >>= 1;
                d += match cur_bit {
                    true => 128,
                    false => 0,
                };
                self.ctx_update(cur_bit);
            }
            dest.push(d);
        }
    }
}
