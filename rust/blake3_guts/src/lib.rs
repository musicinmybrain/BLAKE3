use core::cmp;
use core::marker::PhantomData;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering::Relaxed};

mod portable;

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

pub const IV: CVWords = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];
pub const IV_BYTES: CVBytes = le_bytes_from_words_32(&IV);

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
        // never less than 2
        pub const MAX_SIMD_DEGREE: usize = 2;
    }
}

pub type CVBytes = [u8; 32];
pub type CVWords = [u32; 8];
pub type BlockBytes = [u8; 64];
pub type BlockWords = [u32; 16];

#[inline]
pub fn degree() -> usize {
    DETECTED_IMPL.degree()
}

#[inline]
pub fn split_transposed_vectors(
    vectors: &mut TransposedVectors,
) -> (TransposedSplit, TransposedSplit) {
    DETECTED_IMPL.split_transposed_vectors(vectors)
}

#[inline]
pub fn compress(
    block: &BlockBytes,
    block_len: u32,
    cv: &CVBytes,
    counter: u64,
    flags: u32,
) -> CVBytes {
    DETECTED_IMPL.compress(block, block_len, cv, counter, flags)
}

#[inline]
pub fn hash_chunks(
    input: &[u8],
    key: &CVBytes,
    counter: u64,
    flags: u32,
    transposed_output: TransposedSplit,
) -> usize {
    DETECTED_IMPL.hash_chunks(input, key, counter, flags, transposed_output)
}

#[inline]
pub fn hash_parents(
    transposed_input: &TransposedVectors,
    num_cvs: usize,
    key: &CVBytes,
    flags: u32,
    transposed_output: TransposedSplit,
) -> usize {
    DETECTED_IMPL.hash_parents(transposed_input, num_cvs, key, flags, transposed_output)
}

#[inline]
pub fn reduce_parents(
    transposed_in_out: &mut TransposedVectors,
    num_cvs: usize,
    key: &CVBytes,
    flags: u32,
) -> usize {
    DETECTED_IMPL.reduce_parents(transposed_in_out, num_cvs, key, flags)
}

#[inline]
pub fn xof(
    block: &BlockBytes,
    block_len: u32,
    cv: &CVBytes,
    counter: u64,
    flags: u32,
    out: &mut [u8],
) {
    DETECTED_IMPL.xof(block, block_len, cv, counter, flags, out);
}

#[inline]
pub fn xof_xor(
    block: &BlockBytes,
    block_len: u32,
    cv: &CVBytes,
    counter: u64,
    flags: u32,
    out: &mut [u8],
) {
    DETECTED_IMPL.xof_xor(block, block_len, cv, counter, flags, out);
}

#[inline]
pub fn universal_hash(input: &[u8], key: &CVBytes, counter: u64) -> [u8; 16] {
    DETECTED_IMPL.universal_hash(input, key, counter)
}

static DETECTED_IMPL: Implementation = Implementation::new(
    degree_init,
    compress_init,
    hash_chunks_init,
    hash_parents_init,
    xof_init,
    xof_xor_init,
    universal_hash_init,
);

fn init_detected_impl() {
    let detected = Implementation::portable();

    DETECTED_IMPL
        .degree_ptr
        .store(detected.degree_ptr.load(Relaxed), Relaxed);
    DETECTED_IMPL
        .compress_ptr
        .store(detected.compress_ptr.load(Relaxed), Relaxed);
    DETECTED_IMPL
        .hash_chunks_ptr
        .store(detected.hash_chunks_ptr.load(Relaxed), Relaxed);
    DETECTED_IMPL
        .hash_parents_ptr
        .store(detected.hash_parents_ptr.load(Relaxed), Relaxed);
    DETECTED_IMPL
        .xof_ptr
        .store(detected.xof_ptr.load(Relaxed), Relaxed);
    DETECTED_IMPL
        .xof_xor_ptr
        .store(detected.xof_xor_ptr.load(Relaxed), Relaxed);
    DETECTED_IMPL
        .universal_hash_ptr
        .store(detected.universal_hash_ptr.load(Relaxed), Relaxed);
}

pub struct Implementation {
    degree_ptr: AtomicPtr<()>,
    compress_ptr: AtomicPtr<()>,
    hash_chunks_ptr: AtomicPtr<()>,
    hash_parents_ptr: AtomicPtr<()>,
    xof_ptr: AtomicPtr<()>,
    xof_xor_ptr: AtomicPtr<()>,
    universal_hash_ptr: AtomicPtr<()>,
}

impl Implementation {
    const fn new(
        degree_fn: DegreeFn,
        compress_fn: CompressFn,
        hash_chunks_fn: HashChunksFn,
        hash_parents_fn: HashParentsFn,
        xof_fn: XofFn,
        xof_xor_fn: XofFn,
        universal_hash_fn: UniversalHashFn,
    ) -> Self {
        Self {
            degree_ptr: AtomicPtr::new(degree_fn as *mut ()),
            compress_ptr: AtomicPtr::new(compress_fn as *mut ()),
            hash_chunks_ptr: AtomicPtr::new(hash_chunks_fn as *mut ()),
            hash_parents_ptr: AtomicPtr::new(hash_parents_fn as *mut ()),
            xof_ptr: AtomicPtr::new(xof_fn as *mut ()),
            xof_xor_ptr: AtomicPtr::new(xof_xor_fn as *mut ()),
            universal_hash_ptr: AtomicPtr::new(universal_hash_fn as *mut ()),
        }
    }

    pub fn portable() -> Self {
        Self::new(
            || portable::DEGREE,
            portable::compress,
            portable::hash_chunks,
            portable::hash_parents,
            portable::xof,
            portable::xof_xor,
            portable::universal_hash,
        )
    }

    #[inline]
    fn degree_fn(&self) -> DegreeFn {
        unsafe { mem::transmute(self.degree_ptr.load(Relaxed)) }
    }

    #[inline]
    pub fn degree(&self) -> usize {
        let degree = self.degree_fn()();
        debug_assert!(degree >= 2);
        degree
    }

    #[inline]
    pub fn split_transposed_vectors<'v>(
        &self,
        vectors: &'v mut TransposedVectors,
    ) -> (TransposedSplit<'v>, TransposedSplit<'v>) {
        vectors.split(self.degree())
    }

    #[inline]
    fn compress_fn(&self) -> CompressFn {
        unsafe { mem::transmute(self.compress_ptr.load(Relaxed)) }
    }

    #[inline]
    pub fn compress(
        &self,
        block: &BlockBytes,
        block_len: u32,
        cv: &CVBytes,
        counter: u64,
        flags: u32,
    ) -> CVBytes {
        let mut out = [0u8; 32];
        unsafe {
            self.compress_fn()(block, block_len, cv, counter, flags, &mut out);
        }
        out
    }

    #[inline]
    fn hash_chunks_fn(&self) -> HashChunksFn {
        unsafe { mem::transmute(self.hash_chunks_ptr.load(Relaxed)) }
    }

    #[inline]
    pub fn hash_chunks(
        &self,
        input: &[u8],
        key: &CVBytes,
        counter: u64,
        flags: u32,
        transposed_output: TransposedSplit,
    ) -> usize {
        debug_assert!(input.len() > 0);
        debug_assert!(input.len() <= self.degree() * CHUNK_LEN);
        // SAFETY: If the caller passes in more than MAX_SIMD_DEGREE * CHUNK_LEN bytes, silently
        // ignore the remainder. This makes it impossible to write out of bounds in a properly
        // constructed TransposedSplit.
        let len = cmp::min(input.len(), MAX_SIMD_DEGREE * CHUNK_LEN);
        unsafe {
            self.hash_chunks_fn()(
                input.as_ptr(),
                len,
                key,
                counter,
                flags,
                transposed_output.ptr,
            );
        }
        if input.len() % CHUNK_LEN == 0 {
            input.len() / CHUNK_LEN
        } else {
            (input.len() / CHUNK_LEN) + 1
        }
    }

    #[inline]
    fn hash_parents_fn(&self) -> HashParentsFn {
        unsafe { mem::transmute(self.hash_parents_ptr.load(Relaxed)) }
    }

    #[inline]
    pub fn hash_parents(
        &self,
        transposed_input: &TransposedVectors,
        num_cvs: usize,
        key: &CVBytes,
        flags: u32,
        transposed_output: TransposedSplit,
    ) -> usize {
        let num_parents = num_cvs / 2;
        unsafe {
            self.hash_parents_fn()(
                transposed_input.as_ptr(),
                num_parents,
                key,
                flags,
                transposed_output.ptr,
            );
        }
        if num_cvs % 2 == 1 {
            unsafe {
                copy_one_transposed_cv(
                    transposed_input.as_ptr().add(num_cvs - 1),
                    transposed_output.ptr.add(num_parents),
                );
            }
            num_parents + 1
        } else {
            num_parents
        }
    }

    #[inline]
    pub fn reduce_parents(
        &self,
        transposed_in_out: &mut TransposedVectors,
        num_cvs: usize,
        key: &CVBytes,
        flags: u32,
    ) -> usize {
        let num_parents = num_cvs / 2;
        let in_out_ptr = transposed_in_out.as_mut_ptr();
        unsafe {
            self.hash_parents_fn()(in_out_ptr, num_parents, key, flags, in_out_ptr);
        }
        if num_cvs % 2 == 1 {
            unsafe {
                copy_one_transposed_cv(in_out_ptr.add(num_cvs - 1), in_out_ptr.add(num_parents));
            }
            num_parents + 1
        } else {
            num_parents
        }
    }

    #[inline]
    fn xof_fn(&self) -> XofFn {
        unsafe { mem::transmute(self.xof_ptr.load(Relaxed)) }
    }

    #[inline]
    pub fn xof(
        &self,
        block: &BlockBytes,
        block_len: u32,
        cv: &CVBytes,
        counter: u64,
        flags: u32,
        out: &mut [u8],
    ) {
        unsafe {
            self.xof_fn()(
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

    #[inline]
    fn xof_xor_fn(&self) -> XofFn {
        unsafe { mem::transmute(self.xof_xor_ptr.load(Relaxed)) }
    }

    #[inline]
    pub fn xof_xor(
        &self,
        block: &BlockBytes,
        block_len: u32,
        cv: &CVBytes,
        counter: u64,
        flags: u32,
        out: &mut [u8],
    ) {
        unsafe {
            self.xof_xor_fn()(
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

    #[inline]
    fn universal_hash_fn(&self) -> UniversalHashFn {
        unsafe { mem::transmute(self.universal_hash_ptr.load(Relaxed)) }
    }

    #[inline]
    pub fn universal_hash(&self, input: &[u8], key: &CVBytes, counter: u64) -> [u8; 16] {
        let mut out = [0u8; 16];
        unsafe {
            self.universal_hash_fn()(input.as_ptr(), input.len(), key, counter, &mut out);
        }
        out
    }
}

impl Clone for Implementation {
    fn clone(&self) -> Self {
        Self {
            degree_ptr: AtomicPtr::new(self.degree_ptr.load(Relaxed)),
            compress_ptr: AtomicPtr::new(self.compress_ptr.load(Relaxed)),
            hash_chunks_ptr: AtomicPtr::new(self.hash_chunks_ptr.load(Relaxed)),
            hash_parents_ptr: AtomicPtr::new(self.hash_parents_ptr.load(Relaxed)),
            xof_ptr: AtomicPtr::new(self.xof_ptr.load(Relaxed)),
            xof_xor_ptr: AtomicPtr::new(self.xof_xor_ptr.load(Relaxed)),
            universal_hash_ptr: AtomicPtr::new(self.universal_hash_ptr.load(Relaxed)),
        }
    }
}

// never less than 2
type DegreeFn = fn() -> usize;

fn degree_init() -> usize {
    init_detected_impl();
    DETECTED_IMPL.degree_fn()()
}

type CompressFn = unsafe extern "C" fn(
    block: *const BlockBytes, // zero padded to 64 bytes
    block_len: u32,
    cv: *const CVBytes,
    counter: u64,
    flags: u32,
    out: *mut CVBytes, // may overlap the input
);

unsafe extern "C" fn compress_init(
    block: *const BlockBytes,
    block_len: u32,
    cv: *const CVBytes,
    counter: u64,
    flags: u32,
    out: *mut CVBytes,
) {
    init_detected_impl();
    DETECTED_IMPL.compress_fn()(block, block_len, cv, counter, flags, out);
}

type CompressXofFn = unsafe extern "C" fn(
    block: *const BlockBytes, // zero padded to 64 bytes
    block_len: u32,
    cv: *const CVBytes,
    counter: u64,
    flags: u32,
    out: *mut BlockBytes, // may overlap the input
);

type HashChunksFn = unsafe extern "C" fn(
    input: *const u8,
    input_len: usize,
    key: *const CVBytes,
    counter: u64,
    flags: u32,
    transposed_output: *mut u32,
);

unsafe extern "C" fn hash_chunks_init(
    input: *const u8,
    input_len: usize,
    key: *const CVBytes,
    counter: u64,
    flags: u32,
    transposed_output: *mut u32,
) {
    init_detected_impl();
    DETECTED_IMPL.hash_chunks_fn()(input, input_len, key, counter, flags, transposed_output);
}

type HashParentsFn = unsafe extern "C" fn(
    transposed_input: *const u32,
    num_parents: usize,
    key: *const CVBytes,
    flags: u32,
    transposed_output: *mut u32, // may overlap the input
);

unsafe extern "C" fn hash_parents_init(
    transposed_input: *const u32,
    num_parents: usize,
    key: *const CVBytes,
    flags: u32,
    transposed_output: *mut u32,
) {
    init_detected_impl();
    DETECTED_IMPL.hash_parents_fn()(transposed_input, num_parents, key, flags, transposed_output);
}

// This signature covers both xof() and xof_xor().
type XofFn = unsafe extern "C" fn(
    block: *const BlockBytes, // zero padded to 64 bytes
    block_len: u32,
    cv: *const CVBytes,
    counter: u64,
    flags: u32,
    out: *mut u8,
    out_len: usize,
);

unsafe extern "C" fn xof_init(
    block: *const BlockBytes,
    block_len: u32,
    cv: *const CVBytes,
    counter: u64,
    flags: u32,
    out: *mut u8,
    out_len: usize,
) {
    init_detected_impl();
    DETECTED_IMPL.xof_fn()(block, block_len, cv, counter, flags, out, out_len);
}

unsafe extern "C" fn xof_xor_init(
    block: *const BlockBytes,
    block_len: u32,
    cv: *const CVBytes,
    counter: u64,
    flags: u32,
    out: *mut u8,
    out_len: usize,
) {
    init_detected_impl();
    DETECTED_IMPL.xof_xor_fn()(block, block_len, cv, counter, flags, out, out_len);
}

type UniversalHashFn = unsafe extern "C" fn(
    input: *const u8,
    input_len: usize,
    key: *const CVBytes,
    counter: u64,
    out: *mut [u8; 16],
);

unsafe extern "C" fn universal_hash_init(
    input: *const u8,
    input_len: usize,
    key: *const CVBytes,
    counter: u64,
    out: *mut [u8; 16],
) {
    init_detected_impl();
    DETECTED_IMPL.universal_hash_fn()(input, input_len, key, counter, out);
}

// The implicit degree of this implementation is MAX_SIMD_DEGREE.
#[inline(always)]
unsafe fn hash_chunks_using_compress(
    compress: CompressFn,
    mut input: *const u8,
    mut input_len: usize,
    key: *const CVBytes,
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
        let mut cv = *key;
        let cv_ptr: *mut CVBytes = &mut cv;
        let mut chunk_flags = flags | CHUNK_START;
        while chunk_len > BLOCK_LEN {
            compress(
                input as *const BlockBytes,
                BLOCK_LEN as u32,
                cv_ptr,
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
            cv_ptr,
            counter,
            chunk_flags | CHUNK_END,
            cv_ptr,
        );
        let cv_words = words_from_le_bytes_32(&cv);
        for word_index in 0..8 {
            transposed_output
                .add(word_index * TRANSPOSED_STRIDE)
                .write(cv_words[word_index]);
        }
        transposed_output = transposed_output.add(1);
        counter += 1;
    }
}

// The implicit degree of this implementation is MAX_SIMD_DEGREE.
#[inline(always)]
unsafe fn hash_parents_using_compress(
    compress: CompressFn,
    mut transposed_input: *const u32,
    mut num_parents: usize,
    key: *const CVBytes,
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
        let mut cv = [0u8; 32];
        compress(&block_bytes, BLOCK_LEN as u32, key, 0, flags, &mut cv);
        let cv_words = words_from_le_bytes_32(&cv);
        for word_index in 0..8 {
            transposed_output
                .add(word_index * TRANSPOSED_STRIDE)
                .write(cv_words[word_index]);
        }
        transposed_input = transposed_input.add(2);
        transposed_output = transposed_output.add(1);
        num_parents -= 1;
    }
}

#[inline(always)]
unsafe fn xof_using_compress_xof(
    compress_xof: CompressXofFn,
    block: *const BlockBytes,
    block_len: u32,
    cv: *const CVBytes,
    mut counter: u64,
    flags: u32,
    mut out: *mut u8,
    mut out_len: usize,
) {
    while out_len > 0 {
        let mut block_output = [0u8; 64];
        compress_xof(block, block_len, cv, counter, flags, &mut block_output);
        let take = cmp::min(out_len, BLOCK_LEN);
        ptr::copy_nonoverlapping(block_output.as_ptr(), out, take);
        out = out.add(take);
        out_len -= take;
        counter += 1;
    }
}

#[inline(always)]
unsafe fn xof_xor_using_compress_xof(
    compress_xof: CompressXofFn,
    block: *const BlockBytes,
    block_len: u32,
    cv: *const CVBytes,
    mut counter: u64,
    flags: u32,
    mut out: *mut u8,
    mut out_len: usize,
) {
    while out_len > 0 {
        let mut block_output = [0u8; 64];
        compress_xof(block, block_len, cv, counter, flags, &mut block_output);
        let take = cmp::min(out_len, BLOCK_LEN);
        for i in 0..take {
            *out.add(i) ^= block_output[i];
        }
        out = out.add(take);
        out_len -= take;
        counter += 1;
    }
}

#[inline(always)]
unsafe fn universal_hash_using_compress(
    compress: CompressFn,
    mut input: *const u8,
    mut input_len: usize,
    key: *const CVBytes,
    mut counter: u64,
    out: *mut [u8; 16],
) {
    let flags = KEYED_HASH | CHUNK_START | CHUNK_END | ROOT;
    let mut result = [0u8; 16];
    while input_len > 0 {
        let block_len = cmp::min(input_len, BLOCK_LEN);
        let mut block = [0u8; BLOCK_LEN];
        ptr::copy_nonoverlapping(input, block.as_mut_ptr(), block_len);
        let mut block_output = [0u8; 32];
        compress(
            &block,
            BLOCK_LEN as u32,
            key,
            counter,
            flags,
            &mut block_output,
        );
        for i in 0..16 {
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

// this is in units of *words*, for pointer operations on *const/*mut u32
const TRANSPOSED_STRIDE: usize = 2 * MAX_SIMD_DEGREE;

#[cfg_attr(any(target_arch = "x86", target_arch = "x86_64"), repr(C, align(64)))]
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct TransposedVectors([[u32; 2 * MAX_SIMD_DEGREE]; 8]);

impl TransposedVectors {
    pub fn parent_node(&self, parent_index: usize) -> BlockBytes {
        let mut bytes = [0u8; 64];
        for word_index in 0..8 {
            bytes[word_index * WORD_LEN..][..WORD_LEN]
                .copy_from_slice(&self.0[word_index][2 * parent_index].to_le_bytes());
            bytes[(word_index + 8) * WORD_LEN..][..WORD_LEN]
                .copy_from_slice(&self.0[word_index][2 * parent_index + 1].to_le_bytes());
        }
        bytes
    }

    fn as_ptr(&self) -> *const u32 {
        self.0[0].as_ptr()
    }

    fn as_mut_ptr(&mut self) -> *mut u32 {
        self.0[0].as_mut_ptr()
    }

    fn split(&mut self, degree: usize) -> (TransposedSplit, TransposedSplit) {
        debug_assert!(degree > 0);
        debug_assert!(degree <= MAX_SIMD_DEGREE);
        debug_assert_eq!(degree.count_ones(), 1, "power of 2");
        let ptr = self.as_mut_ptr();
        let left = TransposedSplit {
            ptr,
            phantom_data: PhantomData,
        };
        let right = TransposedSplit {
            ptr: ptr.wrapping_add(degree),
            phantom_data: PhantomData,
        };
        (left, right)
    }
}

pub struct TransposedSplit<'vectors> {
    ptr: *mut u32,
    phantom_data: PhantomData<&'vectors mut u32>,
}

unsafe impl<'vectors> Send for TransposedSplit<'vectors> {}
unsafe impl<'vectors> Sync for TransposedSplit<'vectors> {}

unsafe fn copy_one_transposed_cv(transposed_src: *const u32, transposed_dest: *mut u32) {
    for word_index in 0..8 {
        let offset_words = word_index * TRANSPOSED_STRIDE;
        let word = transposed_src.add(offset_words).read();
        transposed_dest.add(offset_words).write(word);
    }
}

#[inline(always)]
pub const fn le_bytes_from_words_32(words: &CVWords) -> CVBytes {
    let mut bytes = [0u8; 32];
    // This loop is super verbose because currently that's what it takes to be const.
    let mut word_index = 0;
    while word_index < bytes.len() / WORD_LEN {
        let word_bytes = words[word_index].to_le_bytes();
        let mut byte_index = 0;
        while byte_index < WORD_LEN {
            bytes[word_index * WORD_LEN + byte_index] = word_bytes[byte_index];
            byte_index += 1;
        }
        word_index += 1;
    }
    bytes
}

#[inline(always)]
pub const fn le_bytes_from_words_64(words: &BlockWords) -> BlockBytes {
    let mut bytes = [0u8; 64];
    // This loop is super verbose because currently that's what it takes to be const.
    let mut word_index = 0;
    while word_index < bytes.len() / WORD_LEN {
        let word_bytes = words[word_index].to_le_bytes();
        let mut byte_index = 0;
        while byte_index < WORD_LEN {
            bytes[word_index * WORD_LEN + byte_index] = word_bytes[byte_index];
            byte_index += 1;
        }
        word_index += 1;
    }
    bytes
}

#[inline(always)]
pub const fn words_from_le_bytes_32(bytes: &CVBytes) -> CVWords {
    let mut words = [0u32; 8];
    // This loop is super verbose because currently that's what it takes to be const.
    let mut word_index = 0;
    while word_index < words.len() {
        let mut word_bytes = [0u8; WORD_LEN];
        let mut byte_index = 0;
        while byte_index < WORD_LEN {
            word_bytes[byte_index] = bytes[word_index * WORD_LEN + byte_index];
            byte_index += 1;
        }
        words[word_index] = u32::from_le_bytes(word_bytes);
        word_index += 1;
    }
    words
}

#[inline(always)]
pub const fn words_from_le_bytes_64(bytes: &BlockBytes) -> BlockWords {
    let mut words = [0u32; 16];
    // This loop is super verbose because currently that's what it takes to be const.
    let mut word_index = 0;
    while word_index < words.len() {
        let mut word_bytes = [0u8; WORD_LEN];
        let mut byte_index = 0;
        while byte_index < WORD_LEN {
            word_bytes[byte_index] = bytes[word_index * WORD_LEN + byte_index];
            byte_index += 1;
        }
        words[word_index] = u32::from_le_bytes(word_bytes);
        word_index += 1;
    }
    words
}

#[test]
fn test_byte_word_round_trips() {
    let cv = *b"This is 32 LE bytes/eight words.";
    assert_eq!(cv, le_bytes_from_words_32(&words_from_le_bytes_32(&cv)));
    let block = *b"This is sixty-four little-endian bytes, or sixteen 32-bit words.";
    assert_eq!(
        block,
        le_bytes_from_words_64(&words_from_le_bytes_64(&block)),
    );
}
