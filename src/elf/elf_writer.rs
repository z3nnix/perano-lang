use crate::pe::MachineCode;
use std::fs::File;
use std::io::{self, Write};

const ELF_MAGIC: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46];
const ELF_CLASS_64: u8 = 2;
const ELF_DATA_LSB: u8 = 1;
const ELF_VERSION: u8 = 1;
const ELF_OSABI_SYSV: u8 = 0;

const ET_EXEC: u16 = 2;
const EM_X86_64: u16 = 0x3E;

const PT_LOAD: u32 = 1;

pub struct ELFWriter {
    entry_point: u64,
    load_address: u64,
}

impl ELFWriter {
    pub fn new() -> Self {
        ELFWriter {
            entry_point: 0x401000,
            load_address: 0x400000,
        }
    }

    pub fn write(&mut self, filename: &str, machine_code: &MachineCode) -> io::Result<()> {
        let mut buffer = Vec::new();

        self.write_elf_header(&mut buffer);

        let code_size = machine_code.code.len() as u64;
        let file_size = 0x1000 + code_size;
        self.write_program_header(&mut buffer, file_size, code_size);

        while buffer.len() < 0x1000 {
            buffer.push(0);
        }

        buffer.extend_from_slice(&machine_code.code);

        let mut file = File::create(filename)?;
        file.write_all(&buffer)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = file.metadata()?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(filename, perms)?;
        }

        Ok(())
    }

    fn write_elf_header(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&ELF_MAGIC);
        buffer.push(ELF_CLASS_64);
        buffer.push(ELF_DATA_LSB);
        buffer.push(ELF_VERSION);
        buffer.push(ELF_OSABI_SYSV);
        buffer.extend_from_slice(&[0; 8]);

        buffer.extend_from_slice(&ET_EXEC.to_le_bytes());

        buffer.extend_from_slice(&EM_X86_64.to_le_bytes());

        buffer.extend_from_slice(&1u32.to_le_bytes());

        buffer.extend_from_slice(&self.entry_point.to_le_bytes());

        buffer.extend_from_slice(&64u64.to_le_bytes());

        buffer.extend_from_slice(&0u64.to_le_bytes());

        buffer.extend_from_slice(&0u32.to_le_bytes());

        buffer.extend_from_slice(&64u16.to_le_bytes());

        buffer.extend_from_slice(&56u16.to_le_bytes());

        buffer.extend_from_slice(&1u16.to_le_bytes());

        buffer.extend_from_slice(&0u16.to_le_bytes());

        buffer.extend_from_slice(&0u16.to_le_bytes());

        buffer.extend_from_slice(&0u16.to_le_bytes());
    }

    fn write_program_header(&self, buffer: &mut Vec<u8>, file_size: u64, _mem_size: u64) {
        buffer.extend_from_slice(&PT_LOAD.to_le_bytes());

        buffer.extend_from_slice(&5u32.to_le_bytes());

        buffer.extend_from_slice(&0u64.to_le_bytes());

        buffer.extend_from_slice(&self.load_address.to_le_bytes());

        buffer.extend_from_slice(&self.load_address.to_le_bytes());

        buffer.extend_from_slice(&file_size.to_le_bytes());

        buffer.extend_from_slice(&file_size.to_le_bytes());

        buffer.extend_from_slice(&0x1000u64.to_le_bytes());
    }
}