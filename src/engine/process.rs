use anyhow::Result;

use super::memory::{MemoryRead, MemoryWrite};

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::ptr::null_mut;

    use anyhow::{anyhow, bail, Context, Result};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
        MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ,
        PROCESS_VM_WRITE,
    };

    use crate::engine::addresses::MODULE_NAME;
    use crate::engine::memory::ensure_exact_read;

    pub struct GameProcess {
        handle: HANDLE,
        process_id: u32,
        crusader_base: usize,
    }

    impl GameProcess {
        pub fn attach(process_name: &str) -> Result<Self> {
            let process_id = find_process_id(process_name)
                .with_context(|| format!("process not found: {process_name}"))?;

            let handle = unsafe {
                OpenProcess(
                    PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION,
                    0,
                    process_id,
                )
            };

            if handle.is_null() {
                return Err(last_os_error("OpenProcess"));
            }

            let crusader_base = match find_module_base(process_id, MODULE_NAME) {
                Ok(base) => base,
                Err(err) => {
                    unsafe {
                        CloseHandle(handle);
                    }
                    return Err(err);
                }
            };

            Ok(Self {
                handle,
                process_id,
                crusader_base,
            })
        }

        pub fn process_id(&self) -> u32 {
            self.process_id
        }

        pub fn crusader_base(&self) -> usize {
            self.crusader_base
        }

        pub fn module_addr(&self, offset: usize) -> usize {
            self.crusader_base + offset
        }

        pub fn read_exact(&self, address: usize, buffer: &mut [u8]) -> Result<()> {
            let mut bytes_read = 0usize;
            let ok = unsafe {
                ReadProcessMemory(
                    self.handle,
                    address as *const c_void,
                    buffer.as_mut_ptr() as *mut c_void,
                    buffer.len(),
                    &mut bytes_read,
                )
            };

            if ok == 0 {
                return Err(last_os_error(&format!("ReadProcessMemory({address:#x})")));
            }

            ensure_exact_read(buffer.len(), bytes_read, address)
        }

        pub fn write_exact(&self, address: usize, buffer: &[u8]) -> Result<()> {
            let write_handle = unsafe {
                OpenProcess(
                    PROCESS_VM_WRITE | PROCESS_VM_OPERATION | PROCESS_QUERY_LIMITED_INFORMATION,
                    0,
                    self.process_id,
                )
            };

            if write_handle.is_null() {
                return Err(last_os_error("OpenProcess(write)"));
            }

            let mut bytes_written = 0usize;
            let ok = unsafe {
                WriteProcessMemory(
                    write_handle,
                    address as *mut c_void,
                    buffer.as_ptr() as *const c_void,
                    buffer.len(),
                    &mut bytes_written,
                )
            };

            unsafe {
                CloseHandle(write_handle);
            }

            if ok == 0 {
                return Err(last_os_error(&format!("WriteProcessMemory({address:#x})")));
            }

            ensure_exact_read(buffer.len(), bytes_written, address)
        }
    }

    impl Drop for GameProcess {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    CloseHandle(self.handle);
                }
                self.handle = null_mut();
            }
        }
    }

    fn find_process_id(process_name: &str) -> Result<u32> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(last_os_error("CreateToolhelp32Snapshot(processes)"));
        }

        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..PROCESSENTRY32W::default()
        };
        let target = process_name.to_ascii_lowercase();

        if unsafe { Process32FirstW(snapshot, &mut entry) } != 0 {
            loop {
                let current = wide_array_to_string(&entry.szExeFile).to_ascii_lowercase();
                if current == target {
                    unsafe {
                        CloseHandle(snapshot);
                    }
                    return Ok(entry.th32ProcessID);
                }

                let next_ok = unsafe { Process32NextW(snapshot, &mut entry) };
                if next_ok == 0 {
                    break;
                }
            }
        }

        unsafe {
            CloseHandle(snapshot);
        }
        bail!("process not found: {process_name}")
    }

    fn find_module_base(process_id: u32, module_name: &str) -> Result<usize> {
        let snapshot = unsafe {
            CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, process_id)
        };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(last_os_error("CreateToolhelp32Snapshot(modules)"));
        }

        let mut entry = MODULEENTRY32W {
            dwSize: size_of::<MODULEENTRY32W>() as u32,
            ..MODULEENTRY32W::default()
        };
        let target = module_name.to_ascii_lowercase();

        if unsafe { Module32FirstW(snapshot, &mut entry) } != 0 {
            loop {
                let current = wide_array_to_string(&entry.szModule).to_ascii_lowercase();
                if current == target {
                    let base = entry.modBaseAddr as usize;
                    unsafe {
                        CloseHandle(snapshot);
                    }
                    return Ok(base);
                }

                let next_ok = unsafe { Module32NextW(snapshot, &mut entry) };
                if next_ok == 0 {
                    break;
                }
            }
        }

        unsafe {
            CloseHandle(snapshot);
        }
        bail!("module not found in process {process_id}: {module_name}")
    }

    fn wide_array_to_string(value: &[u16]) -> String {
        let len = value.iter().position(|ch| *ch == 0).unwrap_or(value.len());
        String::from_utf16_lossy(&value[..len])
    }

    fn last_os_error(context: &str) -> anyhow::Error {
        anyhow!("{context} failed: {}", std::io::Error::last_os_error())
    }
}

#[cfg(windows)]
pub use win::GameProcess;

#[cfg(not(windows))]
use super::addresses::MODULE_NAME;

#[cfg(not(windows))]
pub struct GameProcess {
    crusader_base: usize,
}

#[cfg(not(windows))]
impl GameProcess {
    pub fn crusader_base(&self) -> usize {
        self.crusader_base
    }

    pub fn module_addr(&self, offset: usize) -> usize {
        self.crusader_base + offset
    }
}

impl MemoryRead for GameProcess {
    fn read_u8(&self, address: usize) -> Result<u8> {
        let mut buffer = [0u8; 1];
        self.read_exact(address, &mut buffer)?;
        Ok(buffer[0])
    }

    fn read_u16(&self, address: usize) -> Result<u16> {
        let mut buffer = [0u8; 2];
        self.read_exact(address, &mut buffer)?;
        Ok(u16::from_le_bytes(buffer))
    }

    fn read_i16(&self, address: usize) -> Result<i16> {
        let mut buffer = [0u8; 2];
        self.read_exact(address, &mut buffer)?;
        Ok(i16::from_le_bytes(buffer))
    }

    fn read_u32(&self, address: usize) -> Result<u32> {
        let mut buffer = [0u8; 4];
        self.read_exact(address, &mut buffer)?;
        Ok(u32::from_le_bytes(buffer))
    }

    fn read_i32(&self, address: usize) -> Result<i32> {
        let mut buffer = [0u8; 4];
        self.read_exact(address, &mut buffer)?;
        Ok(i32::from_le_bytes(buffer))
    }
}

impl MemoryWrite for GameProcess {
    fn write_u32(&self, address: usize, value: u32) -> Result<()> {
        self.write_exact(address, &value.to_le_bytes())
    }
}

#[cfg(not(windows))]
impl GameProcess {
    pub fn read_exact(&self, _address: usize, _buffer: &mut [u8]) -> Result<()> {
        anyhow::bail!("process memory reading is only supported on Windows")
    }

    pub fn write_exact(&self, _address: usize, _buffer: &[u8]) -> Result<()> {
        anyhow::bail!("process memory writing is only supported on Windows")
    }
}

pub fn attach_to_game_process(process_name: &str) -> Result<GameProcess> {
    #[cfg(windows)]
    {
        GameProcess::attach(process_name)
    }

    #[cfg(not(windows))]
    {
        let _ = process_name;
        anyhow::bail!("cannot attach to {MODULE_NAME}: this platform is not Windows")
    }
}

pub fn attach_to_known_game_process() -> Result<GameProcess> {
    const PROCESS_NAMES: [&str; 3] = [
        "CrusaderDE.exe",
        "Stronghold Crusader Definitive Edition.exe",
        "StrongholdCrusaderDE.exe",
    ];

    let mut errors = Vec::new();
    for process_name in PROCESS_NAMES {
        match attach_to_game_process(process_name) {
            Ok(process) => return Ok(process),
            Err(err) => errors.push(format!("{process_name}: {err:#}")),
        }
    }

    anyhow::bail!(
        "could not attach to known SHC:DE process names: {}",
        errors.join("; ")
    )
}
