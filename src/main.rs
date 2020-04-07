mod bits;
use bits::{BitReader, BitWriter};

use log::debug;

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::env;
use std::fs::File;
use std::io::{stdin, stdout, Read, Write};
use std::mem::MaybeUninit;

#[cfg(test)]
extern crate quickcheck;
#[cfg(test)]
#[macro_use(quickcheck)]
extern crate quickcheck_macros;

#[derive(Debug)]
struct Node {
    byte: u8,
    freq: usize,
    children: Option<Box<(Node, Node)>>,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.freq == other.freq
    }
}

impl Eq for Node {}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        self.freq.cmp(&other.freq).reverse()
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Node {
    fn leaf(byte: u8, freq: usize) -> Self {
        Node {
            byte,
            freq,
            children: None,
        }
    }

    fn merge(l: Node, r: Node) -> Node {
        Node {
            byte: 0,
            freq: l.freq + r.freq,
            children: Some(Box::new((l, r))),
        }
    }
}

fn freq_table(data: &[u8]) -> [usize; 256] {
    let mut counter = [0; 256];
    for &b in data {
        counter[b as usize] += 1;
    }
    counter
}

fn build_trie(freq_table: [usize; 256]) -> Node {
    let mut heap = BinaryHeap::with_capacity(256);
    for (b, &freq) in freq_table.iter().enumerate() {
        if freq > 0 {
            heap.push(Node::leaf(b as u8, freq));
        }
    }

    while heap.len() > 1 {
        // Merge two smallest nodes and push the result back.
        let l = heap.pop().unwrap();
        let r = heap.pop().unwrap();
        heap.push(Node::merge(l, r));
    }

    heap.pop().unwrap_or(Node::leaf(0, 0))
}

/// We write the tree by walking it in pre-order. For every internal
/// node we write one bit: '0'. For every leaf node we write '1'
/// followed by eight bits of its character, so it's nine bits in
/// total. With this format we can easily reconstruct the trie later.
fn write_trie(writer: &mut BitWriter, node: &Node) {
    match &node.children {
        None => {
            debug!("write_trie: writing leaf: {}", node.byte);
            writer.write_bit(true);
            writer.write_bits(8, node.byte);
        }
        Some(children) => {
            debug!("write_trie: writing internal node");
            writer.write_bit(false);
            write_trie(writer, &children.0);
            write_trie(writer, &children.1);
        }
    }
}

/// Reading trie by pre-order traversal. See `write_trie` docs for
/// details of format.
fn read_trie(reader: &mut BitReader) -> Node {
    if reader
        .read_bit()
        .expect("read_trie: can't decode node type")
    {
        let byte = reader
            .read_bits(8)
            .expect("read_trie: can't read data from node");
        debug!("read_trie: reading leaf {}", byte);
        Node::leaf(byte, 0)
    } else {
        debug!("read_trie: reading internal node");
        let left = read_trie(reader);
        let right = read_trie(reader);
        Node::merge(left, right)
    }
}

fn build_code(trie: &Node) -> [Option<Box<Vec<u8>>>; 256] {
    fn go(t: &Node, path: Vec<u8>, table: &mut [Option<Box<Vec<u8>>>; 256]) {
        match t.children {
            None => {
                table[t.byte as usize] = Some(Box::new(path));
            }
            Some(ref ch) => {
                let mut left_path = path.clone();
                left_path.push(0);
                let mut right_path = path.clone();
                right_path.push(1);
                go(&ch.0, left_path, table);
                go(&ch.1, right_path, table);
            }
        }
    }

    // A somewhat hacky way to initialize array with 256 Nones, but
    // just using [None; 256] doesn't work.
    let mut table = {
        let mut data: [MaybeUninit<Option<Box<Vec<u8>>>>; 256] =
            unsafe { MaybeUninit::uninit().assume_init() };
        for elem in &mut data[..] {
            unsafe {
                std::ptr::write(elem.as_mut_ptr(), None);
            }
        }
        unsafe { std::mem::transmute(data) }
    };

    go(trie, vec![], &mut table);

    table
}

fn write_encoded_data(writer: &mut BitWriter, data: &[u8], code: [Option<Box<Vec<u8>>>; 256]) {
    writer.write_u32_be(data.len() as u32);
    debug!("write_encoded_data: writing size of data: {}", data.len());
    for &d in data {
        let code_entry = code[d as usize].as_ref().unwrap();
        debug!(
            "write_encoded_data: writing byte {} using code {:?}",
            d, &code_entry
        );
        for &b in code_entry.iter() {
            writer.write_bit(b == 1);
        }
    }
}

fn read_decoded_data(reader: &mut BitReader, trie: Node) -> Vec<u8> {
    let size = reader
        .read_u32_be()
        .expect("decode_data: can't read size of data");

    debug!("read_decoded_data: reading size of data: {}", size);

    fn go(reader: &mut BitReader, node: &Node) -> u8 {
        match node.children {
            None => node.byte,
            Some(ref ch) => {
                if reader
                    .read_bit()
                    .expect("read_decoded_data: unexpected end of data")
                {
                    go(reader, &ch.1)
                } else {
                    go(reader, &ch.0)
                }
            }
        }
    }

    (0..size).map(|_| go(reader, &trie)).collect()
}

fn compress(data: &[u8]) -> Vec<u8> {
    let freqs = freq_table(&data);
    let trie = build_trie(freqs);
    let code = build_code(&trie);
    let mut writer = BitWriter::new();
    write_trie(&mut writer, &trie);
    write_encoded_data(&mut writer, data, code);
    writer.dump()
}

fn extract(data: &[u8]) -> Vec<u8> {
    let mut reader = BitReader::new(data);
    let trie = read_trie(&mut reader);
    read_decoded_data(&mut reader, trie)
}

fn print_usage() {
    eprintln!("usage: huffman <mode> <file>");
    eprintln!("       where <mode> is either `x` for extract or `c` for compress");
    eprintln!("         and <file> is either `-` for stdin or filename");
    std::process::exit(1);
}

fn main() -> std::io::Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        print_usage();
    }

    let mode = &args[1];
    let file_name = &args[2];

    // Read input data.
    let mut data = vec![];
    if file_name == "-" {
        stdin().read_to_end(&mut data)?;
    } else {
        let mut file = File::open(file_name)?;
        file.read_to_end(&mut data)?;
    }

    // Print result to stdout.
    let result;
    if mode == "x" {
        result = extract(&data);
        stdout().write(result.as_slice())?;
    } else if mode == "c" {
        result = compress(&data);
        stdout().write(result.as_slice())?;
    } else {
        print_usage();
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[quickcheck]
    fn test_encode_decode_identity(bytes: Vec<u8>) -> bool {
        let compressed = compress(&bytes[..]);
        let extracted = extract(&compressed[..]);
        bytes == extracted
    }
}
