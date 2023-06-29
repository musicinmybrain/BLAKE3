use atomic::{Atomic, Ordering::Relaxed};
use core::cmp;
use core::ptr;

use crate::portable2;

pub const BLOCK_LEN: usize = 64;
pub const CHUNK_LEN: usize = 1024;
pub const WORD_LEN: usize = 4;
pub const UNIVERSAL_HASH_LEN: usize = 16;

pub const CHUNK_START: u32 = 1 << 0;
pub const CHUNK_END: u32 = 1 << 1;
pub const PARENT: u32 = 1 << 2;
pub const ROOT: u32 = 1 << 3;
pub const KEYED_HASH: u32 = 1 << 4;
pub const DERIVE_KEY_CONTEXT: u32 = 1 << 5;
pub const DERIVE_KEY_MATERIAL: u32 = 1 << 6;

pub const IV: [u32; 8] = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];

pub const MSG_SCHEDULE: [[usize; 16]; 7] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8],
    [3, 4, 10, 12, 13, 2, 7, 14, 6, 5, 9, 0, 11, 15, 8, 1],
    [10, 7, 12, 9, 14, 3, 13, 15, 4, 0, 11, 2, 5, 8, 1, 6],
    [12, 13, 9, 11, 15, 10, 14, 8, 7, 2, 5, 3, 0, 1, 6, 4],
    [9, 14, 11, 5, 8, 12, 15, 1, 13, 3, 0, 10, 2, 6, 4, 7],
    [11, 15, 5, 0, 1, 9, 8, 6, 14, 10, 2, 12, 3, 4, 7, 13],
];

cfg_if::cfg_if! {
    if #[cfg(any(target_arch = "x86", target_arch = "x86_64"))] {
        pub const MAX_SIMD_DEGREE: usize = 16;
    } else if #[cfg(blake3_neon)] {
        pub const MAX_SIMD_DEGREE: usize = 4;
    } else {
        pub const MAX_SIMD_DEGREE: usize = 1;
    }
}

pub struct Implementation {
    degree_fn: Atomic<DegreeFn>,
    compress_fn: Atomic<CompressFn>,
    hash_chunks_fn: Atomic<HashChunksFn>,
    hash_parents_fn: Atomic<HashParentsFn>,
    xof_fn: Atomic<XofFn>,
    xof_xor_fn: Atomic<XofFn>,
    universal_hash_fn: Atomic<UniversalHashFn>,
}

impl Implementation {
    pub fn detect() -> Self {
        Self::portable()
    }

    pub fn portable() -> Self {
        Self {
            degree_fn: Atomic::new(|| portable2::DEGREE),
            compress_fn: Atomic::new(portable2::compress),
            hash_chunks_fn: Atomic::new(portable2::hash_chunks),
            hash_parents_fn: Atomic::new(portable2::hash_parents),
            xof_fn: Atomic::new(portable2::xof),
            xof_xor_fn: Atomic::new(portable2::xof_xor),
            universal_hash_fn: Atomic::new(portable2::universal_hash),
        }
    }

    fn assign(&self, other: &Self) {
        self.degree_fn.store(other.degree_fn.load(Relaxed), Relaxed);
        self.compress_fn
            .store(other.compress_fn.load(Relaxed), Relaxed);
        self.hash_chunks_fn
            .store(other.hash_chunks_fn.load(Relaxed), Relaxed);
        self.hash_parents_fn
            .store(other.hash_parents_fn.load(Relaxed), Relaxed);
        self.xof_fn.store(other.xof_fn.load(Relaxed), Relaxed);
        self.xof_xor_fn
            .store(other.xof_xor_fn.load(Relaxed), Relaxed);
        self.universal_hash_fn
            .store(other.universal_hash_fn.load(Relaxed), Relaxed);
    }

    pub fn degree(&self) -> usize {
        self.degree_fn.load(Relaxed)()
    }

    pub fn compress(
        &self,
        block: &[u8; 64],
        block_len: u32,
        cv: &[u32; 8],
        counter: u64,
        flags: u32,
    ) -> [u32; 16] {
        let mut out = [0u32; 16];
        unsafe {
            self.compress_fn.load(Relaxed)(block, block_len, cv, counter, flags, &mut out);
        }
        out
    }

    pub fn hash_chunks(
        &self,
        input: &[u8],
        key: &[u32; 8],
        counter: u64,
        flags: u32,
        transposed_output: &mut TransposedVectors,
    ) {
        debug_assert!(input.len() > 0);
        debug_assert!(input.len() <= self.degree() * CHUNK_LEN);
        debug_assert!(transposed_output.len == 0 || transposed_output.len == self.degree());
        let output_offset = if transposed_output.len == 0 {
            0
        } else {
            self.degree()
        };
        unsafe {
            self.hash_chunks_fn.load(Relaxed)(
                input.as_ptr(),
                input.len(),
                key,
                counter,
                flags,
                transposed_output[0].as_mut_ptr().add(output_offset),
            );
        }
    }

    pub fn hash_parents(
        &self,
        transposed_input: &TransposedVectors,
        key: &[u32; 8],
        flags: u32,
        transposed_output: &mut TransposedVectors,
    ) {
        debug_assert!(transposed_output.len == 0 || transposed_output.len == self.degree());
        let output_offset = if transposed_output.len == 0 {
            0
        } else {
            self.degree()
        };
        let num_parents = transposed_input.len / 2;
        unsafe {
            self.hash_parents_fn.load(Relaxed)(
                transposed_input[0].as_ptr(),
                num_parents,
                key,
                flags,
                transposed_output[0].as_mut_ptr().add(output_offset),
            );
        }
        transposed_output.len += num_parents;
        if transposed_input.len % 2 == 1 {
            for word_index in 0..8 {
                transposed_output[word_index][transposed_input.len] =
                    transposed_input[word_index][transposed_input.len - 1];
            }
            transposed_output.len += 1;
        }
    }

    pub fn reduce_parents(
        &self,
        transposed_io: &mut TransposedVectors,
        key: &[u32; 8],
        flags: u32,
    ) {
        let len = transposed_io.len;
        let num_parents = len / 2;
        unsafe {
            self.hash_parents_fn.load(Relaxed)(
                transposed_io[0].as_ptr(),
                num_parents,
                key,
                flags,
                transposed_io[0].as_mut_ptr(),
            );
        }
        if len % 2 == 1 {
            for word_index in 0..8 {
                transposed_io[word_index][len / 2] = transposed_io[word_index][len - 1];
            }
            transposed_io.len = len / 2 + 1;
        } else {
            transposed_io.len = len / 2;
        }
    }

    pub fn xof(
        &self,
        block: &[u8; 64],
        block_len: u32,
        cv: &[u32; 8],
        counter: u64,
        flags: u32,
        out: &mut [u8],
    ) {
        unsafe {
            self.xof_fn.load(Relaxed)(
                block,
                block_len,
                cv,
                counter,
                flags,
                out.as_mut_ptr(),
                out.len(),
            );
        }
    }

    pub fn xof_xor(
        &self,
        block: &[u8; 64],
        block_len: u32,
        cv: &[u32; 8],
        counter: u64,
        flags: u32,
        out: &mut [u8],
    ) {
        unsafe {
            self.xof_xor_fn.load(Relaxed)(
                block,
                block_len,
                cv,
                counter,
                flags,
                out.as_mut_ptr(),
                out.len(),
            );
        }
    }

    pub fn universal_hash(&self, input: &[u8], key: &[u32; 8], counter: u64) -> [u8; 16] {
        let mut out = [0u8; 16];
        unsafe {
            self.universal_hash_fn.load(Relaxed)(
                input.as_ptr(),
                input.len(),
                key,
                counter,
                &mut out,
            );
        }
        out
    }
}

impl Clone for Implementation {
    fn clone(&self) -> Self {
        Self {
            degree_fn: Atomic::new(self.degree_fn.load(Relaxed)),
            compress_fn: Atomic::new(self.compress_fn.load(Relaxed)),
            hash_chunks_fn: Atomic::new(self.hash_chunks_fn.load(Relaxed)),
            hash_parents_fn: Atomic::new(self.hash_parents_fn.load(Relaxed)),
            xof_fn: Atomic::new(self.xof_fn.load(Relaxed)),
            xof_xor_fn: Atomic::new(self.xof_xor_fn.load(Relaxed)),
            universal_hash_fn: Atomic::new(self.universal_hash_fn.load(Relaxed)),
        }
    }
}

pub static DETECTED_IMPL: Implementation = Implementation {
    degree_fn: Atomic::new(degree_init),
    compress_fn: Atomic::new(compress_init),
    hash_chunks_fn: Atomic::new(hash_chunks_init),
    hash_parents_fn: Atomic::new(hash_parents_init),
    xof_fn: Atomic::new(xof_init),
    xof_xor_fn: Atomic::new(xof_xor_init),
    universal_hash_fn: Atomic::new(universal_hash_init),
};

type DegreeFn = fn() -> usize;

fn degree_init() -> usize {
    DETECTED_IMPL.assign(&Implementation::detect());
    DETECTED_IMPL.degree_fn.load(Relaxed)()
}

type CompressFn = unsafe extern "C" fn(
    block: *const [u8; 64], // zero padded to 64 bytes
    block_len: u32,
    cv: *const [u32; 8],
    counter: u64,
    flags: u32,
    out: *mut [u32; 16], // may overlap the input
);

unsafe extern "C" fn compress_init(
    block: *const [u8; 64],
    block_len: u32,
    cv: *const [u32; 8],
    counter: u64,
    flags: u32,
    out: *mut [u32; 16],
) {
    DETECTED_IMPL.assign(&Implementation::detect());
    DETECTED_IMPL.compress_fn.load(Relaxed)(block, block_len, cv, counter, flags, out);
}

type HashChunksFn = unsafe extern "C" fn(
    input: *const u8,
    input_len: usize,
    key: *const [u32; 8],
    counter: u64,
    flags: u32,
    transposed_output: *mut u32,
);

unsafe extern "C" fn hash_chunks_init(
    input: *const u8,
    input_len: usize,
    key: *const [u32; 8],
    counter: u64,
    flags: u32,
    transposed_output: *mut u32,
) {
    DETECTED_IMPL.assign(&Implementation::detect());
    DETECTED_IMPL.hash_chunks_fn.load(Relaxed)(
        input,
        input_len,
        key,
        counter,
        flags,
        transposed_output,
    );
}

type HashParentsFn = unsafe extern "C" fn(
    transposed_input: *const u32,
    num_parents: usize,
    key: *const [u32; 8],
    flags: u32,
    transposed_output: *mut u32, // may overlap the input
);

unsafe extern "C" fn hash_parents_init(
    transposed_input: *const u32,
    num_parents: usize,
    key: *const [u32; 8],
    flags: u32,
    transposed_output: *mut u32,
) {
    DETECTED_IMPL.assign(&Implementation::detect());
    DETECTED_IMPL.hash_parents_fn.load(Relaxed)(
        transposed_input,
        num_parents,
        key,
        flags,
        transposed_output,
    );
}

// This signature covers both xof() and xof_xor().
type XofFn = unsafe extern "C" fn(
    block: *const [u8; 64], // zero padded to 64 bytes
    block_len: u32,
    cv: *const [u32; 8],
    counter: u64,
    flags: u32,
    out: *mut u8,
    out_len: usize,
);

unsafe extern "C" fn xof_init(
    block: *const [u8; 64],
    block_len: u32,
    cv: *const [u32; 8],
    counter: u64,
    flags: u32,
    out: *mut u8,
    out_len: usize,
) {
    DETECTED_IMPL.assign(&Implementation::detect());
    DETECTED_IMPL.xof_fn.load(Relaxed)(block, block_len, cv, counter, flags, out, out_len);
}

unsafe extern "C" fn xof_xor_init(
    block: *const [u8; 64],
    block_len: u32,
    cv: *const [u32; 8],
    counter: u64,
    flags: u32,
    out: *mut u8,
    out_len: usize,
) {
    DETECTED_IMPL.assign(&Implementation::detect());
    DETECTED_IMPL.xof_xor_fn.load(Relaxed)(block, block_len, cv, counter, flags, out, out_len);
}

type UniversalHashFn = unsafe extern "C" fn(
    input: *const u8,
    input_len: usize,
    key: *const [u32; 8],
    counter: u64,
    out: *mut [u8; 16],
);

unsafe extern "C" fn universal_hash_init(
    input: *const u8,
    input_len: usize,
    key: *const [u32; 8],
    counter: u64,
    out: *mut [u8; 16],
) {
    DETECTED_IMPL.assign(&Implementation::detect());
    DETECTED_IMPL.universal_hash_fn.load(Relaxed)(input, input_len, key, counter, out);
}

// The implicit degree of this implementation is MAX_SIMD_DEGREE.
pub(crate) unsafe fn hash_chunks_using_compress(
    compress: CompressFn,
    mut input: *const u8,
    mut input_len: usize,
    key: *const [u32; 8],
    mut counter: u64,
    flags: u32,
    mut transposed_output: *mut u32,
) {
    debug_assert!(input_len > 0);
    debug_assert!(input_len <= MAX_SIMD_DEGREE * CHUNK_LEN);
    while input_len > 0 {
        let mut chunk_len = cmp::min(input_len, CHUNK_LEN);
        input_len -= chunk_len;
        // We only use 8 words of the CV, but compress returns 16.
        let mut cv = [0u32; 16];
        cv[..8].copy_from_slice(&*key);
        let cv_ptr: *mut [u32; 16] = &mut cv;
        let mut chunk_flags = flags | CHUNK_START;
        while chunk_len > BLOCK_LEN {
            compress(
                input as *const [u8; 64],
                BLOCK_LEN as u32,
                cv_ptr as *const [u32; 8],
                counter,
                chunk_flags,
                cv_ptr,
            );
            input = input.add(BLOCK_LEN);
            chunk_len -= BLOCK_LEN;
            chunk_flags &= !CHUNK_START;
        }
        let mut last_block = [0u8; BLOCK_LEN];
        ptr::copy_nonoverlapping(input, last_block.as_mut_ptr(), chunk_len);
        input = input.add(chunk_len);
        compress(
            &last_block,
            chunk_len as u32,
            cv_ptr as *const [u32; 8],
            counter,
            chunk_flags | CHUNK_END,
            cv_ptr,
        );
        for word_index in 0..8 {
            transposed_output
                .add(word_index * TRANSPOSED_STRIDE)
                .write(cv[word_index]);
        }
        transposed_output = transposed_output.add(1);
        counter += 1;
    }
}

// The implicit degree of this implementation is MAX_SIMD_DEGREE.
pub(crate) unsafe fn hash_parents_using_compress(
    compress: CompressFn,
    mut transposed_input: *const u32,
    mut num_parents: usize,
    key: *const [u32; 8],
    flags: u32,
    mut transposed_output: *mut u32, // may overlap the input
) {
    debug_assert!(num_parents > 0);
    debug_assert!(num_parents <= MAX_SIMD_DEGREE);
    while num_parents > 0 {
        let mut block_bytes = [0u8; 64];
        for word_index in 0..8 {
            let left_child_word = transposed_input.add(word_index * TRANSPOSED_STRIDE).read();
            block_bytes[WORD_LEN * word_index..][..WORD_LEN]
                .copy_from_slice(&left_child_word.to_le_bytes());
            let right_child_word = transposed_input
                .add(word_index * TRANSPOSED_STRIDE + 1)
                .read();
            block_bytes[WORD_LEN * (word_index + 8)..][..WORD_LEN]
                .copy_from_slice(&right_child_word.to_le_bytes());
        }
        let mut cv = [0u32; 16];
        compress(&block_bytes, BLOCK_LEN as u32, key, 0, flags, &mut cv);
        for word_index in 0..8 {
            transposed_output
                .add(word_index * TRANSPOSED_STRIDE)
                .write(cv[word_index]);
        }
        transposed_input = transposed_input.add(2);
        transposed_output = transposed_output.add(1);
        num_parents -= 1;
    }
}

pub(crate) unsafe fn xof_using_compress(
    compress: CompressFn,
    block: *const [u8; 64],
    block_len: u32,
    cv: *const [u32; 8],
    mut counter: u64,
    flags: u32,
    mut out: *mut u8,
    mut out_len: usize,
) {
    while out_len > 0 {
        let mut block_output = [0u32; 16];
        compress(block, block_len, cv, counter, flags, &mut block_output);
        for output_word in block_output {
            let bytes = output_word.to_le_bytes();
            let take = cmp::min(bytes.len(), out_len);
            ptr::copy_nonoverlapping(bytes.as_ptr(), out, take);
            out = out.add(take);
            out_len -= take;
        }
        counter += 1;
    }
}

pub(crate) unsafe fn xof_xor_using_compress(
    compress: CompressFn,
    block: *const [u8; 64],
    block_len: u32,
    cv: *const [u32; 8],
    mut counter: u64,
    flags: u32,
    mut out: *mut u8,
    mut out_len: usize,
) {
    while out_len > 0 {
        let mut block_output = [0u32; 16];
        compress(block, block_len, cv, counter, flags, &mut block_output);
        for output_word in block_output {
            let bytes = output_word.to_le_bytes();
            for i in 0..cmp::min(bytes.len(), out_len) {
                *out = *out ^ bytes[i];
                out = out.add(1);
                out_len -= 1;
            }
        }
        counter += 1;
    }
}

pub(crate) unsafe fn universal_hash_using_compress(
    compress: CompressFn,
    mut input: *const u8,
    mut input_len: usize,
    key: *const [u32; 8],
    mut counter: u64,
    out: *mut [u8; 16],
) {
    let flags = KEYED_HASH | CHUNK_START | CHUNK_END | ROOT;
    let mut result = [0u32; 4];
    while input_len > 0 {
        let block_len = cmp::min(input_len, BLOCK_LEN);
        let mut block = [0u8; BLOCK_LEN];
        ptr::copy_nonoverlapping(input, block.as_mut_ptr(), block_len);
        let mut block_output = [0u32; 16];
        compress(
            &block,
            BLOCK_LEN as u32,
            key,
            counter,
            flags,
            &mut block_output,
        );
        for i in 0..4 {
            result[i] ^= block_output[i];
        }
        input = input.add(block_len);
        input_len -= block_len;
        counter += 1;
    }
    for i in 0..4 {
        (*out)[WORD_LEN * i..][..WORD_LEN].copy_from_slice(&result[i].to_le_bytes());
    }
}

// this is in units of _32-bit words_, for pointer operations on *const/mut u32
pub const TRANSPOSED_STRIDE: usize = 2 * MAX_SIMD_DEGREE;

#[cfg_attr(any(target_arch = "x86", target_arch = "x86_64"), repr(C, align(64)))]
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct TransposedVectors {
    vectors: [[u32; 2 * MAX_SIMD_DEGREE]; 8],
    len: usize, // the number of CVs populated in each vector
}

impl core::ops::Index<usize> for TransposedVectors {
    type Output = [u32];

    fn index(&self, i: usize) -> &[u32] {
        &self.vectors[i][..self.len]
    }
}

impl core::ops::IndexMut<usize> for TransposedVectors {
    fn index_mut(&mut self, i: usize) -> &mut [u32] {
        &mut self.vectors[i][..self.len]
    }
}
