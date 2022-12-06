use std::{ffi::CString, fs, ptr::null};

use bindings::emp_rot;
extern crate tokio;

mod bindings;

/// Get bit ROTs from u8 ROTs returned by EMP
pub fn get_bit_rot(m0: &[u8], m1: &[u8]) -> (Vec<bool>, Vec<bool>) {
    let b0 = m0.iter().map(|x| (*x & 1u8) > 0).collect();
    let b1 = m1.iter().map(|x| (*x & 1u8) > 0).collect();
    (b0, b1)
}

pub enum RotConfig {
    Alice(i32),        // If I'm alice, I listen to this port.
    Bob(CString, i32), // If I'm bob, I connect to this host and this port.
}

#[derive(Debug, Clone, Copy)]
pub enum ROTMode {
    IKNP,
    FERRET,
}

impl ROTMode {
    pub fn emp_mode_num(&self) -> i32 {
        match self {
            ROTMode::IKNP => 0,
            ROTMode::FERRET => 1,
        }
    }
}

/// party is false for Alice and true for Bob. Returns count number of bit ROTs
/// from EMP. Set mode = 1 for Ferret and mode = 0 for IKNP
/// Return number of bytes sent.
pub fn get_rot_emp(
    count: i64,
    rot_config: &RotConfig,
    mode: ROTMode,
) -> (Vec<bool>, Vec<bool>, u64) {
    let mode = mode.emp_mode_num();
    let mut m0: Vec<u8> = vec![0; count as usize];
    let mut m1: Vec<u8> = vec![0; count as usize];

    let num_bytes_transfered = match rot_config {
        RotConfig::Alice(port) => unsafe {
            // delete previous data
            // intentionally ignore the warning
            let _ = fs::remove_file("data/".to_string() + &port.to_string());
            let _ = fs::create_dir("data");
            emp_rot(
                1,
                null(),
                *port,
                count,
                mode,
                m0.as_mut_ptr(),
                m1.as_mut_ptr(),
            )
        },
        RotConfig::Bob(addr, port) => unsafe {
            // delete previous data
            // intentionally ignore the warning
            let _ = fs::remove_file("data/".to_string() + &port.to_string());
            let _ = fs::create_dir("data");
            emp_rot(
                2,
                addr.as_ptr(),
                *port,
                count,
                mode,
                m0.as_mut_ptr(),
                m1.as_mut_ptr(),
            )
        },
    };

    let (m0, m1) = get_bit_rot(&m0, &m1);
    (m0, m1, num_bytes_transfered)
}

/// party is false for Alice and true for Bob. Returns count number of bit ROTs
/// from EMP. Set mode = 1 for Ferret and mode = 0 for IKNP
/// Return number of bytes sent.
/// This function is without `get_bit_rot`.
pub fn get_rot_emp_dummy(count: i64, rot_config: &RotConfig, mode: ROTMode) -> u64 {
    let mode = mode.emp_mode_num();
    let mut m0: Vec<u8> = vec![0; count as usize];
    let mut m1: Vec<u8> = vec![0; count as usize];

    let num_bytes_transfered = match rot_config {
        RotConfig::Alice(port) => unsafe {
            // delete previous data
            // intentionally ignore the warning
            let _ = fs::remove_file("data/".to_string() + &port.to_string());
            let _ = fs::create_dir("data");
            emp_rot(
                1,
                null(),
                *port,
                count,
                mode,
                m0.as_mut_ptr(),
                m1.as_mut_ptr(),
            )
        },
        RotConfig::Bob(addr, port) => unsafe {
            // delete previous data
            // intentionally ignore the warning
            let _ = fs::remove_file("data/".to_string() + &port.to_string());
            let _ = fs::create_dir("data");
            emp_rot(
                2,
                addr.as_ptr(),
                *port,
                count,
                mode,
                m0.as_mut_ptr(),
                m1.as_mut_ptr(),
            )
        },
    };

    // generate random data of 2bytes

    num_bytes_transfered
}

#[cfg(test)]
mod tests {
    use std::{ffi::CString, ptr::null};

    use crate::emp_rot;
    extern crate tokio;
    use super::get_bit_rot;

    #[tokio::test]
    #[ignore]
    async fn test_emp_rot() {
        // EMP ROT

        let port = 32000;
        let count = 32 * 100000;

        let mut m0: Vec<u8> = vec![0; count as usize];
        let mut m1: Vec<u8> = vec![0; count as usize];
        let mut m: Vec<u8> = vec![0; count as usize];
        let mut choice: Vec<u8> = vec![0; count as usize];

        let s0_handle = tokio::task::spawn_blocking(move || {
            let num_bytes_sent =
                unsafe { emp_rot(1, null(), port, count, 0, m0.as_mut_ptr(), m1.as_mut_ptr()) };
            (m0, m1, num_bytes_sent)
        });
        let s1_handle = tokio::task::spawn_blocking(move || {
            let c_addr = CString::new("127.0.0.1").unwrap();
            let num_bytes_sent = unsafe {
                emp_rot(
                    2,
                    c_addr.as_ptr(),
                    port,
                    count,
                    0,
                    m.as_mut_ptr(),
                    choice.as_mut_ptr(),
                )
            };
            (m, choice, num_bytes_sent)
        });

        let ((m0, m1, nsent_s0), (m, choice, nsent_s1)) =
            (s0_handle.await.unwrap(), s1_handle.await.unwrap());

        let (m0, m1) = get_bit_rot(&m0, &m1);
        let (m, choice) = get_bit_rot(&m, &choice);

        // verifying if OTs are correct
        (0..count as usize).for_each(|i| {
            if choice[i] == false {
                assert_eq!(m[i], m0[i]);
            } else {
                assert_eq!(m[i], m1[i]);
            }
        });

        println!("Random OTs are correct!");
        println!("Sent {} bytes by s0, {} bytes by s1", nsent_s0, nsent_s1);
    }
}
