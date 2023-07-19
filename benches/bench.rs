#![feature(test)]

extern crate test;

use blake3_guts as guts;
use guts::BLOCK_LEN;
use rand::prelude::*;
use test::Bencher;

const KIB: usize = 1024;

// This struct randomizes two things:
// 1. The actual bytes of input.
// 2. The page offset the input starts at.
pub struct RandomInput {
    buf: Vec<u8>,
    len: usize,
    offsets: Vec<usize>,
    offset_index: usize,
}

impl RandomInput {
    pub fn new(b: &mut Bencher, len: usize) -> Self {
        b.bytes += len as u64;
        let page_size: usize = page_size::get();
        let mut buf = vec![0u8; len + page_size];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut buf);
        let mut offsets: Vec<usize> = (0..page_size).collect();
        offsets.shuffle(&mut rng);
        Self {
            buf,
            len,
            offsets,
            offset_index: 0,
        }
    }

    pub fn get(&mut self) -> &[u8] {
        let offset = self.offsets[self.offset_index];
        self.offset_index += 1;
        if self.offset_index >= self.offsets.len() {
            self.offset_index = 0;
        }
        &self.buf[offset..][..self.len]
    }
}

fn bench_atonce(b: &mut Bencher, len: usize) {
    let mut input = RandomInput::new(b, len);
    b.iter(|| blake3::hash(input.get()));
}

#[bench]
fn bench_atonce_0001_block(b: &mut Bencher) {
    bench_atonce(b, BLOCK_LEN);
}

#[bench]
fn bench_atonce_0001_kib(b: &mut Bencher) {
    bench_atonce(b, 1 * KIB);
}

#[bench]
fn bench_atonce_0002_kib(b: &mut Bencher) {
    bench_atonce(b, 2 * KIB);
}

#[bench]
fn bench_atonce_0004_kib(b: &mut Bencher) {
    bench_atonce(b, 4 * KIB);
}

#[bench]
fn bench_atonce_0008_kib(b: &mut Bencher) {
    bench_atonce(b, 8 * KIB);
}

#[bench]
fn bench_atonce_0016_kib(b: &mut Bencher) {
    bench_atonce(b, 16 * KIB);
}

#[bench]
fn bench_atonce_0032_kib(b: &mut Bencher) {
    bench_atonce(b, 32 * KIB);
}

#[bench]
fn bench_atonce_0064_kib(b: &mut Bencher) {
    bench_atonce(b, 64 * KIB);
}

#[bench]
fn bench_atonce_0128_kib(b: &mut Bencher) {
    bench_atonce(b, 128 * KIB);
}

#[bench]
fn bench_atonce_0256_kib(b: &mut Bencher) {
    bench_atonce(b, 256 * KIB);
}

#[bench]
fn bench_atonce_0512_kib(b: &mut Bencher) {
    bench_atonce(b, 512 * KIB);
}

#[bench]
fn bench_atonce_1024_kib(b: &mut Bencher) {
    bench_atonce(b, 1024 * KIB);
}

fn bench_incremental(b: &mut Bencher, len: usize) {
    let mut input = RandomInput::new(b, len);
    b.iter(|| blake3::Hasher::new().update(input.get()).finalize());
}

#[bench]
fn bench_incremental_0001_block(b: &mut Bencher) {
    bench_incremental(b, BLOCK_LEN);
}

#[bench]
fn bench_incremental_0001_kib(b: &mut Bencher) {
    bench_incremental(b, 1 * KIB);
}

#[bench]
fn bench_incremental_0002_kib(b: &mut Bencher) {
    bench_incremental(b, 2 * KIB);
}

#[bench]
fn bench_incremental_0004_kib(b: &mut Bencher) {
    bench_incremental(b, 4 * KIB);
}

#[bench]
fn bench_incremental_0008_kib(b: &mut Bencher) {
    bench_incremental(b, 8 * KIB);
}

#[bench]
fn bench_incremental_0016_kib(b: &mut Bencher) {
    bench_incremental(b, 16 * KIB);
}

#[bench]
fn bench_incremental_0032_kib(b: &mut Bencher) {
    bench_incremental(b, 32 * KIB);
}

#[bench]
fn bench_incremental_0064_kib(b: &mut Bencher) {
    bench_incremental(b, 64 * KIB);
}

#[bench]
fn bench_incremental_0128_kib(b: &mut Bencher) {
    bench_incremental(b, 128 * KIB);
}

#[bench]
fn bench_incremental_0256_kib(b: &mut Bencher) {
    bench_incremental(b, 256 * KIB);
}

#[bench]
fn bench_incremental_0512_kib(b: &mut Bencher) {
    bench_incremental(b, 512 * KIB);
}

#[bench]
fn bench_incremental_1024_kib(b: &mut Bencher) {
    bench_incremental(b, 1024 * KIB);
}

fn bench_reference(b: &mut Bencher, len: usize) {
    let mut input = RandomInput::new(b, len);
    b.iter(|| {
        let mut hasher = reference_impl::Hasher::new();
        hasher.update(input.get());
        let mut out = [0; 32];
        hasher.finalize(&mut out);
        out
    });
}

#[bench]
fn bench_reference_0001_block(b: &mut Bencher) {
    bench_reference(b, BLOCK_LEN);
}

#[bench]
fn bench_reference_0001_kib(b: &mut Bencher) {
    bench_reference(b, 1 * KIB);
}

#[bench]
fn bench_reference_0002_kib(b: &mut Bencher) {
    bench_reference(b, 2 * KIB);
}

#[bench]
fn bench_reference_0004_kib(b: &mut Bencher) {
    bench_reference(b, 4 * KIB);
}

#[bench]
fn bench_reference_0008_kib(b: &mut Bencher) {
    bench_reference(b, 8 * KIB);
}

#[bench]
fn bench_reference_0016_kib(b: &mut Bencher) {
    bench_reference(b, 16 * KIB);
}

#[bench]
fn bench_reference_0032_kib(b: &mut Bencher) {
    bench_reference(b, 32 * KIB);
}

#[bench]
fn bench_reference_0064_kib(b: &mut Bencher) {
    bench_reference(b, 64 * KIB);
}

#[bench]
fn bench_reference_0128_kib(b: &mut Bencher) {
    bench_reference(b, 128 * KIB);
}

#[bench]
fn bench_reference_0256_kib(b: &mut Bencher) {
    bench_reference(b, 256 * KIB);
}

#[bench]
fn bench_reference_0512_kib(b: &mut Bencher) {
    bench_reference(b, 512 * KIB);
}

#[bench]
fn bench_reference_1024_kib(b: &mut Bencher) {
    bench_reference(b, 1024 * KIB);
}

#[cfg(feature = "rayon")]
fn bench_rayon(b: &mut Bencher, len: usize) {
    let mut input = RandomInput::new(b, len);
    b.iter(|| blake3::Hasher::new().update_rayon(input.get()).finalize());
}

#[bench]
#[cfg(feature = "rayon")]
fn bench_rayon_0001_block(b: &mut Bencher) {
    bench_rayon(b, BLOCK_LEN);
}

#[bench]
#[cfg(feature = "rayon")]
fn bench_rayon_0001_kib(b: &mut Bencher) {
    bench_rayon(b, 1 * KIB);
}

#[bench]
#[cfg(feature = "rayon")]
fn bench_rayon_0002_kib(b: &mut Bencher) {
    bench_rayon(b, 2 * KIB);
}

#[bench]
#[cfg(feature = "rayon")]
fn bench_rayon_0004_kib(b: &mut Bencher) {
    bench_rayon(b, 4 * KIB);
}

#[bench]
#[cfg(feature = "rayon")]
fn bench_rayon_0008_kib(b: &mut Bencher) {
    bench_rayon(b, 8 * KIB);
}

#[bench]
#[cfg(feature = "rayon")]
fn bench_rayon_0016_kib(b: &mut Bencher) {
    bench_rayon(b, 16 * KIB);
}

#[bench]
#[cfg(feature = "rayon")]
fn bench_rayon_0032_kib(b: &mut Bencher) {
    bench_rayon(b, 32 * KIB);
}

#[bench]
#[cfg(feature = "rayon")]
fn bench_rayon_0064_kib(b: &mut Bencher) {
    bench_rayon(b, 64 * KIB);
}

#[bench]
#[cfg(feature = "rayon")]
fn bench_rayon_0128_kib(b: &mut Bencher) {
    bench_rayon(b, 128 * KIB);
}

#[bench]
#[cfg(feature = "rayon")]
fn bench_rayon_0256_kib(b: &mut Bencher) {
    bench_rayon(b, 256 * KIB);
}

#[bench]
#[cfg(feature = "rayon")]
fn bench_rayon_0512_kib(b: &mut Bencher) {
    bench_rayon(b, 512 * KIB);
}

#[bench]
#[cfg(feature = "rayon")]
fn bench_rayon_1024_kib(b: &mut Bencher) {
    bench_rayon(b, 1024 * KIB);
}

// This checks that update() splits up its input in increasing powers of 2, so
// that it can recover a high degree of parallelism when the number of bytes
// hashed so far is uneven. The performance of this benchmark should be
// reasonably close to bench_incremental_0064_kib, within 80% or so. When we
// had a bug in this logic (https://github.com/BLAKE3-team/BLAKE3/issues/69),
// performance was less than half.
#[bench]
fn bench_two_updates(b: &mut Bencher) {
    let len = 65536;
    let mut input = RandomInput::new(b, len);
    b.iter(|| {
        let mut hasher = blake3::Hasher::new();
        let input = input.get();
        hasher.update(&input[..1]);
        hasher.update(&input[1..]);
        hasher.finalize()
    });
}

fn bench_hash_chunks(b: &mut Bencher, len: usize) {
    if len > guts::DETECTED_IMPL.degree() * guts::CHUNK_LEN {
        return;
    }
    let mut input = RandomInput::new(b, len);
    let key = [99; 32];
    let mut output = guts::TransposedVectors::new();
    b.iter(|| {
        let (output_left, _) = guts::DETECTED_IMPL.split_transposed_vectors(&mut output);
        guts::DETECTED_IMPL.hash_chunks(input.get(), &key, 0, 0, output_left);
    });
}

#[bench]
fn bench_hash_chunks_01_kib(b: &mut Bencher) {
    bench_hash_chunks(b, 1024);
}

#[bench]
fn bench_hash_chunks_02_kib(b: &mut Bencher) {
    bench_hash_chunks(b, 2048);
}

#[bench]
fn bench_hash_chunks_04_kib(b: &mut Bencher) {
    bench_hash_chunks(b, 4096);
}

#[bench]
fn bench_hash_chunks_08_kib(b: &mut Bencher) {
    bench_hash_chunks(b, 8192);
}

#[bench]
fn bench_hash_chunks_16_kib(b: &mut Bencher) {
    bench_hash_chunks(b, 16384);
}

fn bench_hash_parents(b: &mut Bencher, num_parents: usize) {
    if num_parents > guts::DETECTED_IMPL.degree() {
        return;
    }
    b.bytes = 64 * num_parents as u64;
    let num_cvs = 2 * num_parents;
    let key = [99; 32];
    let mut output = guts::TransposedVectors::new();
    b.iter(|| {
        guts::DETECTED_IMPL.reduce_parents(&mut output, num_cvs, &key, 0);
    });
}

#[bench]
fn bench_hash_parents_02(b: &mut Bencher) {
    bench_hash_parents(b, 2);
}

#[bench]
fn bench_hash_parents_04(b: &mut Bencher) {
    bench_hash_parents(b, 4);
}

#[bench]
fn bench_hash_parents_08(b: &mut Bencher) {
    bench_hash_parents(b, 8);
}

#[bench]
fn bench_hash_parents_16(b: &mut Bencher) {
    bench_hash_parents(b, 16);
}

fn bench_xof(b: &mut Bencher, len: usize) {
    b.bytes = len as u64;
    let mut output = [0u8; 65536];
    let output_slice = &mut output[..len];
    let mut reader = blake3::Hasher::new().finalize_xof();
    b.iter(|| reader.fill(output_slice));
}

#[bench]
fn bench_xof_0064(b: &mut Bencher) {
    bench_xof(b, 64);
}

#[bench]
fn bench_xof_0128(b: &mut Bencher) {
    bench_xof(b, 128);
}

#[bench]
fn bench_xof_0256(b: &mut Bencher) {
    bench_xof(b, 256);
}

#[bench]
fn bench_xof_0512(b: &mut Bencher) {
    bench_xof(b, 512);
}

#[bench]
fn bench_xof_1024(b: &mut Bencher) {
    bench_xof(b, 1024);
}

fn bench_universal_hash(b: &mut Bencher, len: usize) {
    let mut input = RandomInput::new(b, len);
    let key = [99; 32];
    b.iter(|| guts::DETECTED_IMPL.universal_hash(input.get(), &key, 0));
}

#[bench]
fn bench_universal_hash_0064(b: &mut Bencher) {
    bench_universal_hash(b, 64);
}

#[bench]
fn bench_universal_hash_0128(b: &mut Bencher) {
    bench_universal_hash(b, 128);
}

#[bench]
fn bench_universal_hash_0256(b: &mut Bencher) {
    bench_universal_hash(b, 256);
}

#[bench]
fn bench_universal_hash_0512(b: &mut Bencher) {
    bench_universal_hash(b, 512);
}

#[bench]
fn bench_universal_hash_1024(b: &mut Bencher) {
    bench_universal_hash(b, 1024);
}
