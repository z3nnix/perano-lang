use crate::pe::codegen::MachineCode;
use std::fs::File;
use std::io::{self, Write};

const IMAGE_DOS_SIGNATURE: u16 = 0x5A4D;
const IMAGE_NT_SIGNATURE: u32 = 0x00004550;
const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;

pub struct PEWriter {
    image_base: u64,
    section_alignment: u32,
    file_alignment: u32,
}

impl PEWriter {
    pub fn new() -> Self {
        PEWriter {
            image_base: 0x140000000,
            section_alignment: 0x1000,
            file_alignment: 0x200,
        }
    }

    pub fn write(&mut self, filename: &str, machine_code: &MachineCode) -> io::Result<()> {
        let mut buffer = Vec::new();

        let has_imports = machine_code.code.windows(6).any(|w| {
            if w[0] == 0xFF && w[1] == 0x15 {
                let placeholder = i32::from_le_bytes([w[2], w[3], w[4], w[5]]);
                placeholder == 0x20000000u32 as i32 ||
                placeholder == 0x20080000u32 as i32 ||
                placeholder == 0x10000000u32 as i32
            } else {
                false
            }
        });

        let import_data = if has_imports { self.build_import_data() } else { Vec::new() };

        let import_size = if import_data.is_empty() {
            0
        } else {
            self.align(import_data.len() as u32, self.file_alignment)
        };

        let num_sections = if import_size > 0 { 2 } else { 1 };

        self.write_dos_header(&mut buffer);

        self.write_dos_stub(&mut buffer);

        while buffer.len() < 0x80 {
            buffer.push(0);
        }
        buffer.extend_from_slice(&IMAGE_NT_SIGNATURE.to_le_bytes());

        self.write_coff_header(&mut buffer, num_sections);

        let code_size = self.align(machine_code.code.len() as u32, self.file_alignment);
        self.write_optional_header(&mut buffer, code_size, import_size);

        self.write_section_headers(&mut buffer, code_size, import_size, num_sections);

        while buffer.len() % self.file_alignment as usize != 0 {
            buffer.push(0);
        }

        let mut patched_code = machine_code.code.clone();
        if import_size > 0 {
            self.patch_import_addresses(&mut patched_code, code_size);
        }

        buffer.extend_from_slice(&patched_code);
        while buffer.len() % self.file_alignment as usize != 0 {
            buffer.push(0);
        }

        if import_size > 0 {
            buffer.extend_from_slice(&import_data);
            while buffer.len() % self.file_alignment as usize != 0 {
                buffer.push(0);
            }
        }

        let mut file = File::create(filename)?;
        file.write_all(&buffer)?;

        Ok(())
    }

    fn write_dos_header(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&IMAGE_DOS_SIGNATURE.to_le_bytes());
        buffer.extend_from_slice(&[0x90, 0x00]);
        buffer.extend_from_slice(&[0x03, 0x00]);
        buffer.extend_from_slice(&[0x00, 0x00]);
        buffer.extend_from_slice(&[0x04, 0x00]);
        buffer.extend_from_slice(&[0x00, 0x00]);
        buffer.extend_from_slice(&[0xFF, 0xFF]);
        buffer.extend_from_slice(&[0x00, 0x00]);
        buffer.extend_from_slice(&[0xB8, 0x00]);
        buffer.extend_from_slice(&[0x00, 0x00]);
        buffer.extend_from_slice(&[0x00, 0x00]);
        buffer.extend_from_slice(&[0x00, 0x00]);
        buffer.extend_from_slice(&[0x40, 0x00]);
        buffer.extend_from_slice(&[0x00, 0x00]);
        buffer.extend_from_slice(&[0; 8]);
        buffer.extend_from_slice(&[0x00, 0x00]);
        buffer.extend_from_slice(&[0x00, 0x00]);
        buffer.extend_from_slice(&[0; 20]);
        buffer.extend_from_slice(&0x80u32.to_le_bytes());
    }

    fn write_dos_stub(&self, buffer: &mut Vec<u8>) {
        let stub = [
            0x0E, 0x1F, 0xBA, 0x0E, 0x00, 0xB4, 0x09, 0xCD,
            0x21, 0xB8, 0x01, 0x4C, 0xCD, 0x21, 0x54, 0x68,
            0x69, 0x73, 0x20, 0x70, 0x72, 0x6F, 0x67, 0x72,
            0x61, 0x6D, 0x20, 0x63, 0x61, 0x6E, 0x6E, 0x6F,
            0x74, 0x20, 0x62, 0x65, 0x20, 0x72, 0x75, 0x6E,
            0x20, 0x69, 0x6E, 0x20, 0x44, 0x4F, 0x53, 0x20,
            0x6D, 0x6F, 0x64, 0x65, 0x2E, 0x0D, 0x0D, 0x0A,
            0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        buffer.extend_from_slice(&stub);
    }

    fn write_coff_header(&self, buffer: &mut Vec<u8>, num_sections: u16) {
        buffer.extend_from_slice(&IMAGE_FILE_MACHINE_AMD64.to_le_bytes());
        buffer.extend_from_slice(&num_sections.to_le_bytes());
        buffer.extend_from_slice(&0u32.to_le_bytes());
        buffer.extend_from_slice(&0u32.to_le_bytes());
        buffer.extend_from_slice(&0u32.to_le_bytes());
        buffer.extend_from_slice(&0xF0u16.to_le_bytes());
        buffer.extend_from_slice(&0x0022u16.to_le_bytes());
    }

    fn write_optional_header(&self, buffer: &mut Vec<u8>, code_size: u32, import_size: u32) {
        buffer.extend_from_slice(&0x20Bu16.to_le_bytes());
        buffer.push(14);
        buffer.push(0);
        buffer.extend_from_slice(&code_size.to_le_bytes());
        buffer.extend_from_slice(&import_size.to_le_bytes());
        buffer.extend_from_slice(&0u32.to_le_bytes());
        buffer.extend_from_slice(&0x1000u32.to_le_bytes());
        buffer.extend_from_slice(&0x1000u32.to_le_bytes());

        buffer.extend_from_slice(&self.image_base.to_le_bytes());
        buffer.extend_from_slice(&self.section_alignment.to_le_bytes());
        buffer.extend_from_slice(&self.file_alignment.to_le_bytes());
        buffer.extend_from_slice(&6u16.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&6u16.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&0u32.to_le_bytes());

        let total_sections_size = if import_size > 0 {
            self.align(code_size, self.section_alignment) + self.align(import_size, self.section_alignment)
        } else {
            self.align(code_size, self.section_alignment)
        };
        let image_size = 0x1000 + total_sections_size;
        buffer.extend_from_slice(&image_size.to_le_bytes());
        buffer.extend_from_slice(&0x200u32.to_le_bytes());
        buffer.extend_from_slice(&0u32.to_le_bytes());
        buffer.extend_from_slice(&3u16.to_le_bytes());
        buffer.extend_from_slice(&0x0140u16.to_le_bytes());
        buffer.extend_from_slice(&0x100000u64.to_le_bytes());
        buffer.extend_from_slice(&0x1000u64.to_le_bytes());
        buffer.extend_from_slice(&0x100000u64.to_le_bytes());
        buffer.extend_from_slice(&0x1000u64.to_le_bytes());
        buffer.extend_from_slice(&0u32.to_le_bytes());
        buffer.extend_from_slice(&16u32.to_le_bytes());

        buffer.extend_from_slice(&0u32.to_le_bytes());
        buffer.extend_from_slice(&0u32.to_le_bytes());

        if import_size > 0 {
            let import_rva = 0x1000 + self.align(code_size, self.section_alignment);
            buffer.extend_from_slice(&import_rva.to_le_bytes());
            buffer.extend_from_slice(&import_size.to_le_bytes());
        } else {
            buffer.extend_from_slice(&0u32.to_le_bytes());
            buffer.extend_from_slice(&0u32.to_le_bytes());
        }

        for _ in 0..14 {
            buffer.extend_from_slice(&0u32.to_le_bytes());
            buffer.extend_from_slice(&0u32.to_le_bytes());
        }
    }

    fn write_section_headers(&self, buffer: &mut Vec<u8>, code_size: u32, import_size: u32, num_sections: u16) {
        let name = b".text\0\0\0";
        buffer.extend_from_slice(name);
        buffer.extend_from_slice(&code_size.to_le_bytes());
        buffer.extend_from_slice(&0x1000u32.to_le_bytes());
        buffer.extend_from_slice(&code_size.to_le_bytes());
        buffer.extend_from_slice(&0x200u32.to_le_bytes());
        buffer.extend_from_slice(&0u32.to_le_bytes());
        buffer.extend_from_slice(&0u32.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&0x60000020u32.to_le_bytes());

        if num_sections > 1 {
            let idata_name = b".idata\0\0";
            buffer.extend_from_slice(idata_name);
            buffer.extend_from_slice(&import_size.to_le_bytes());
            let idata_rva = 0x1000 + self.align(code_size, self.section_alignment);
            buffer.extend_from_slice(&idata_rva.to_le_bytes());
            buffer.extend_from_slice(&import_size.to_le_bytes());
            let idata_offset = 0x200 + code_size;
            buffer.extend_from_slice(&idata_offset.to_le_bytes());
            buffer.extend_from_slice(&0u32.to_le_bytes());
            buffer.extend_from_slice(&0u32.to_le_bytes());
            buffer.extend_from_slice(&0u16.to_le_bytes());
            buffer.extend_from_slice(&0u16.to_le_bytes());
            buffer.extend_from_slice(&0xC0000040u32.to_le_bytes());
        }
    }

    fn align(&self, value: u32, alignment: u32) -> u32 {
        (value + alignment - 1) & !(alignment - 1)
    }

    fn build_import_data(&self) -> Vec<u8> {
        let mut data = Vec::new();

        let base_rva = 0x1000 + self.section_alignment;

        let descriptor_offset = data.len();
        data.extend_from_slice(&[0u8; 40]);

        let name_rva = base_rva + data.len() as u32;
        data.extend_from_slice(b"KERNEL32.dll\0");
        while data.len() % 2 != 0 { data.push(0); }

        let ilt_rva = base_rva + data.len() as u32;
        let ilt_start = data.len();
        data.extend_from_slice(&[0u8; 32]);

        let iat_rva = base_rva + data.len() as u32;
        let iat_start = data.len();
        data.extend_from_slice(&[0u8; 32]);

        let mut hint_name_rvas = Vec::new();

        let pos1 = data.len() as u32 + base_rva;
        hint_name_rvas.push(pos1);
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(b"GetStdHandle\0");
        while data.len() % 2 != 0 { data.push(0); }

        let pos2 = data.len() as u32 + base_rva;
        hint_name_rvas.push(pos2);
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(b"WriteFile\0");
        while data.len() % 2 != 0 { data.push(0); }

        let pos3 = data.len() as u32 + base_rva;
        hint_name_rvas.push(pos3);
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(b"ExitProcess\0");
        while data.len() % 2 != 0 { data.push(0); }

        for (i, &rva) in hint_name_rvas.iter().enumerate() {
            let offset = ilt_start + i * 8;
            data[offset..offset+8].copy_from_slice(&(rva as u64).to_le_bytes());
            let offset = iat_start + i * 8;
            data[offset..offset+8].copy_from_slice(&(rva as u64).to_le_bytes());
        }

        data[descriptor_offset..descriptor_offset+4].copy_from_slice(&ilt_rva.to_le_bytes());
        data[descriptor_offset+4..descriptor_offset+8].copy_from_slice(&0u32.to_le_bytes());
        data[descriptor_offset+8..descriptor_offset+12].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        data[descriptor_offset+12..descriptor_offset+16].copy_from_slice(&name_rva.to_le_bytes());
        data[descriptor_offset+16..descriptor_offset+20].copy_from_slice(&iat_rva.to_le_bytes());

        data
    }

    fn patch_import_addresses(&self, code: &mut [u8], code_size: u32) {
        let idata_rva = 0x1000 + self.align(code_size, self.section_alignment);

        let iat_rva = idata_rva + 40 + 14 + 32;

        for i in 0..code.len().saturating_sub(5) {
            if code[i] == 0xFF && code[i+1] == 0x15 {
                let placeholder = i32::from_le_bytes([
                    code[i+2], code[i+3], code[i+4], code[i+5]
                ]);

                let instr_end = i + 6;
                let target_rva = instr_end as u32 + 0x1000;

                let offset = if placeholder == 0x2000_0000u32 as i32 {
                    (iat_rva as i32) - (target_rva as i32)
                } else if placeholder == 0x2008_0000u32 as i32 {
                    (iat_rva as i32 + 8) - (target_rva as i32)
                } else if placeholder == 0x1000_0000u32 as i32 {
                    (iat_rva as i32 + 16) - (target_rva as i32)
                } else {
                    continue;
                };

                code[i+2..i+6].copy_from_slice(&offset.to_le_bytes());
            }
        }
    }
}