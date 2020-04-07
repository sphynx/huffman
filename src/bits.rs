pub struct BitReader<'a> {
    data: &'a [u8],
    ix: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        BitReader { data, ix: 0 }
    }

    pub fn read_bit(&mut self) -> Option<bool> {
        let byte_ix = self.byte_ix();

        if byte_ix < self.data.len() {
            let byte = self.data[byte_ix];
            let res = (byte & (1 << self.bit_ix())) > 0;
            self.ix += 1;
            Some(res)
        } else {
            None
        }
    }

    pub fn read_bits(&mut self, n: u8) -> Option<u8> {
        assert!(n <= 8);

        let mut res = 0;
        for _ in 0..n {
            if let Some(b) = self.read_bit() {
                res = (res << 1) | if b { 1 } else { 0 };
            } else {
                return None;
            }
        }
        Some(res)
    }

    pub fn read_u32_be(&mut self) -> Option<u32> {
        let mut bytes = [0; 4];
        for i in 0..4 {
            if let Some(byte) = self.read_bits(8) {
                bytes[i] = byte;
            } else {
                return None;
            }
        }
        Some(u32::from_be_bytes(bytes))
    }


    fn byte_ix(&self) -> usize {
        self.ix / 8
    }

    /// This returns a bit index in current byte using standard
    /// indexing convention: i.e. 0 is the least significant bit, 7 is
    /// the most significant bit.
    fn bit_ix(&self) -> usize {
        7 - self.ix % 8
    }
}

/// BitWriter for dynamic Vector-based buffers.
pub struct BitWriter {
    /// Underlying buffer.
    buf: Vec<u8>,

    /// Index pointing to the current bit. Starts from the leftmost
    /// byte. Goes from the most signicant bit to the least
    /// significant bit of each byte.
    ix: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        Self {
            buf: vec![],
            ix: 0,
        }
    }

    /// Passes ownership of the internally built buffer to be used
    /// outside.
    pub fn dump(self) -> Vec<u8> {
        self.buf
    }

    /// Write u32 big endian style.
    pub fn write_u32_be(&mut self, x: u32) {
        for &b in x.to_be_bytes().iter() {
            self.write_bits(8, b);
        }
    }

    /// Write a single bit passed as `bool`.
    pub fn write_bit(&mut self, bit: bool) {
        if self.is_full() {
            self.buf.push(0);
        }

        let byte_ix = self.byte_ix();
        if bit {
            self.buf[byte_ix] |= 1 << self.bit_ix();
        } else {
            self.buf[byte_ix] &= !(1 << self.bit_ix());
        }

        self.ix += 1;
    }

    /// Write less than 8 bits passed in `u8`.
    pub fn write_bits(&mut self, num_of_bits: u8, data: u8) {
        assert!(num_of_bits <= 8);
        for offset in (0..num_of_bits).rev() {
            let bit = (data >> offset) & 1;
            self.write_bit(bit == 1);
        }
    }

    /// Index of the current byte.
    fn byte_ix(&self) -> usize {
        self.ix / 8
    }

    /// This returns a bit index in current byte using standard
    /// indexing convention: i.e. 0 is the least significant bit, 7 is
    /// the most significant bit.
    fn bit_ix(&self) -> usize {
        7 - self.ix % 8
    }

    /// If the buffer is full and there is no space to write anything.
    fn is_full(&self) -> bool {
        self.ix >= self.buf.len() * 8
    }
}
