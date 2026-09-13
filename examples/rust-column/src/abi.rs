//! Buffer ABI 1 adapter, wasm32 only. No raw-pointer dereference or unsafe block.
//! The host writes only into the allocation returned by os_alloc, while no guest
//! function is running; each invocation uses a fresh instance and one caller.
use os_plugin_api::generic::MAX_BYTES;
use std::sync::Mutex;

static BUFFERS: Mutex<Buffers> = Mutex::new(Buffers {
    input: Vec::new(),
    output: Vec::new(),
    ready: false,
});
struct Buffers {
    input: Vec<u8>,
    output: Vec<u8>,
    ready: bool,
}

// SAFETY: These unique ABI names/signatures are exported only in the standalone
// wasm module, not linked into a native process or the host application.
#[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
pub extern "C" fn os_abi_version() -> i32 {
    1
}

// SAFETY: See module contract. Allocation remains owned and alive in BUFFERS.
#[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
pub extern "C" fn os_alloc(len: u32) -> u32 {
    let mut buffers = BUFFERS.lock().unwrap();
    buffers.ready = false;
    if len == 0 || len as usize > MAX_BYTES {
        return 0;
    }
    buffers.input = vec![0; len as usize];
    buffers.output.clear();
    buffers.ready = true;
    buffers.input.as_ptr() as u32
}

// SAFETY: Unique Wasm export. Never construct a Rust reference from the supplied
// address: compare it with the owned allocation, then use the owned slice.
#[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
pub extern "C" fn os_invoke(ptr: u32, len: u32) -> u64 {
    let mut buffers = BUFFERS.lock().unwrap();
    if !buffers.ready || ptr != buffers.input.as_ptr() as u32 || len as usize != buffers.input.len()
    {
        return 0;
    }
    buffers.ready = false;
    let Some(output) = crate::respond(&buffers.input) else {
        return 0;
    };
    buffers.output = output;
    ((buffers.output.len() as u64) << 32) | buffers.output.as_ptr() as u32 as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buffer_ownership_bounds_and_single_invocation() {
        assert_eq!(os_abi_version(), 1);
        assert_eq!(os_alloc(0), 0);
        assert_eq!(os_alloc(MAX_BYTES as u32 + 1), 0);
        assert_eq!(os_invoke(0, 0), 0);
        let input = br#"{"api_version":2,"request_id":"nonce","context":null,"operation":{"kind":"Describe"}}"#;
        let ptr = os_alloc(input.len() as u32);
        assert_eq!(os_invoke(ptr.wrapping_add(1), input.len() as u32), 0);
        assert_eq!(os_invoke(ptr, input.len() as u32 - 1), 0);
        BUFFERS.lock().unwrap().input.copy_from_slice(input);
        let output = os_invoke(ptr, input.len() as u32);
        assert!(output >> 32 > 0);
        assert_eq!(os_invoke(ptr, input.len() as u32), 0);
        let buffers = BUFFERS.lock().unwrap();
        let reply: os_plugin_api::generic::Response =
            serde_json::from_slice(&buffers.output).unwrap();
        assert_eq!(reply.request_id, "nonce");
    }
}
